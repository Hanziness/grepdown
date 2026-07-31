use std::path::Path;

use rusqlite::Connection;
use crate::error::Result;

mod init;
mod parse;

#[cfg(test)]
pub use init::bootstrap;

pub const DB_PATH: &str = "md.db";

/// PRAGMAs that must be set on every connection (they are per-connection, not persistent).
const CONNECTION_PRAGMAS: &str = "\
    PRAGMA journal_mode = WAL;
    PRAGMA synchronous = NORMAL;
    PRAGMA foreign_keys = ON;
    PRAGMA cache_size = -8000;
    PRAGMA temp_store = MEMORY;
    PRAGMA mmap_size = 268435456;
";

/** Start the database engine at the default location */
pub fn start(root: &str) -> Result<Connection> {
    let db_path = Path::new(root).join(DB_PATH);
    let conn = Connection::open(&db_path)?;
    conn.execute_batch(CONNECTION_PRAGMAS)?;
    log::debug!("Opened database at {} (WAL, 8MB cache, 256MB mmap)", db_path.display());
    init::bootstrap(&conn)?;

    Ok(conn)
}