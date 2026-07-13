# mddb

**Instant full-text search across thousands of Markdown files.**

mddb indexes your `.md` files into a local SQLite FTS5 database and lets you query them with rich search syntax. Built for humans and AI agents alike — zero config, fast incremental indexing, and link graph traversal out of the box.

Supports the [Open Knowledge Format (OKF)](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md) through frontmatter tags and link graphs.

## Features

- **AI-compatible**. Easy-to-use CLI for both humans and AI agents to search knowledge bases
- **Full-text search**. SQLite FTS5 with Porter stemming, BM25 ranking, and phrase/prefix/NEAR queries
- **Incremental indexing**. Only re-processes changed files (mtime + blake3 content hashing)
- **Parallel processing**. Reads and parses files concurrently
- **Tag search**. Extracts YAML frontmatter tags and searches them alongside body text
- **Link graph**. Forward links, backlinks, citations, and BFS reachability traversal
- **Linting**. Detects stale references when linked documents change, with approval workflow
- **Highlighted snippets**. ANSI-colored context snippets with match highlighting
- **Zero config**. Just run it in any directory of Markdown files

## Installation

```bash
# From source (requires Rust toolchain)
git clone https://github.com/Hanziness/mddb && cd mddb
cargo build --release

# Install the binary
cargo install --path .
# or manually:
cp target/release/mddb-cli ~/.local/bin/
```

No system SQLite required — it's already bundled.

## Quick Start

```bash
# Index and search in one step (auto-initializes on first run)
mddb-cli search "deployment guide"

# Explicitly (re-)index the current directory
mddb-cli init

# Search with a result limit
mddb-cli search "error handling" --limit 10

# Skip re-indexing before search (faster if index is fresh)
mddb-cli search "async runtime" --no-refresh
```

## Commands

| Command | Description |
|---------|-------------|
| `init` | Initialize and index all `.md` files in the current directory |
| `index` | Re-index (refresh) the database |
| `search <query>` | Search indexed files (auto-refreshes by default) |
| `lint` | Run lints on the knowledge base (exit 0 = clean, 1 = issues found) |
| `approve-edits` | Approve all stale references by updating pinned versions |

### Search Options

| Flag | Default | Description |
|------|---------|-------------|
| `-l`, `--limit <N>` | `20` | Maximum number of results |
| `--no-refresh` | `false` | Skip index refresh before searching |

### Global Flags

| Flag | Description |
|------|-------------|
| `-v` / `-vv` / `-vvv` | Verbosity: Warn → Info → Debug → Trace |

## Search Syntax

mddb uses SQLite FTS5 query syntax:

```
deployment guide          # all words must match
deploy OR guide           # either word
"exact phrase"            # phrase search
config*                   # prefix match
NEAR(server client, 5)    # words within 5 tokens
```

## How It Works

1. **Walk** — recursively finds all `.md` files
2. **Diff** — compares mtimes and content hashes to skip unchanged files
3. **Parse** — extracts body text, YAML frontmatter tags, and internal/external links
4. **Index** — bulk-inserts into SQLite FTS5 in a single transaction
5. **Search** — queries both body and tag indexes, returns BM25-ranked results with snippets

The database lives at `md.db` in your project root (gitignored by convention).

## OKF Support

mddb supports the Open Knowledge Format by:

- Parsing `tags` arrays from YAML frontmatter
- Building a link graph from internal Markdown links
- Enabling graph traversal (forward links, backlinks, reachability)

This turns a folder of Markdown files into a queryable knowledge base.

## Linting

mddb tracks document versions and detects when links become stale. When you link from document A to document B, mddb pins the version of B at link time. If B changes later, the link is flagged as stale until you explicitly approve it.

### Example workflow

```bash
# Initial setup - index your knowledge base
mddb-cli index

# Check for lint issues (stale references)
mddb-cli lint
# Output: No lint issues found.

# Edit a document that other documents link to
echo "# Updated content" >> doc-b.md

# Re-index to detect changes
mddb-cli index

# Check for stale references
mddb-cli lint
# Output:
# WARNING: pinned version 1 is behind current version 2 (/path/to/doc-a.md → /path/to/doc-b.md)
#
# 1 issue(s) found.

# Review the changes in doc-b.md, then approve all stale references
mddb-cli approve-edits
# Output: Approved 1 link(s).

# Verify no more issues
mddb-cli lint
# Output: No lint issues found.
```

### How it works

- Each document has a `version` counter that increments when its content changes
- When a link is created, it captures the target's current version as `pinned_version`
- A link is stale when `pinned_version < current_version`
- `approve-edits` updates all pinned versions to match current versions

This is useful for knowledge bases where you want to track when referenced content has changed and needs review.

## License

See [LICENSE](LICENSE.md) for details.
