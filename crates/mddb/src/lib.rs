mod db;
mod error;
mod project;
mod search;

pub use error::{Error, Result};
pub use project::MDDBProject;
pub use search::SearchResult;

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn it_works() {
        assert_eq!(42, 42);
    }
}
