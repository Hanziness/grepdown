use anyhow::Result;
use std::collections::HashMap;
use mddb::{Lint, StaleRef};

pub fn lint() -> Result<()> {
    let project = mddb::MDDBProject::new(".")?;
    project.refresh()?;
    let diags = mddb::run_lints(project.get_conn())?;

    if diags.is_empty() {
        println!("No lint issues found.");
        std::process::exit(0);
    }

    // Build lint registry
    let lints: Vec<Box<dyn Lint>> = vec![Box::new(StaleRef)];
    let lint_map: HashMap<&str, &dyn Lint> = lints.iter().map(|l| (l.id(), l.as_ref())).collect();

    // Group by lint_id
    let mut by_lint: HashMap<&str, Vec<&mddb::Diagnostic>> = HashMap::new();
    for d in &diags {
        by_lint.entry(d.lint_id).or_default().push(d);
    }

    // Print each lint's output
    for (lint_id, lint_diags) in &by_lint {
        if let Some(lint) = lint_map.get(lint_id) {
            println!("⚠️  {}\n", lint.title());
            println!("The following files were updated, but their dependents may need review:\n");

            // Group by updated file (to_path)
            let mut by_updated: HashMap<String, Vec<&&mddb::Diagnostic>> = HashMap::new();
            for d in lint_diags {
                by_updated.entry(d.to_path.clone()).or_default().push(d);
            }

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

            println!("{}\n", lint.suggestions());
        }
    }

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
