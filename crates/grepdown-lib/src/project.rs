use rusqlite::Connection;
use crate::error::Result;
use crate::error::Error;

use crate::db;
use crate::db::DB_PATH;

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

    /// Open an existing project. Returns an error if no project database exists.
    pub fn open(root: impl AsRef<std::path::Path>) -> Result<Self> {
        let root_path = root.as_ref().canonicalize()?.to_string_lossy().into_owned();
        let db_path = std::path::Path::new(&root_path).join(DB_PATH);
        if !db_path.exists() {
            return Err(Error::ProjectNotFound);
        }
        Self::new(root)
    }

    /// Get a reference to the project's database connection
    pub fn get_conn(&self) -> &Connection {
        &self.conn
    }

    pub fn get_root(&self) -> &str {
        &self.root
    }
}