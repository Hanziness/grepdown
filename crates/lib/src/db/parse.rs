use crate::error::Result;
use ignore::WalkBuilder;
use pulldown_cmark::{Event, Parser, Tag};
use rayon::prelude::*;
use rusqlite::params;
use std::{
    collections::{HashMap, HashSet},
    fs,
    os::unix::fs::MetadataExt,
    path::Path,
};

use crate::frontmatter::{extract_tags, parse_frontmatter};
use crate::project::MDDBProject;

const STMT_MTIME: &str = "SELECT path, mtime, content_hash FROM documents";
const STMT_DEL_FTS: &str = "DELETE FROM documents_fts WHERE path = ?1";
const STMT_INS_FTS: &str = "INSERT INTO documents_fts (path, body) VALUES (?1, ?2)";
const STMT_UPD_META: &str = "INSERT INTO documents (path, mtime, content_hash) VALUES (?1, ?2, ?3) ON CONFLICT(path) DO UPDATE SET mtime = excluded.mtime, content_hash = excluded.content_hash, version = CASE WHEN content_hash != excluded.content_hash THEN version + 1 ELSE version END";
const STMT_DEL_TAGS: &str = "DELETE FROM tags_fts WHERE path = ?1";
const STMT_INS_TAGS: &str = "INSERT INTO tags_fts (path, tags) VALUES (?1, ?2)";
const STMT_DEL_LINKS: &str = "DELETE FROM links WHERE from_id = ?1";
const STMT_INS_LINK: &str = "INSERT INTO links (from_id, to_id, raw_target, pinned_version, anchor) VALUES (?1, ?2, ?3, (SELECT version FROM documents WHERE path = ?2), ?4)";
const STMT_DEL_CITATIONS: &str = "DELETE FROM citations WHERE from_id = ?1";
const STMT_INS_CITATION: &str =
    "INSERT INTO citations (from_id, url, raw_target) VALUES (?1, ?2, ?3)";
const STMT_DEL_BROKEN: &str = "DELETE FROM broken_links WHERE from_id = ?1";
const STMT_INS_BROKEN: &str = "INSERT INTO broken_links (from_id, raw_target) VALUES (?1, ?2)";
const STMT_DEL_HEADINGS: &str = "DELETE FROM headings WHERE path = ?1";
const STMT_INS_HEADING: &str =
    "INSERT INTO headings (path, level, text, anchor) VALUES (?1, ?2, ?3, ?4)";
const STMT_DEL_HEADINGS_FTS: &str = "DELETE FROM headings_fts WHERE path = ?1";
const STMT_INS_HEADINGS_FTS: &str = "INSERT INTO headings_fts (path, headings) VALUES (?1, ?2)";

/// Extract all links from markdown content.
/// Returns (target, is_external, anchor) where is_external=true means citation (URL).
fn extract_links(content: &str) -> Vec<(String, bool, Option<String>)> {
    Parser::new(content)
        .filter_map(|event| match event {
            Event::Start(Tag::Link { dest_url, .. })
            | Event::Start(Tag::Image { dest_url, .. }) => {
                let url = dest_url.to_string();
                let is_external =
                    url.contains("://") || url.starts_with("mailto:") || url.starts_with("//");

                // Extract anchor (fragment) from URL
                let (target, anchor) = if let Some(hash_pos) = url.find('#') {
                    let anchor = url[hash_pos + 1..].to_string();
                    let target = url[..hash_pos].to_string();
                    (target, Some(anchor))
                } else {
                    (url, None)
                };

                Some((target, is_external, anchor))
            }
            _ => None,
        })
        .collect()
}

/// Extract all headings from markdown content.
/// Returns (level, text) pairs where level is 1-6.
fn extract_headings(content: &str) -> Vec<(i32, String)> {
    let mut headings = Vec::new();
    let mut in_heading = false;
    let mut current_level = 0;
    let mut current_text = String::new();

    for event in Parser::new(content) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading = true;
                current_level = level as i32;
                current_text.clear();
            }
            Event::Text(text) if in_heading => {
                current_text.push_str(&text);
            }
            Event::End(_) if in_heading => {
                headings.push((current_level, current_text.clone()));
                in_heading = false;
            }
            _ => {}
        }
    }

    headings
}

