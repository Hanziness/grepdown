use anyhow::{Context, Result};
use clap::Args;

#[derive(Args, Debug)]
pub struct TagsArgs {
    /// Output results as compact JSON
    #[arg(long)]
    pub json: bool,
}

pub fn execute(args: TagsArgs) -> Result<()> {
    let project = grepdown_lib::MDDBProject::open(".").context("Failed to open project")?;
    let conn = project.get_conn();
    
    // Extract all tags from tags_fts and count documents per tag
    // tags_fts stores space-separated tags in the 'tags' column
    // We need to split them and count occurrences
    let mut stmt = conn.prepare(
        "SELECT path, tags FROM tags_fts"
    )?;
    
    let mut tag_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    
    for row in rows {
        let (_path, tags_str) = row?;
        for tag in tags_str.split_whitespace() {
            *tag_counts.entry(tag.to_string()).or_insert(0) += 1;
        }
    }
    
    // Sort by count (descending), then by tag name
    let mut tags: Vec<(String, i64)> = tag_counts.into_iter().collect();
    tags.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    
    if args.json {
        println!("{}", serde_json::to_string(&tags)?);
    } else {
        println!("Tags ({} unique):", tags.len());
        println!("{:<30} {:>10}", "Tag", "Doc Count");
        println!("{}", "-".repeat(42));
        for (tag, count) in tags {
            println!("{:<30} {:>10}", tag, count);
        }
    }
    
    Ok(())
}
