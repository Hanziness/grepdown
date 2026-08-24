use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("database error: {0}")]
    Rusqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("No project found in current directory. Run `init` first to initialize a project.")]
    ProjectNotFound,
}

pub type Result<T> = std::result::Result<T, Error>;
