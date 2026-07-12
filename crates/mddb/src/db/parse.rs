use std::{collections::{HashMap, HashSet}, fs, os::unix::fs::MetadataExt, path::Path};
use rayon::prelude::*;
use rusqlite::params;
use walkdir::WalkDir;
use pulldown_cmark::{Event, Parser, Tag};
use crate::error::Result;

use crate::project::MDDBProject;
use crate::frontmatter::{parse_frontmatter, extract_tags};

const STMT_MTIME: &str = "SELECT path, mtime, content_hash FROM documents";
const STMT_DEL_FTS: &str = "DELETE FROM documents_fts WHERE path = ?1";
const STMT_INS_FTS: &str = "INSERT INTO documents_fts (path, body) VALUES (?1, ?2)";
const STMT_UPD_META: &str = "INSERT INTO documents (path, mtime, content_hash) VALUES (?1, ?2, ?3) ON CONFLICT(path) DO UPDATE SET mtime = excluded.mtime, content_hash = excluded.content_hash";
const STMT_DEL_TAGS: &str = "DELETE FROM tags_fts WHERE path = ?1";
const STMT_INS_TAGS: &str = "INSERT INTO tags_fts (path, tags) VALUES (?1, ?2)";
const STMT_DEL_LINKS: &str = "DELETE FROM links WHERE from_id = ?1";
const STMT_INS_LINK: &str = "INSERT INTO links (from_id, to_id, raw_target) VALUES (?1, ?2, ?3)";
const STMT_DEL_CITATIONS: &str = "DELETE FROM citations WHERE from_id = ?1";
const STMT_INS_CITATION: &str = "INSERT INTO citations (from_id, url, raw_target) VALUES (?1, ?2, ?3)";

/// Extract all links from markdown content.
/// Returns (target, is_external) where is_external=true means citation (URL).
fn extract_links(content: &str) -> Vec<(String, bool)> {
    Parser::new(content)
        .filter_map(|event| match event {
            Event::Start(Tag::Link { dest_url, .. }) | Event::Start(Tag::Image { dest_url, .. }) => {
                let url = dest_url.to_string();
                let is_external = url.starts_with("http://") || url.starts_with("https://") || url.contains("://");
                Some((url, is_external))
            }
            _ => None,
        })
        .collect()
}

