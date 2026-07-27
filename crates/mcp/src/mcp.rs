use std::sync::Arc;
use rmcp::{ServerHandler, handler::server::wrapper::Parameters, tool, tool_handler, tool_router};
use grepdown_lib::{MDDBProject, Link, ReachableNode, DocumentContent, run_lints, approve_edits};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct GrepdownMCP {
    project: Arc<Mutex<MDDBProject>>,
}

impl GrepdownMCP {
    pub fn new(project: MDDBProject) -> Self {
        Self {
            project: Arc::new(Mutex::new(project)),
        }
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchParams {
    /// Full-text search query to match against document content
    #[schemars(description = "Full-text search query to match against document content")]
    pub query: String,
    /// Maximum number of results to return (default: 20)
    #[schemars(description = "Maximum number of results to return (default: 20)")]
    pub limit: Option<usize>,
    /// Restrict results to documents whose path contains this substring
    #[schemars(description = "Restrict results to documents whose path contains this substring")]
    pub path_filter: Option<String>,
    /// Number of tokens to include in search snippets (default: 32)
    #[schemars(description = "Number of tokens to include in search snippets (default: 32)")]
    pub snippet_length: Option<i64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DocIdParams {
    /// Unique document identifier (typically the file path relative to the project root)
    #[schemars(description = "Unique document identifier (typically the file path relative to the project root)")]
    pub doc_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ApproveEditsParams {
    /// Only approve stale references in these paths. Empty or omitted means approve all.
    #[schemars(description = "Only approve stale references in these paths. Empty or omitted means approve all.")]
    pub paths: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ReachableParams {
    /// Starting document identifier (typically the file path relative to the project root)
    #[schemars(description = "Starting document identifier (typically the file path relative to the project root)")]
    pub doc_id: String,
    /// Maximum hop depth for BFS traversal (default: 2)
    #[schemars(description = "Maximum hop depth for BFS traversal (default: 2)")]
    pub max_depth: Option<i64>,
}

#[tool_router(vis = "pub")]
impl GrepdownMCP {
    #[tool(
        description = "Search the knowledge base using full-text search. Returns matching documents ranked by relevance. Call `refresh` first if files may have been added or modified since the last index.",
        annotations(title = "Search documents", read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    pub async fn search(&self, Parameters(SearchParams { query, limit, path_filter, snippet_length }): Parameters<SearchParams>) -> Result<String, String> {
        let project = self.project.lock().await;
        let results = project.search(&query, limit.unwrap_or(20), path_filter.as_deref(), snippet_length)
            .map_err(|e| e.to_string())?;

        serde_json::to_string(&results)
            .map_err(|e| format!("failed to serialize: {e}"))
    }

    #[tool(
        description = "Re-index the project. Scans for new, modified, or deleted files and updates the search index. Returns changed file paths and their new mtimes. Call this before `search` when files may have changed.",
        annotations(title = "Refresh index", read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    pub async fn refresh(&self) -> Result<String, String> {
        let project = self.project.lock().await;
        let result = project.refresh()
            .map_err(|e| e.to_string())?;

        serde_json::to_string(&result)
            .map_err(|e| format!("serialization error: {e}"))
    }

    #[tool(
        description = "Run knowledge base lints and return diagnostics. Detects issues such as broken links, stale references, and orphaned documents. Use this to audit KB health.",
        annotations(title = "Lint knowledge base", read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    pub async fn lint(&self) -> Result<String, String> {
        let project = self.project.lock().await;
        let conn = project.get_conn();
        let result = run_lints(conn)
            .map_err(|e| e.to_string())?;

        serde_json::to_string(&result)
            .map_err(|e| format!("serialization error: {e}"))
    }

    #[tool(
        description = "Approve stale references in the knowledge base. Marks outdated link targets as accepted. When `paths` is omitted or empty, approves all stale references. When `paths` is provided, only approves stale references in those specific documents. Use after reviewing lint diagnostics.",
        annotations(title = "Approve stale references", read_only_hint = false, destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    pub async fn approve_edits(&self, Parameters(ApproveEditsParams { paths }): Parameters<ApproveEditsParams>) -> Result<String, String> {
        let project = self.project.lock().await;
        let conn = project.get_conn();
        let count = approve_edits(conn, &paths.unwrap_or_default())
            .map_err(|e| e.to_string())?;

        serde_json::to_string(&count)
            .map_err(|e| format!("serialization error: {e}"))
    }

    #[tool(
        description = "Get outgoing links (documents this one links to) and incoming links (backlinks: documents linking to this one) for a given document. Useful for understanding document relationships and navigation.",
        annotations(title = "Get document links", read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    pub async fn get_links(&self, Parameters(DocIdParams { doc_id }): Parameters<DocIdParams>) -> Result<String, String> {
        let project = self.project.lock().await;
        let outgoing = project.get_links_from(&doc_id)
            .map_err(|e| e.to_string())?;
        let incoming = project.get_links_to(&doc_id)
            .map_err(|e| e.to_string())?;

        #[derive(serde::Serialize)]
        struct LinkResult {
            outgoing: Vec<Link>,
            incoming: Vec<Link>,
        }

        serde_json::to_string(&LinkResult { outgoing, incoming })
            .map_err(|e| format!("serialization error: {e}"))
    }

    #[tool(
        description = "Get external URLs (HTTP/HTTPS citations) referenced in a document. Useful for finding source material or verifying references.",
        annotations(title = "Get document citations", read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    pub async fn get_citations(&self, Parameters(DocIdParams { doc_id }): Parameters<DocIdParams>) -> Result<String, String> {
        let project = self.project.lock().await;
        let result = project.get_citations_from(&doc_id)
            .map_err(|e| e.to_string())?;

        serde_json::to_string(&result)
            .map_err(|e| format!("serialization error: {e}"))
    }

    #[tool(
        description = "Get all documents reachable from a starting document via the link graph using BFS traversal. Returns each reachable document with its minimum hop depth. Useful for exploring knowledge neighborhoods and understanding document clusters.",
        annotations(title = "Get reachable documents", read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    pub async fn get_reachable(&self, Parameters(ReachableParams { doc_id, max_depth }): Parameters<ReachableParams>) -> Result<String, String> {
        let project = self.project.lock().await;
        let result: Vec<ReachableNode> = project.get_reachable(&doc_id, max_depth.unwrap_or(2))
            .map_err(|e| e.to_string())?;

        serde_json::to_string(&result)
            .map_err(|e| format!("serialization error: {e}"))
    }

    #[tool(
        description = "Read the full content of a document and its frontmatter tags. Use after `search` to retrieve the complete text of a matching document. The doc_id is the document path relative to the project root.",
        annotations(title = "Read document content", read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    pub async fn get_document(&self, Parameters(DocIdParams { doc_id }): Parameters<DocIdParams>) -> Result<String, String> {
        let project = self.project.lock().await;
        let result: DocumentContent = project.read_document(&doc_id)
            .map_err(|e| e.to_string())?;

        serde_json::to_string(&result)
            .map_err(|e| format!("serialization error: {e}"))
    }
}

#[tool_handler(
    name = "grepdown",
    version = "0.1.0",
    instructions = "Markdown knowledge base management. Typical workflow: 1) Call `refresh` to sync the index with the filesystem. 2) Use `search` for full-text queries. 3) Use `get_document` to read the full content and tags of a specific document. 4) Use `get_links` to explore document relationships (forward links and backlinks). 5) Use `get_citations` to find external URLs in a document. 6) Use `get_reachable` to explore knowledge neighborhoods via BFS traversal of the link graph. 7) Use `lint` to audit KB health and find stale/broken references. 8) Use `approve_edits` to mark stale references as reviewed after fixing them. All write operations (`refresh`, `approve_edits`) are safe and idempotent."
)]
impl ServerHandler for GrepdownMCP {}


