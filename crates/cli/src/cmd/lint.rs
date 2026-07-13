use anyhow::Result;

pub fn lint() -> Result<()> {
    let project = mddb::MDDBProject::new(".")?;
    project.refresh()?;
    let diags = mddb::run_lints(project.get_conn())?;

    if diags.is_empty() {
        println!("No lint issues found.");
        std::process::exit(0);
    }

    for d in &diags {
        let severity = match d.severity {
            mddb::Severity::Error => "ERROR",
            mddb::Severity::Warning => "WARNING",
        };
        println!("{}: {} ({} → {})", severity, d.message, d.from_path, d.to_path);
    }

    println!("\n{} issue(s) found.", diags.len());
    std::process::exit(1);
}

pub fn approve() -> Result<()> {
    let project = mddb::MDDBProject::new(".")?;
    project.refresh()?;
    let n = mddb::approve_edits(project.get_conn())?;
    println!("Approved {} link(s).", n);
    Ok(())
}
