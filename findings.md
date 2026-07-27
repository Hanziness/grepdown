# Findings & Decisions — Grepdown Capability Expansion

## Architecture Overview

Grepdown is a Rust workspace with 3 crates:

| Crate | Purpose | Key Files |
|-------|---------|-----------|
| `grepdown-lib` | Core library | `src/search.rs`, `src/lint.rs`, `src/db/parse.rs`, `src/db/init.rs`, `src/project.rs` |
| `cli` | CLI binary | `src/main.rs` (clap), `src/cmd/{search,lint,init}.rs` |
| `mcp` | MCP server (stdio) | `src/mcp.rs` (rmcp framework), `src/main.rs` |

## Database Schema

| Table | Columns | Purpose |
|-------|---------|---------|
| `documents` | path, content_hash, mtime, version | Document metadata + versioning |
| `documents_fts` | path, body | FTS5 virtual table (porter unicode61 tokenizer) |
| `tags_fts` | path (UNINDEXED), tags | FTS5 virtual table for frontmatter tags |
| `links` | from_id, to_id, link_type, raw_target, pinned_version | Internal link graph adjacency list |
| `citations` | from_id, url, raw_target | External URL references |
| `schema_migrations` | version, applied_at | Migration tracking |

**Key detail:** FTS5 tokenizer is `porter unicode61` — Porter stemmer + Unicode-aware word boundaries. Supports: phrase, prefix*, NEAR, OR, NOT.

**Search query:** UNION ALL between `documents_fts` (body) and `tags_fts` (tags), ordered by BM25 score. Currently returns duplicates when a doc matches in both.

## Current Capabilities (Inventory)

### Search (`grepdown-lib/src/search.rs`)
- FTS5 BM25 ranking with snippet extraction (32 tokens, hardcoded)
- Body + tag search via UNION ALL
- Path prefix filtering (`LIKE ?%`)
- `escape_fts5_query()` for literal search fallback
- Auto-fallback: invalid FTS5 syntax → retry as literal

### Lints (`grepdown-lib/src/lint.rs`)
- `StaleRef`: Detects version-pinned link staleness
- `Orphan`: Documents with zero in/out links
- `approve_edits()`: Bulk or selective approval of stale references
- Trait-based lint system: `Lint` trait with `id()`, `title()`, `suggestions()`, `check()`, `format_group()`

### Link Graph (`grepdown-lib/src/search.rs`)
- `get_links_from(doc_id)` — forward traversal
- `get_links_to(doc_id)` — backlinks
- `get_reachable(doc_id, max_depth)` — BFS reachability (NOT exposed in CLI or MCP)
- `get_citations_from(doc_id)` — external URLs

### Parse (`grepdown-lib/src/db/parse.rs`)
- Parallel file walking with Rayon
- Incremental indexing (mtime + blake3 hash diffing)
- Link resolution at index time (relative paths → canonical paths)
- Broken links are silently dropped (resolve_link returns None → skipped)
- Heading extraction: NOT implemented

### Frontmatter (`grepdown-lib/src/frontmatter.rs`)
- Parses YAML frontmatter (`---` delimited)
- Extracts `tags` array
- Only `tags` field is used; other frontmatter fields are ignored

### CLI (`crates/cli/src/main.rs`)
- Subcommands: init, index, search, lint, approve-edits
- Flags: --limit, --no-refresh, --literal, --json, --path, -v/-vv/-vvv

### MCP (`crates/mcp/src/mcp.rs`)
- Tools: search, refresh, lint, approve_edits, get_links, get_citations
- All tools have annotations (read_only_hint, destructive_hint, etc.)
- JSON serialization of all results

## Key File Locations for Implementation

| Feature | Files to Modify |
|---------|-----------------|
| New lint | `grepdown-lib/src/lint.rs` (add struct), `grepdown-lib/src/lib.rs` (export), `cli/src/cmd/lint.rs` (register) |
| Expose reachable | `cli/src/main.rs` (add subcommand), `cli/src/cmd.rs` (add module), `mcp/src/mcp.rs` (add tool) |
| Broken links lint | `db/parse.rs` (store broken links), `db/init.rs` (new migration), `lint.rs` (new lint struct) |
| Heading extraction | `db/parse.rs` (extract headings), `db/init.rs` (new migration), `search.rs` (annotate snippets) |
| Trigram fuzzy | `db/init.rs` (new FTS5 table migration), `search.rs` (dual-table query), `cli/src/main.rs` (--fuzzy flag) |
| Related docs | `search.rs` (add `related()` method using term extraction) |
| Tags in results | `search.rs` (JOIN with tags), `cli/src/cmd/search.rs` (output format) |
| Mermaid export | New file `cli/src/cmd/graph.rs`, or add to `search.rs` + new CLI subcommand |
| Document retrieval | `search.rs` (add `read_document()`), `cli/src/main.rs` (add subcommand), `mcp/src/mcp.rs` (add tool) |

## Migration Pattern

New migrations go in `crates/grepdown-lib/src/db/migrations/` as SQL files. Registered in `db/init.rs` via `include_str!` in the `MIGRATIONS` array. Sequential numbering: `0006_<name>.sql`.

## Lint Trait Pattern

New lints implement the `Lint` trait:
```rust
pub trait Lint {
    fn id(&self) -> LintId;
    fn title(&self) -> &'static str;
    fn suggestions(&self) -> &'static str;
    fn check(&self, conn: &Connection) -> Result<Vec<Diagnostic>>;
    fn format_group(&self, diags: &[&Diagnostic]) -> String;
}
```
Registered in `run_lints()` as `&[&dyn Lint]`.

## Resources
- FTS5 docs: https://www.sqlite.org/fts5.html
- FTS5 trigram tokenizer: https://www.sqlite.org/fts5.html#trigram_tokenizer
- FTS5 spellfix1: https://www.sqlite.org/fts5.html#the_spellfix1_extension
- rmcp (MCP framework): used in `crates/mcp/src/mcp.rs`
