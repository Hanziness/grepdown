use anyhow::{Context, Result};

pub fn reach(doc: &str, depth: i64, json: bool, json_pretty: bool) -> Result<()> {
    let project = grepdown_lib::MDDBProject::open(".").context("Failed to open project")?;

    let nodes = project
        .get_reachable(doc, depth)
        .context("Failed to compute reachable documents")?;

    if nodes.is_empty() {
        println!("No reachable documents from {} within {} hops.", doc, depth);
        return Ok(());
    }

    if json || json_pretty {
        let output = if json_pretty {
            serde_json::to_string_pretty(&nodes)?
        } else {
            serde_json::to_string(&nodes)?
        };
        println!("{}", output);
        return Ok(());
    }

    println!("Reachable from {} (max depth {}):\n", doc, depth);
    for node in &nodes {
        let indent = "  ".repeat(node.depth as usize);
        println!("{}[depth {}] {}", indent, node.depth, node.path);
    }
    println!("\n{} document(s) reachable.", nodes.len());
    Ok(())
}