/// Convert heading text to a URL-friendly anchor slug.
/// - Lowercase
/// - Replace spaces with hyphens
/// - Remove non-alphanumeric characters (except hyphens)
/// - Collapse consecutive hyphens
fn slugify(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut last_was_hyphen = false;

    for c in text.chars() {
        if c.is_alphanumeric() {
            result.push(c.to_ascii_lowercase());
            last_was_hyphen = false;
        } else if c == ' ' || c == '-' {
            if !last_was_hyphen && !result.is_empty() {
                result.push('-');
                last_was_hyphen = true;
            }
        }
    }

    // Remove trailing hyphen
    if result.ends_with('-') {
        result.pop();
    }

    result
}

/// Result of indexing a single document in the parallel phase.
/// Holds the fully-resolved link data so the write transaction never touches the filesystem.
struct IndexedDoc {
    path: String,
    mtime: i64,
    content: String,
    hash: Vec<u8>,
    tags: String,
    /// (resolved_target, raw_target, anchor) — deduped by resolved_target
    resolved_links: Vec<(String, String, Option<String>)>,
    /// External URLs — deduped
    citations: Vec<String>,
    /// Raw targets that couldn't be resolved
    broken_raw: Vec<String>,
    /// (level, text, anchor)
    headings: Vec<(i32, String, String)>,
}

/// Resolve a bundle-relative link target to a canonical document path using only
/// in-memory path-set membership — no filesystem access.
/// Returns `None` if the target isn't in `current_paths` (i.e., the link is broken
/// or escapes the project root).
fn resolve_in_set(
    current_paths: &HashSet<String>,
    base_path: &str,
    target: &str,
) -> Option<String> {
    let base_dir = Path::new(base_path).parent()?;
    let normalized = normalize_path(&base_dir.join(target));

    let direct = normalized.with_extension("md");
    let direct_s = direct.to_string_lossy().into_owned();
    if current_paths.contains(&direct_s) {
        return Some(direct_s);
    }

    let index = normalized.join("index.md");
    let index_s = index.to_string_lossy().into_owned();
    if current_paths.contains(&index_s) {
        return Some(index_s);
    }

    None
}

/// Normalize a path by resolving . and .. components without following symlinks.
fn normalize_path(path: &Path) -> std::path::PathBuf {
    let mut normalized = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {
                // Skip .
            }
            _ => {
                normalized.push(component);
            }
        }
    }
    normalized
}

/// Result of processing a file whose mtime changed.
/// `Changed` = content differs → full re-index.
/// `Unchanged` = content matches → only mtime needs write-back.
enum ParseResult {
    Changed(IndexedDoc),
    Unchanged {
        path: String,
        mtime: i64,
        hash: Vec<u8>,
    },
}

