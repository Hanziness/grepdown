use std::{collections::HashMap, fs, os::unix::fs::MetadataExt};
use rayon::prelude::*;
use rusqlite::{Result, params};
use walkdir::WalkDir;

use crate::project::MDDBProject;

const STMT_MTIME: &str = "SELECT path, mtime, content_hash FROM documents";
const STMT_DEL_FTS: &str = "DELETE FROM documents_fts WHERE path = ?1";
const STMT_INS_FTS: &str = "INSERT INTO documents_fts (path, body) VALUES (?1, ?2)";
const STMT_UPD_META: &str = "INSERT INTO documents (path, mtime, content_hash) VALUES (?1, ?2, ?3) ON CONFLICT(path) DO UPDATE SET mtime = excluded.mtime, content_hash = excluded.content_hash";

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
        let mut walked = 0usize;
        
        for entry in WalkDir::new(self.get_root())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |x| x == "md")) {
                let meta = entry.metadata().unwrap();
                let mtime = meta.mtime();
                let path = entry.path().to_string_lossy().into_owned();

                walked += 1;
                match known.get(&path) {
                    Some(&(old_mtime, _)) if old_mtime == mtime => { },
                    _ => changed.push((path, mtime)),
                }
            }
        log::debug!("Walked {} files, {} changed", walked, changed.len());

        // Parallel read changed files (level-2: skip if content unchanged)
        let read_results: Vec<(String, i64, String, String)> = changed
            .par_iter()
            .filter_map(|(path, mtime)| {
                let content = fs::read_to_string(path).unwrap_or_default();
                let hash = blake3::hash(&content.as_bytes()).to_string();

                if let Some((_, old_hash)) = known.get(path) {
                    if *old_hash == hash {
                        return None; // content unchanged, skip FTS re-index
                    }
                }

                Some((path.clone(), *mtime, content, hash))
            })
            .collect();

        // Rebuild changed from read_results so it reflects only actually-processed files
        changed = read_results.iter().map(|(p, m, _, _)| (p.clone(), *m)).collect();

        log::info!("Indexed {} files", read_results.len());

        // Do a single transaction for the whole batch
        let tx = conn.unchecked_transaction()?;
        {
            let mut del_fts = tx.prepare(STMT_DEL_FTS)?;
            let mut ins_fts = tx.prepare(STMT_INS_FTS)?;
            let mut upsert_meta = tx.prepare(STMT_UPD_META)?;

            for (path, mtime, content, hash) in &read_results {
                del_fts.execute(params![path])?;
                ins_fts.execute(params![path, content])?;
                upsert_meta.execute(params![path, mtime, hash])?;
            }

        }
        tx.commit()?;
        log::debug!("Committed transaction with {} rows", read_results.len());

        Ok(changed)
    }
}