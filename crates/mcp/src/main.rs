use clap::Parser;
use grepdown_lib::MDDBProject;
use rmcp::ServiceExt;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::io::{stdin, stdout};

mod mcp;
#[cfg(test)] mod tests;
mod watch;

#[derive(Parser)]
#[command(name = "grepdown-mcp", about = "MCP server for grepdown")]
struct Args {
    /// Watch for file changes and auto-refresh the index
    #[arg(long)]
    watch: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let project = Arc::new(Mutex::new(MDDBProject::open(".")?));
    
    // Start file watcher if --watch flag is set
    if args.watch {
        let project_clone = Arc::clone(&project);
        tokio::spawn(async move {
            if let Err(e) = watch::start_watch(project_clone).await {
                eprintln!("Watcher error: {}", e);
            }
        });
        eprintln!("File watching enabled. Auto-refreshing on .md file changes.");
    }
    
    let handler = mcp::GrepdownMCP::new(Arc::clone(&project));

    let transport = (stdin(), stdout());
    let service = handler.serve(transport).await?;
    service.waiting().await?;
    Ok(())
}