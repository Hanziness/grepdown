use std::path::PathBuf;

use rusqlite::Connection;
use crate::error::Result;

mod init;
mod parse;

pub const DB_PATH: &str = "md.db";

/** Start the database engine at the default location */
pub fn start(root: &str) -> Result<Connection> {
    let db_path = PathBuf::from_iter([root, DB_PATH]);
    let conn = Connection::open(&db_path)?;
    log::debug!("Opened database at {}", &db_path.to_str().unwrap_or("<unknown>"));
    init::bootstrap(&conn)?;

    return Ok(conn);
}