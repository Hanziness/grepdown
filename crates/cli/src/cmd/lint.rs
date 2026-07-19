use anyhow::Result;
use std::collections::HashMap;
use grepdown_lib::{Lint, LintId, StaleRef, Orphan};

pub fn lint(json: bool) -> Result<()> {
    let project = grepdown_lib::MDDBProject::open(".")?;
    project.refresh()?;
    let diags = grepdown_lib::run_lints(project.get_conn())?;

    if json {
        println!("{}", serde_json::to_string_pretty(&diags)?);
        std::process::exit(if diags.is_empty() { 0 } else { 1 });
    }

    if diags.is_empty() {
        println!("No lint issues found.");
        std::process::exit(0);
    }

    // Build lint registry
    let lints: Vec<Box<dyn Lint>> = vec![Box::new(StaleRef), Box::new(Orphan)];
    let lint_map: HashMap<LintId, &dyn Lint> = lints.iter().map(|l| (l.id(), l.as_ref())).collect();

    // Group by lint_id
    let mut by_lint: HashMap<LintId, Vec<&grepdown_lib::Diagnostic>> = HashMap::new();
    for d in &diags {
        by_lint.entry(d.lint_id).or_default().push(d);
    }

    // Print each lint's output
    for (lint_id, lint_diags) in &by_lint {
        if let Some(lint) = lint_map.get(lint_id) {
            println!("⚠️  {}\n", lint.title());
            println!("{}", lint.format_group(lint_diags));
            println!("{}\n", lint.suggestions());
        }
    }

    println!("{} issue(s) found.", diags.len());
    std::process::exit(1);
}

pub fn approve(all: bool, paths: &[String]) -> Result<()> {
    let project = grepdown_lib::MDDBProject::open(".")?;
    project.refresh()?;
    
    let n = if all {
        grepdown_lib::approve_edits(project.get_conn(), &[])?
    } else {
        grepdown_lib::approve_edits(project.get_conn(), paths)?
    };
    
    println!("Approved {} link(s).", n);
    Ok(())
}
