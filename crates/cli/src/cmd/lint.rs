use anyhow::Result;
use std::collections::HashMap;

pub fn lint() -> Result<()> {
    let project = mddb::MDDBProject::new(".")?;
    project.refresh()?;
    let diags = mddb::run_lints(project.get_conn())?;

    if diags.is_empty() {
        println!("No lint issues found.");
        std::process::exit(0);
    }

    // Group by updated file (to_path)
    let mut by_updated: HashMap<String, Vec<&mddb::Diagnostic>> = HashMap::new();
    for d in &diags {
        by_updated.entry(d.to_path.clone()).or_default().push(d);
    }

    println!("⚠️  STALE REFERENCES DETECTED\n");
    println!("The following files were updated, but their dependents may need review:\n");

    for (updated_file, deps) in &by_updated {
        // Extract version from first diagnostic's message
        let version_info = deps[0].message.split("version ").nth(2).unwrap_or("?");
        println!("📄 {} (version {})", updated_file, version_info.trim());
        println!("   └─ Referenced by:");
        for dep in deps {
            let pinned_ver = dep.message.split("version ").nth(1).and_then(|s| s.split(" is").next()).unwrap_or("?");
            println!("      • {} (pinned at version {})", dep.from_path, pinned_ver);
        }
        println!();
    }

    println!("💡 Suggested actions:");
    println!("   1. Update them if needed, or");
    println!("   2. Run `mddb-cli approve-edits` to mark them as reviewed\n");

    println!("{} issue(s) found.", diags.len());
    std::process::exit(1);
}

pub fn approve(all: bool, paths: &[String]) -> Result<()> {
    if !all && paths.is_empty() {
        println!("Usage: mddb-cli approve-edits [OPTIONS] [PATHS]...\n");
        println!("Approve stale references for specific files or all files.\n");
        println!("Options:");
        println!("  --all    Approve all stale references");
        println!("\nArguments:");
        println!("  [PATHS]...  Specific file or folder paths to approve\n");
        println!("Examples:");
        println!("  mddb-cli approve-edits --all");
        println!("  mddb-cli approve-edits path/to/file.md");
        println!("  mddb-cli approve-edits path/to/folder/");
        return Ok(());
    }

    let project = mddb::MDDBProject::new(".")?;
    project.refresh()?;
    
    let n = if all {
        mddb::approve_edits(project.get_conn(), None)?
    } else {
        mddb::approve_edits(project.get_conn(), Some(paths))?
    };
    
    println!("Approved {} link(s).", n);
    Ok(())
}