/// Resolve a bundle-relative link target to a canonical document path.
/// Tries: target.md, target/index.md
fn resolve_link(base_path: &str, target: &str) -> Option<String> {
    let base_dir = Path::new(base_path).parent()?;
    let resolved = base_dir.join(target);
    
    // Normalize the path to resolve .. and . components
    let normalized = normalize_path(&resolved);
    
    // Try direct: target.md
    let direct = normalized.with_extension("md");
    if direct.exists() {
        return Some(direct.to_string_lossy().into_owned());
    }
    
    // Try directory index: target/index.md
    let index = normalized.join("index.md");
    if index.exists() {
        return Some(index.to_string_lossy().into_owned());
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

impl MDDBProject {
    /// Refresh the database and index files not seen before
    pub fn refresh(&self) -> Result<Vec<(String, i64)>> {
        let mut known = HashMap::<String, (i64, String)>::new();
        let conn = self.get_conn();

        // Load known mtimes into memory
        {
            let mut stmt = conn.prepare(STMT_MTIME)?;
            let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;

            for r in rows {
                let (path, mtime, content_hash): (String, i64, String) = r?;
                known.insert(path, (mtime, content_hash));
            }
        }

        // Walk and diff
        let mut changed: Vec<(String, i64)> = Vec::new();
        let mut current_paths = HashSet::new();
        let mut walked = 0usize;
        
        for entry in WalkDir::new(self.get_root())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |x| x == "md")) {
                let meta = match entry.metadata() { Ok(m) => m, Err(_) => continue };
                let mtime = meta.mtime();
                let path = entry.path().to_string_lossy().into_owned();

                current_paths.insert(path.clone());
                walked += 1;
                match known.get(&path) {
                    Some(&(old_mtime, _)) if old_mtime == mtime => { },
                    _ => changed.push((path, mtime)),
                }
            }
        log::debug!("Walked {} files, {} changed", walked, changed.len());

        // Detect deleted files (known from DB but no longer on disk)
        let deleted: Vec<String> = known.keys().filter(|k| !current_paths.contains(k.as_str())).cloned().collect();

        // Parallel read changed files (level-2: skip if content unchanged)
        let read_results: Vec<(String, i64, String, String, String, Vec<(String, bool)>)> = changed
            .par_iter()
            .filter_map(|(path, mtime)| {
                let content = fs::read_to_string(path).unwrap_or_else(|e| { log::warn!("Failed to read {}: {}", path, e); String::new() });
                let hash = blake3::hash(&content.as_bytes()).to_string();

                if let Some((_, old_hash)) = known.get(path) {
                    if *old_hash == hash {
                        return None; // content unchanged, skip FTS re-index
                    }
                }

                let tags = parse_frontmatter(&content)
                    .map(|fm| extract_tags(&fm))
                    .unwrap_or_default();
                let tags_str = tags.join(" ");
                
                let links = extract_links(&content);

                Some((path.clone(), *mtime, content, hash, tags_str, links))
            })
            .collect();

        // Rebuild changed from read_results so it reflects only actually-processed files
        changed = read_results.iter().map(|(p, m, _, _, _, _)| (p.clone(), *m)).collect();

        log::info!("Indexed {} files", read_results.len());

        // Do a single transaction for the whole batch
        let tx = conn.unchecked_transaction()?;
        {
            let mut del_fts = tx.prepare(STMT_DEL_FTS)?;
            let mut ins_fts = tx.prepare(STMT_INS_FTS)?;
            let mut upsert_meta = tx.prepare(STMT_UPD_META)?;
            let mut del_tags = tx.prepare(STMT_DEL_TAGS)?;
            let mut ins_tags = tx.prepare(STMT_INS_TAGS)?;

            // Phase 1: Upsert all documents first (so FK constraints pass for links)
            for (path, mtime, content, hash, tags_str, _links) in &read_results {
                del_fts.execute(params![path])?;
                ins_fts.execute(params![path, content])?;
                upsert_meta.execute(params![path, mtime, hash])?;
                del_tags.execute(params![path])?;
                if !tags_str.is_empty() {
                    ins_tags.execute(params![path, tags_str])?;
                }
            }

            // Phase 2: Now insert links (all documents exist)
            let mut del_links = tx.prepare(STMT_DEL_LINKS)?;
            let mut ins_link = tx.prepare(STMT_INS_LINK)?;
            let mut del_citations = tx.prepare(STMT_DEL_CITATIONS)?;
            let mut ins_citation = tx.prepare(STMT_INS_CITATION)?;
            for (path, _mtime, _content, _hash, _tags_str, links) in &read_results {
                del_links.execute(params![path])?;
                del_citations.execute(params![path])?;
                for (target, is_external) in links {
                    if *is_external {
                        // Citation: store URL in separate table
                        ins_citation.execute(params![path, target, target])?;
                    } else {
                        // Cross-ref: resolve to canonical path
                        if let Some(resolved) = resolve_link(path, target) {
                            ins_link.execute(params![path, resolved, target])?;
                        }
                        // Unresolved links are silently dropped (target doesn't exist yet)
                    }
                }
            }

            // Remove files deleted from disk
            let mut del_stale = tx.prepare("DELETE FROM documents WHERE path = ?1")?;
            for path in &deleted {
                del_fts.execute(params![path])?;
                del_tags.execute(params![path])?;
                del_stale.execute(params![path])?;
            }

        }
        tx.commit()?;
        log::debug!("Committed transaction with {} rows, {} deleted", read_results.len(), deleted.len());

        Ok(changed)
    }
}