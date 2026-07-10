use rusqlite::params;
use crate::error::Result;
use crate::project::MDDBProject;

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub path: String,
    pub snippet: String,
    pub score: f64,
}

impl MDDBProject {
    /// Search the indexed documents using FTS5 full-text search.
    /// 
    /// The query string supports FTS5 syntax (e.g., "word1 word2", "word1 OR word2",
    /// "word1 NEAR word2", "prefix*"). Searches both path and body columns.
    /// 
    /// Results are ranked by BM25 relevance (lower score = better match).
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "SELECT path, snippet(documents_fts, 1, '<b>', '</b>', ' ... ', 32), bm25(documents_fts)
             FROM documents_fts
             WHERE documents_fts MATCH ?1
             ORDER BY bm25(documents_fts)
             LIMIT ?2"
        )?;
        
        let results = stmt.query_map(params![query, limit as i64], |row| {
            Ok(SearchResult {
                path: row.get(0)?,
                snippet: row.get(1)?,
                score: row.get(2)?,
            })
        })?;
        
        let mut output = Vec::new();
        for result in results {
            output.push(result?);
        }
        
        Ok(output)
    }
}
