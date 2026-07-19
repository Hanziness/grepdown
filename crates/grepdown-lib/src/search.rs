use rusqlite::params;
use serde::Serialize;
use crate::error::Result;
use crate::project::MDDBProject;

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

#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub target: String,
    pub raw_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReachableNode {
    pub path: String,
    pub depth: i64,
}

impl MDDBProject {
    /// Search the indexed documents using FTS5 full-text search.
    /// 
    /// The query string supports FTS5 syntax (e.g., "word1 word2", "word1 OR word2",
    /// "word1 NEAR word2", "prefix*"). Searches body content and tags.
    /// 
    /// Results are ranked by BM25 relevance (lower score = better match).
    pub fn search(&self, query: &str, limit: usize, path_filter: Option<&str>) -> Result<Vec<SearchResult>> {
        let conn = self.get_conn();
        let path_like = match path_filter {
            Some(prefix) => format!("{}%", prefix),
            None => "%".to_string(),
        };
        let mut stmt = conn.prepare(
            "SELECT path, snippet, score FROM (
                SELECT path,
                       snippet(documents_fts, 1, '<b>', '</b>', ' ... ', 32) as snippet,
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
        
        stmt.query_map(params![query, limit as i64, path_like], |row| {
            Ok(SearchResult {
                path: row.get(0)?,
                snippet: row.get(1)?,
                score: row.get(2)?,
            })
        })?.map(|r| r.map_err(Into::into))
          .collect::<Result<Vec<_>>>()
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
}
