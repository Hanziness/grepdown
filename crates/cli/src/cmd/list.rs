use anyhow::{Context, Result};
use clap::Args;

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Output results as compact JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(serde::Serialize)]
struct ListItem {
    path: String,
    version: i64,
    mtime: i64,
    tag_count: i64,
    link_count: i64,
}

pub fn execute(args: ListArgs) -> Result<()> {
    let project = grepdown_lib::MDDBProject::open(".").context("Failed to open project")?;
    let conn = project.get_conn();
    
    let mut stmt = conn.prepare(
        "SELECT d.path, d.version, d.mtime,
                (SELECT COUNT(*) FROM tags_fts WHERE path = d.path) as tag_count,
                (SELECT COUNT(*) FROM links WHERE from_id = d.path) as link_count
         FROM documents d
         ORDER BY d.path"
    )?;
    
    let rows = stmt.query_map([], |row| {
        Ok(ListItem {
            path: row.get(0)?,
            version: row.get(1)?,
            mtime: row.get(2)?,
            tag_count: row.get(3)?,
            link_count: row.get(4)?,
        })
    })?;
    
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    
    if args.json {
        println!("{}", serde_json::to_string(&items)?);
    } else {
        println!("Indexed documents ({}):", items.len());
        println!("{:<50} {:>7} {:>10} {:>5}", "Path", "Version", "Mtime", "Links");
        println!("{}", "-".repeat(75));
        for item in items {
            println!("{:<50} {:>7} {:>10} {:>5}", 
                     item.path, item.version, item.mtime, item.link_count);
        }
    }
    
    Ok(())
}
