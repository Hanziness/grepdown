use anyhow::{Context, Result};
use clap::Args;

#[derive(Args, Debug)]
pub struct StatsArgs {
    /// Output results as compact JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(serde::Serialize)]
struct Stats {
    doc_count: i64,
    link_count: i64,
    broken_link_count: i64,
    tagged_doc_count: i64,
    heading_count: i64,
}

pub fn execute(args: StatsArgs) -> Result<()> {
    let project = grepdown_lib::MDDBProject::open(".").context("Failed to open project")?;
    let conn = project.get_conn();
    
    let mut stmt = conn.prepare(
        "SELECT 
            (SELECT COUNT(*) FROM documents) as doc_count,
            (SELECT COUNT(*) FROM links) as link_count,
            (SELECT COUNT(*) FROM broken_links) as broken_link_count,
            (SELECT COUNT(DISTINCT path) FROM tags_fts) as tagged_doc_count,
            (SELECT COUNT(*) FROM headings) as heading_count"
    )?;
    
    let stats: Stats = stmt.query_row([], |row| {
        Ok(Stats {
            doc_count: row.get(0)?,
            link_count: row.get(1)?,
            broken_link_count: row.get(2)?,
            tagged_doc_count: row.get(3)?,
            heading_count: row.get(4)?,
        })
    })?;
    
    // Count orphans (documents with no incoming or outgoing links)
    let orphan_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM documents d
         WHERE NOT EXISTS (SELECT 1 FROM links WHERE from_id = d.path)
           AND NOT EXISTS (SELECT 1 FROM links WHERE to_id = d.path)",
        [],
        |row| row.get(0)
    )?;
    
    if args.json {
        let output = serde_json::json!({
            "doc_count": stats.doc_count,
            "link_count": stats.link_count,
            "broken_link_count": stats.broken_link_count,
            "orphan_count": orphan_count,
            "tagged_doc_count": stats.tagged_doc_count,
            "heading_count": stats.heading_count,
        });
        println!("{}", serde_json::to_string(&output)?);
    } else {
        println!("Knowledge Base Statistics:");
        println!("  Documents:      {}", stats.doc_count);
        println!("  Links:          {}", stats.link_count);
        println!("  Broken links:   {}", stats.broken_link_count);
        println!("  Orphan docs:    {}", orphan_count);
        println!("  Tagged docs:    {}", stats.tagged_doc_count);
        println!("  Headings:       {}", stats.heading_count);
    }
    
    Ok(())
}
