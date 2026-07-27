# grepdown

**Knowledge base management for Markdown — search, link, and lint.**

**grepdown** turns a folder of Markdown files into a queryable knowledge base. It indexes your `.md` files and lets you search, traverse link graphs, and detect stale references. Ships as a CLI and as an [MCP server](#mcp-server) for AI agent integration — zero config, fast incremental indexing, and out-of-the-box [Open Knowledge Format (OKF)](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md) support.

## Features

- **CLI + MCP**. Use from the terminal or via an [MCP server](#mcp-server) for AI agent integration
- **Full-text search**. SQLite FTS5 with BM25 ranking and phrase/prefix/NEAR queries
- **Incremental indexing**. Only re-processes changed files (mtime + blake3 content hashing)
- **Parallel processing**. Reads and parses files concurrently
- **Tag search**. Extracts YAML frontmatter tags and searches them alongside body text
- **Link graph**. Forward links, backlinks, citations, and BFS reachability traversal
- **Linting**. Detects stale references, orphaned documents, and broken links
- **JSON output**. Machine-readable output for scripting and programmatic use
- **Highlighted snippets**. ANSI-colored context snippets with match highlighting
- **Zero config**. Just run it in any directory of Markdown files

## Installation

```bash
# From source (requires Rust toolchain)
git clone https://github.com/Hanziness/grepdown && cd grepdown
cargo build --release

# Install both binaries
cargo install --path crates/cli
cargo install --path crates/mcp

# or manually:
cp target/release/grepdown ~/.local/bin/
cp target/release/grepdown-mcp ~/.local/bin/
```

No system SQLite required — it's already bundled.

## Quick Start

```bash
# Initialize the project (creates the index)
grepdown init

# Search
grepdown search "deployment guide"

# Search with a result limit
grepdown search "error handling" --limit 10

# Skip re-indexing before search (faster if index is fresh)
grepdown search "async runtime" --no-refresh

# Literal search (no FTS5 operators)
grepdown search "C++ error handling" --literal
```

## Commands

| Command | Description |
|---------|-------------|
| `init` | Initialize and index all `.md` files in the current directory |
| `index` | Re-index (refresh) the database |
| `search <query>` | Search indexed files (auto-refreshes by default) |
| `lint` | Run lints on the knowledge base (exit 0 = clean, 1 = issues found) |
| `approve-edits` | Approve stale references by updating pinned versions |
| `reach <doc>` | Show documents reachable from a given document via the link graph |
| `read <path>` | Read a document's content from the knowledge base |

### Search Options

| Flag | Default | Description |
|------|---------|-------------|
| `-l`, `--limit <N>` | `20` | Maximum number of results |
| `--no-refresh` | `false` | Skip index refresh before searching |
| `--literal` | `false` | Treat the query as a literal string (no FTS5 operators) |
| `--json` | `false` | Output results as compact JSON |
| `--path <SUBPATH>` | — | Filter results to files under this subfolder |
| `--snippet-length <N>` | `32` | Number of tokens in search snippets |

### Lint Options

| Flag | Description |
|------|-------------|
| `--json` | Output lint results as compact JSON |

### Approve Edits

Requires `--all` or explicit paths:

| Flag | Description |
|------|-------------|
| `--all` | Approve all stale references |
| `<paths...>` | Approve stale references only in the specified files/folders |

### Global Flags

| Flag | Description |
|------|-------------|
| `-v` / `-vv` / `-vvv` | Verbosity: Warn → Info → Debug → Trace |

## Search Syntax

grepdown uses the SQLite FTS5 query syntax:

```
deployment guide          # all words must match
deploy OR guide           # either word
"exact phrase"            # phrase search
config*                   # prefix match
NEAR(server client, 5)    # words within 5 tokens
```

Use `--literal` to search for a plain string without FTS5 operators.

## How It Works

1. **Walk** — recursively finds all `.md` files
2. **Diff** — compares mtimes and content hashes to skip unchanged files
3. **Parse** — extracts body text, YAML frontmatter tags, and internal/external links
4. **Index** — bulk-inserts into SQLite FTS5 in a single transaction
5. **Search** — queries both body and tag indexes, returns BM25-ranked results with snippets

The database lives at `md.db` in your project root (gitignored by convention).

## Linting

grepdown runs three types of lints:

- **Stale references** — when a linked document changes, the link is flagged until you approve it
- **Orphaned documents** — documents with no incoming links from other documents
- **Broken links** — links that point to non-existent documents

### Example workflow

```bash
# Initialize
grepdown init

# Check for lint issues
grepdown lint
# Output: No lint issues found.

# Edit a document that other documents link to
echo "# Updated content" >> doc-b.md

# Re-index to detect changes
grepdown index

# Check for stale references
grepdown lint
# Output:
# WARNING: pinned version 1 is behind current version 2 (/path/to/doc-a.md → /path/to/doc-b.md)
#
# 1 issue(s) found.

# Review the changes, then approve all stale references
grepdown approve-edits --all
# Output: Approved 1 link(s).

# Verify no more issues
grepdown lint
# Output: No lint issues found.
```

Each document has a `version` counter that increments when its content changes. Links capture the target's version at link time; a link is stale when `pinned_version < current_version`.

## MCP Server

grepdown ships an [MCP (Model Context Protocol)](https://modelcontextprotocol.io/) server (`grepdown-mcp`) that exposes the knowledge base to AI agents over stdio.

### Tools

| Tool | Description |
|------|-------------|
| `search` | Full-text search with optional path filter and snippet length |
| `refresh` | Re-index the project |
| `lint` | Run all lints and return diagnostics |
| `approve_edits` | Approve stale references (optionally scoped to specific paths) |
| `get_links` | Get outgoing links and backlinks for a document |
| `get_citations` | Get external URLs referenced in a document |
| `get_reachable` | BFS traversal of the link graph from a starting document |
| `get_document` | Read the full content and frontmatter of a document |

### Usage

```bash
# Start the MCP server in a grepdown project directory
grepdown-mcp
```

Configure your MCP client to use `grepdown-mcp` as a stdio transport:

```json
{
  "mcpServers": {
    "grepdown": {
      "command": "grepdown-mcp",
      "args": []
    }
  }
}
```

## License

See [LICENSE](LICENSE.md) for details.
