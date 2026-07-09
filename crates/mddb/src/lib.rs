mod db;
mod project;

pub use project::MDDBProject;

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn it_works() {
        assert_eq!(42, 42);
    }
}
