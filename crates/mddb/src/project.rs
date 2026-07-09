use std::path::Path;

use rusqlite::{Connection, Result};

use crate::db;

#[derive(Debug)]
pub struct MDDBProject {
    root: String,
    conn: Connection
}

impl MDDBProject {
    pub fn new(root: String) -> Result<Self> {
        let root_path = Path::new(&root);
        let conn = db::start(&root)?;

        return Ok(Self {
            root: root_path.canonicalize().unwrap().to_string_lossy().into_owned(),
            conn
        })
    }

    /// Get a reference to the project's database connection
    pub fn get_conn(&self) -> &Connection {
        return &self.conn;
    }

    pub fn get_root(&self) -> String {
        return self.root.clone();
    }
}