impl MDDBProject {
    /// Refresh the database and index files not seen before
    pub fn refresh(&self) -> Result<Vec<(String, i64)>> {
        let root = self.get_root();
        let mut known = HashMap::<String, (i64, Vec<u8>)>::new();
        let conn = self.get_conn();

        // Load known mtimes into memory
        {
            let mut stmt = conn.prepare(STMT_MTIME)?;
            let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;

            for r in rows {
                let (path, mtime, content_hash): (String, i64, Vec<u8>) = r?;
                known.insert(path, (mtime, content_hash));
            }
        }

        // Walk and diff — respects .gitignore, .ignore, .rgignore automatically
        let mut changed: Vec<(String, i64)> = Vec::new();
        let mut current_paths = HashSet::new();
        let mut walked = 0usize;

        for entry in WalkBuilder::new(self.get_root())
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |x| x == "md"))
        {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let mtime = meta.mtime();
            let abs_path = entry.path();
            let rel_path = abs_path.strip_prefix(root).unwrap_or(abs_path);
            let path = rel_path.to_string_lossy().into_owned();

            current_paths.insert(path.clone());
            walked += 1;
            match known.get(&path) {
                Some(&(old_mtime, _)) if old_mtime == mtime => {}
                _ => changed.push((path, mtime)),
            }
        }
        log::debug!("Walked {} files, {} changed", walked, changed.len());

        // Detect deleted files (known from DB but no longer on disk)
        let deleted: Vec<String> = known
            .keys()
            .filter(|k| !current_paths.contains(k.as_str()))
            .cloned()
            .collect();

        // Parallel read changed files (level-2: skip link resolution if content unchanged)
        let results: Vec<ParseResult> = changed
            .par_iter()
            .map(|(path, mtime)| {
                let content = fs::read_to_string(Path::new(root).join(path)).unwrap_or_else(|e| {
                    log::warn!("Failed to read {}: {}", path, e);
                    String::new()
                });
                let hash = blake3::hash(content.as_bytes()).as_bytes().to_vec();

                if let Some((_, old_hash)) = known.get(path) {
                    if *old_hash == hash {
                        // Content unchanged — record mtime-only touch, skip parsing
                        return ParseResult::Unchanged {
                            path: path.clone(),
                            mtime: *mtime,
                            hash,
                        };
                    }
                }

                let tags = parse_frontmatter(&content)
                    .map(|fm| extract_tags(&fm))
                    .unwrap_or_default();
                let tags_str = tags.join(" ");

                let mut links = extract_links(&content);
                links.sort();
                links.dedup();

                // Resolve all links via in-memory set membership — zero syscalls
                let mut resolved_map: HashMap<String, (String, Option<String>)> = HashMap::new();
                let mut citation_set: HashSet<String> = HashSet::new();
                let mut broken_raw: Vec<String> = Vec::new();

                for (target, is_external, anchor) in &links {
                    if *is_external {
                        citation_set.insert(target.clone());
                    } else {
                        match resolve_in_set(&current_paths, path, target) {
                            Some(resolved) => {
                                resolved_map.insert(resolved, (target.clone(), anchor.clone()));
                            }
                            None => {
                                broken_raw.push(target.clone());
                            }
                        }
                    }
                }

                // Extract headings and generate anchors
                let raw_headings = extract_headings(&content);
                let headings: Vec<(i32, String, String)> = raw_headings
                    .into_iter()
                    .map(|(level, text)| {
                        let anchor = slugify(&text);
                        (level, text, anchor)
                    })
                    .collect();

                ParseResult::Changed(IndexedDoc {
                    path: path.clone(),
                    mtime: *mtime,
                    content,
                    hash,
                    tags: tags_str,
                    resolved_links: resolved_map
                        .into_iter()
                        .map(|(resolved, (raw, anchor))| (resolved, raw, anchor))
                        .collect(),
                    citations: citation_set.into_iter().collect(),
                    broken_raw,
                    headings,
                })
            })
            .collect();

        // Split into changed-docs (full re-index) and touch-only (mtime write-back)
        let mut changed_docs: Vec<IndexedDoc> = Vec::new();
        let mut touch: Vec<(String, i64, Vec<u8>)> = Vec::new();
        for r in results {
            match r {
                ParseResult::Changed(doc) => changed_docs.push(doc),
                ParseResult::Unchanged { path, mtime, hash } => touch.push((path, mtime, hash)),
            }
        }

        // Rebuild changed return value from actually-reindexed docs
        changed = changed_docs
            .iter()
            .map(|doc| (doc.path.clone(), doc.mtime))
            .collect();
        log::info!(
            "Indexed {} files, {} mtime-only touches",
            changed_docs.len(),
            touch.len()
        );

        // Do a single transaction for the whole batch
        let tx = conn.unchecked_transaction()?;
        {
            let mut del_fts = tx.prepare(STMT_DEL_FTS)?;
            let mut ins_fts = tx.prepare(STMT_INS_FTS)?;
            let mut upsert_meta = tx.prepare(STMT_UPD_META)?;
            let mut del_tags = tx.prepare(STMT_DEL_TAGS)?;
            let mut ins_tags = tx.prepare(STMT_INS_TAGS)?;

            // Phase 1: Upsert all documents first (so FK constraints pass for links)
            for doc in &changed_docs {
                del_fts.execute(params![doc.path])?;
                ins_fts.execute(params![doc.path, doc.content])?;
                upsert_meta.execute(params![doc.path, doc.mtime, doc.hash])?;
                del_tags.execute(params![doc.path])?;
                if !doc.tags.is_empty() {
                    ins_tags.execute(params![doc.path, doc.tags])?;
                }
            }

            // Phase 2: Insert links/citations/broken (all documents exist, resolution done in parallel phase)
            let mut del_links = tx.prepare(STMT_DEL_LINKS)?;
            let mut ins_link = tx.prepare(STMT_INS_LINK)?;
            let mut del_citations = tx.prepare(STMT_DEL_CITATIONS)?;
            let mut ins_citation = tx.prepare(STMT_INS_CITATION)?;
            let mut del_broken = tx.prepare(STMT_DEL_BROKEN)?;
            let mut ins_broken = tx.prepare(STMT_INS_BROKEN)?;
            for doc in &changed_docs {
                del_links.execute(params![doc.path])?;
                del_citations.execute(params![doc.path])?;
                del_broken.execute(params![doc.path])?;

                for (resolved, raw, anchor) in &doc.resolved_links {
                    ins_link.execute(params![doc.path, resolved, raw, anchor])?;
                }
                for url in &doc.citations {
                    ins_citation.execute(params![doc.path, url, url])?;
                }
                for raw in &doc.broken_raw {
                    ins_broken.execute(params![doc.path, raw])?;
                }
            }

            // Phase 3: Insert headings (structured + FTS)
            let mut del_headings = tx.prepare(STMT_DEL_HEADINGS)?;
            let mut ins_heading = tx.prepare(STMT_INS_HEADING)?;
            let mut del_headings_fts = tx.prepare(STMT_DEL_HEADINGS_FTS)?;
            let mut ins_headings_fts = tx.prepare(STMT_INS_HEADINGS_FTS)?;
            for doc in &changed_docs {
                del_headings.execute(params![doc.path])?;
                del_headings_fts.execute(params![doc.path])?;

                for (level, text, anchor) in &doc.headings {
                    ins_heading.execute(params![doc.path, level, text, anchor])?;
                }

                // Insert all heading text into FTS for search
                if !doc.headings.is_empty() {
                    let headings_text: Vec<&str> = doc
                        .headings
                        .iter()
                        .map(|(_, text, _)| text.as_str())
                        .collect();
                    ins_headings_fts.execute(params![doc.path, headings_text.join(" ")])?;
                }
            }

            // Remove files deleted from disk
            let mut del_stale = tx.prepare("DELETE FROM documents WHERE path = ?1")?;
            for path in &deleted {
                del_fts.execute(params![path])?;
                del_tags.execute(params![path])?;
                del_headings.execute(params![path])?;
                del_headings_fts.execute(params![path])?;
                del_stale.execute(params![path])?;
            }
        }
        tx.commit()?;
        log::debug!(
            "Committed transaction with {} rows, {} deleted",
            changed_docs.len(),
            deleted.len()
        );

        // Write back new mtimes for files whose content hash was unchanged,
        // so subsequent refreshes don't re-read and re-hash them.
        if !touch.is_empty() {
            let tx = conn.unchecked_transaction()?;
            {
                let mut upd_meta = tx.prepare(STMT_UPD_META)?;
                for (path, mtime, hash) in &touch {
                    upd_meta.execute(params![path, mtime, hash])?;
                }
            }
            tx.commit()?;
            log::debug!("Wrote back mtime for {} hash-unchanged files", touch.len());
        }

        Ok(changed)
    }
}
