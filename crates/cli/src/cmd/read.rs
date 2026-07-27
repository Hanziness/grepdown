use anyhow::{Context, Result};

pub fn read(path: &str) -> Result<()> {
    let project = grepdown_lib::MDDBProject::open(".").context("Failed to open project")?;
    let doc = project.read_document(path).context("Failed to read document")?;
    print!("{}", doc.content);
    Ok(())
}
