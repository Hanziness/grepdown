use anyhow::{Context, Result};

pub fn search(query: &str, limit: usize, no_refresh: bool) -> Result<()> {
    let project = grepdown_lib::MDDBProject::new(".")
        .context("Failed to open project")?;

    if !no_refresh {
        log::info!("Refreshing index...");
        project.refresh().context("Failed to refresh index")?;
    }

    let results = project.search(query, limit)
        .context("Search failed")?;

    if results.is_empty() {
        println!("No results found.");
        return Ok(());
    }

    for result in results {
        // Strip HTML tags and apply ANSI bold
        let snippet = result.snippet
            .replace("<b>", "\x1b[1m")
            .replace("</b>", "\x1b[0m");
        
        println!("\x1b[1;32m{}\x1b[0m", result.path);
        println!("  {}", snippet);
        println!();
    }

    Ok(())
}
