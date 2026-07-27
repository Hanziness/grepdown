use crate::mcp::{GrepdownMCP, SearchParams, DocIdParams, ApproveEditsParams};
use grepdown_lib::MDDBProject;
use rmcp::handler::server::wrapper::Parameters;
use serde_json;
use tempfile;
use tokio;

fn test_mcp() -> GrepdownMCP {
    let tempdir = tempfile::tempdir().unwrap();
    let root = tempdir.path();

    std::fs::write(
        root.join("doc1.md"),
        r#"---
tags: [rust, testing]
---
# Document 1
This is about Rust programming.
[Link to doc2](doc2.md)
[Example](https://example.com)
"#,
    )
    .unwrap();

    std::fs::write(
        root.join("doc2.md"),
        r#"---
tags: [python]
---
# Document 2
Python documentation.
[Back to doc1](doc1.md)
"#,
    )
    .unwrap();

    std::fs::write(root.join("doc3.md"), "# Document 3\nStandalone document with no links.\n")
        .unwrap();

    let project = MDDBProject::new(root).unwrap();
    project.refresh().unwrap();
    GrepdownMCP::new(project)
}

#[test]
fn test_all_tools_registered() {
    let router = GrepdownMCP::tool_router();
    let tools = router.list_all();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert_eq!(tools.len(), 6);
    for name in &["search", "refresh", "lint", "approve_edits", "get_links", "get_citations"] {
        assert!(names.contains(name), "missing tool: {}", name);
    }
}

#[tokio::test]
async fn test_search_finds_documents() {
    let mcp = test_mcp();
    let result = mcp
        .search(Parameters(SearchParams {
            query: "Rust".into(),
            limit: Some(10),
            path_filter: None,
        }))
        .await
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(parsed.as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn test_search_with_path_filter() {
    let mcp = test_mcp();
    let result = mcp
        .search(Parameters(SearchParams {
            query: "document".into(),
            limit: Some(10),
            path_filter: Some("doc1".into()),
        }))
        .await
        .unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
    assert!(parsed.iter().all(|r| r["path"].as_str().unwrap().contains("doc1")));
}

#[tokio::test]
async fn test_refresh_indexes_new_files() {
    let mcp = test_mcp();
    let result = mcp.refresh().await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(parsed.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_lint_clean_project() {
    let mcp = test_mcp();
    let result = mcp.lint().await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(parsed.is_array());
}

#[tokio::test]
async fn test_get_links_bidirectional() {
    let mcp = test_mcp();
    let result = mcp
        .get_links(Parameters(DocIdParams {
            doc_id: "doc1.md".into(),
        }))
        .await
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(parsed.get("outgoing").is_some());
    assert!(parsed.get("incoming").is_some());
    let outgoing = parsed["outgoing"].as_array().unwrap();
    assert!(outgoing.len() > 0, "doc1 should have outgoing links");
}

#[tokio::test]
async fn test_get_citations_extracts_urls() {
    let mcp = test_mcp();
    let result = mcp
        .get_citations(Parameters(DocIdParams {
            doc_id: "doc1.md".into(),
        }))
        .await
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let citations = parsed.as_array().unwrap();
    assert!(citations.iter().any(|c| c.as_str().unwrap().contains("example.com")));
}

#[tokio::test]
async fn test_approve_edits_returns_count() {
    let mcp = test_mcp();
    let result = mcp
        .approve_edits(Parameters(ApproveEditsParams {
            paths: None,
        }))
        .await
        .unwrap();
    let count: u64 = serde_json::from_str(&result).unwrap();
    // ponytail: u64 is always >= 0, this assertion checks deserialization succeeded
    let _ = count;
}

#[tokio::test]
async fn test_search_via_mcp_protocol() -> anyhow::Result<()> {
    use rmcp::{
        ClientHandler, ServiceExt,
        model::{CallToolRequestParams, ClientInfo},
    };

    #[derive(Debug, Clone, Default)]
    struct TestClient;
    impl ClientHandler for TestClient {
        fn get_info(&self) -> ClientInfo {
            ClientInfo::default()
        }
    }

    let tempdir = tempfile::tempdir().unwrap();
    std::fs::write(tempdir.path().join("test.md"), "# Test\nHello world\n").unwrap();
    let project = MDDBProject::new(tempdir.path()).unwrap();
    project.refresh().unwrap();

    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server_handle = tokio::spawn(async move {
        GrepdownMCP::new(project)
            .serve(server_transport)
            .await?
            .waiting()
            .await?;
        anyhow::Ok(())
    });

    let client = TestClient.serve(client_transport).await?;

    let result = client
        .call_tool(
            CallToolRequestParams::new("search")
                .with_arguments(serde_json::json!({"query": "Hello"}).as_object().unwrap().clone()),
        )
        .await?;

    assert!(!result.content.is_empty());
    client.cancel().await?;
    server_handle.await??;
    Ok(())
}
