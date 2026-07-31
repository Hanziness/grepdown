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