use std::path::PathBuf;

use rusqlite::{Connection, Result};

mod init;
mod parse;

pub const DB_PATH: &str = "md.db";

/** Start the database engine at the default location */
pub fn start(root: &String) -> Result<Connection> {
    let conn = Connection::open(PathBuf::from_iter([root, DB_PATH]))?;
    init::bootstrap(&conn)?;

    return Ok(conn);
}