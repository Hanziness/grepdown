use grepdown_lib::MDDBProject;
use rmcp::ServiceExt;
use tokio::io::{stdin, stdout};

mod mcp;
#[cfg(test)] mod tests;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let project = MDDBProject::open(".")?;
    let handler = mcp::GrepdownMCP::new(project);

    let transport = (stdin(), stdout());
    let service = handler.serve(transport).await?;
    service.waiting().await?;
    Ok(())
}

/*
The Grepdown MCP server is an all-in-one knowledege management service:

1. Start it in a folder where the knowledgebase is
    * It keeps the DB in memory, so operations are even faster
2. Manage the knowledge base remotely (e.g., edit files via the `patch` tool)
*/