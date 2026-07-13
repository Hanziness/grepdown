use anyhow::Result;
use std::collections::HashMap;
use mddb::{Lint, LintData, StaleRef};

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

            // Group by updated file (to_path) using &str keys to avoid cloning
            let mut by_updated: HashMap<&str, Vec<&&mddb::Diagnostic>> = HashMap::new();
            for d in lint_diags {
                by_updated.entry(d.to_path.as_str()).or_default().push(d);
            }

            for (updated_file, deps) in &by_updated {
                // Extract version info from LintData::StaleRef
                let current_version = match &deps[0].data {
                    LintData::StaleRef { current_version, .. } => *current_version,
                };
                println!("📄 {} (version {})", updated_file, current_version);
                println!("   └─ Referenced by:");
                for dep in deps {
                    let pinned_version = match &dep.data {
                        LintData::StaleRef { pinned_version, .. } => *pinned_version,
                    };
                    println!("      • {} (pinned at version {})", dep.from_path, pinned_version);
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
    let project = mddb::MDDBProject::new(".")?;
    project.refresh()?;
    
    let n = if all {
        mddb::approve_edits(project.get_conn(), &[])?
    } else {
        mddb::approve_edits(project.get_conn(), paths)?
    };
    
    println!("Approved {} link(s).", n);
    Ok(())
}
