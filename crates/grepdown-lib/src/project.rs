use rusqlite::Connection;
use crate::error::Result;

use crate::db;

#[derive(Debug)]
pub struct MDDBProject {
    root: String,
    conn: Connection
}

impl MDDBProject {
    pub fn new(root: impl AsRef<std::path::Path>) -> Result<Self> {
        let root_path = root.as_ref().canonicalize()?.to_string_lossy().into_owned();
        let conn = db::start(&root_path)?;

        Ok(Self {
            root: root_path,
            conn
        })
    }

    /// Get a reference to the project's database connection
    pub fn get_conn(&self) -> &Connection {
        &self.conn
    }

    pub fn get_root(&self) -> &str {
        &self.root
    }
}