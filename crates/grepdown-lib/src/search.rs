use rusqlite::params;
use serde::Serialize;
use std::path::Path;
use crate::error::Result;
use crate::project::MDDBProject;
use crate::frontmatter::{parse_frontmatter, extract_tags};

/// Escape a query string so FTS5 treats it as a literal phrase.
/// Wraps the input in double quotes and escapes any inner `"` as `""`.
pub fn escape_fts5_query(query: &str) -> String {
    format!("\"{}\"", query.replace('"', "\"\""))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchResult {
    pub path: String,
    pub snippet: String,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Link {
    pub target: String,
    pub raw_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReachableNode {
    pub path: String,
    pub depth: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DocumentContent {
    pub path: String,
    pub content: String,
    pub tags: Vec<String>,
}

impl MDDBProject {
    /// Search the indexed documents using FTS5 full-text search.
    /// 
    /// The query string supports FTS5 syntax (e.g., "word1 word2", "word1 OR word2",
    /// "word1 NEAR word2", "prefix*"). Searches body content and tags.
    /// 
    /// Results are ranked by BM25 relevance (lower score = better match).
    pub fn search(&self, query: &str, limit: usize, path_filter: Option<&str>, snippet_length: Option<i64>) -> Result<Vec<SearchResult>> {
        let conn = self.get_conn();
        let path_like = match path_filter {
            Some(prefix) => format!("{}%", prefix),
            None => "%".to_string(),
        };
        let snippet_len = snippet_length.unwrap_or(32);
        // Fetch 2x results to allow for deduplication
        let fetch_limit = (limit * 2) as i64;
        let mut stmt = conn.prepare(
            "SELECT path, snippet, score FROM (
                SELECT path,
                       snippet(documents_fts, 1, '<b>', '</b>', ' ... ', ?4) as snippet,
                       bm25(documents_fts) as score
                FROM documents_fts
                WHERE documents_fts MATCH ?1 AND path LIKE ?3
                UNION ALL
                SELECT path,
                       tags as snippet,
                       bm25(tags_fts) as score
                FROM tags_fts
                WHERE tags_fts MATCH ?1 AND path LIKE ?3
            )
            ORDER BY score
            LIMIT ?2"
        )?;
        
        let mut raw: Vec<SearchResult> = stmt.query_map(params![query, fetch_limit, path_like, snippet_len], |row| {
            Ok(SearchResult {
                path: row.get(0)?,
                snippet: row.get(1)?,
                score: row.get(2)?,
            })
        })?.map(|r| r.map_err(Into::into))
          .collect::<Result<Vec<_>>>()?;

        // Deduplicate: keep first occurrence (best score), boost if matched in both
        let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let mut deduped: Vec<SearchResult> = Vec::new();
        for result in raw.drain(..) {
            if let Some(&idx) = seen.get(&result.path) {
                // Already seen — boost the existing result's score
                deduped[idx].score *= 0.9;
            } else {
                seen.insert(result.path.clone(), deduped.len());
                deduped.push(result);
            }
        }

        // Truncate to requested limit
        deduped.truncate(limit);
        Ok(deduped)
    }

    /// Get all links from a document (forward traversal).
    /// Returns cross-references to other documents.
    pub fn get_links_from(&self, from_id: &str) -> Result<Vec<Link>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "SELECT to_id, raw_target FROM links WHERE from_id = ?1"
        )?;
        
        stmt.query_map(params![from_id], |row| {
            Ok(Link {
                target: row.get(0)?,
                raw_target: row.get(1)?,
            })
        })?.map(|r| r.map_err(Into::into))
          .collect::<Result<Vec<_>>>()
    }

    /// Get all citations (external URLs) from a document.
    pub fn get_citations_from(&self, from_id: &str) -> Result<Vec<String>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "SELECT url FROM citations WHERE from_id = ?1"
        )?;
        
        stmt.query_map(params![from_id], |row| row.get(0))?
            .map(|r| r.map_err(Into::into))
            .collect::<Result<Vec<_>>>()
    }

    /// Get all links to a document (reverse traversal / backlinks).
    pub fn get_links_to(&self, to_id: &str) -> Result<Vec<Link>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "SELECT from_id, raw_target FROM links WHERE to_id = ?1"
        )?;
        
        stmt.query_map(params![to_id], |row| {
            Ok(Link {
                target: row.get(0)?,
                raw_target: row.get(1)?,
            })
        })?.map(|r| r.map_err(Into::into))
          .collect::<Result<Vec<_>>>()
    }

    /// BFS traversal: get all nodes reachable from a starting node up to max_depth hops.
    /// Returns nodes with their minimum depth from the start.
    pub fn get_reachable(&self, from_id: &str, max_depth: i64) -> Result<Vec<ReachableNode>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "WITH RECURSIVE bfs AS (
                SELECT to_id AS node, 1 AS depth 
                FROM links 
                WHERE from_id = ?1
                UNION ALL
                SELECT l.to_id, bfs.depth + 1
                FROM links l 
                JOIN bfs ON l.from_id = bfs.node
                WHERE bfs.depth < ?2
            )
            SELECT node, MIN(depth) AS depth 
            FROM bfs 
            GROUP BY node 
            ORDER BY depth"
        )?;
        
        stmt.query_map(params![from_id, max_depth], |row| {
            Ok(ReachableNode {
                path: row.get(0)?,
                depth: row.get(1)?,
            })
        })?.map(|r| r.map_err(Into::into))
          .collect::<Result<Vec<_>>>()
    }

    /// Read a document's content and extract its frontmatter tags.
    /// The path should be relative to the project root.
    pub fn read_document(&self, path: &str) -> Result<DocumentContent> {
        let full_path = Path::new(self.get_root()).join(path);
        let content = std::fs::read_to_string(&full_path)?;
        let tags = parse_frontmatter(&content)
            .map(|fm| extract_tags(&fm))
            .unwrap_or_default();

        Ok(DocumentContent {
            path: path.to_string(),
            content,
            tags,
        })
    }
}
