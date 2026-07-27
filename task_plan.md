# Task Plan: Grepdown Capability Expansion

## Goal
Systematically implement new capabilities across four areas (Search, Lints, Graph, Output) to make Grepdown more useful for both humans and AI agents, prioritized by value-to-effort ratio.

## Current Phase
Quick Wins — COMPLETE

## Phases

### Quick Wins — High-Value, Low-Complexity (Do First)

These give the most value for the least effort across all areas. They should be tackled in this order before moving to area-specific deeper work.

- [x] **QW-1: Expose `get_reachable` in CLI + MCP** — Already implemented in lib, dead code from user perspective. Wire up `grepdown reach <doc> --depth N` and a corresponding MCP tool. *(Graph, Trivial)*
- [x] **QW-2: Document content retrieval** — `grepdown read <path>` / MCP `get_document` tool. Completes the search → read loop for MCP clients. *(Output, Trivial)*
- [x] **QW-3: Broken links lint** — Links whose `resolve_link()` returned `None` are silently dropped. Store unresolved links and lint them. Most obvious KB health issue, currently invisible. *(Lints, Low)*
- [x] **QW-4: Search result deduplication** — UNION ALL returns same doc twice (body + tags). Wrap in dedup logic, prefer better snippet, boost score for matching both. *(Search, Low)*
- [x] **QW-5: Compact JSON default for MCP** — CLI `--json` should default to compact (not pretty), add `--json-pretty` for humans. *(Output, Trivial)*
- [x] **QW-6: Configurable snippet window** — `--snippet-length N` to widen context around matches. Current hardcoded 32 tokens is too narrow for complex docs. *(Search, Trivial)*

### Area 1: Search Improvements

Ordered by value: heading context > trigram fuzzy > related docs > tag boosting > natural language > spellfix.

- [ ] **S-1: Heading-aware snippets** — Extract heading hierarchy during parse, annotate snippets with nearest enclosing heading (e.g., `"heading": "## Config > Port"`). *"Where in the doc?" is the #1 follow-up question.*
- [ ] **S-2: Trigram tokenizer for fuzzy matching** — Add second FTS5 table with `tokenize='trigram'`. Query: `search "errr" --fuzzy` falls back to trigram. Real fuzzy search is the most requested search feature.
- [ ] **S-3: "More like this" / related docs** — Extract terms from a document, run as FTS query to find similar docs. Useful for exploration, especially for LLMs building context.
- [ ] **S-4: Tag boosting** — Tag matches should carry more weight than body matches in BM25 score. Multiply tag score by a factor.
- [ ] **S-5: Natural language query mode** — `--natural` flag: auto-add implicit AND, handle quoted phrases, translate common patterns. Makes FTS5 syntax invisible for casual users.
- [ ] **S-6: `spellfix1` for "did you mean?"** — SQLite extension for spelling suggestions when zero results. Nice-to-have (LLMs don't need it).

### Area 2: Lint Expansions

Ordered by value: missing frontmatter > empty docs > fan-in/out hubs > circular links > duplicate tags > stale index.

- [ ] **L-1: Missing frontmatter lint** — Documents without YAML frontmatter at all. Enforces metadata standards, especially for OKF compliance.
- [ ] **L-2: Empty/tiny documents lint** — Documents below a word-count threshold.
- [ ] **L-3: High fan-in / high fan-out hub detection** — Documents with abnormally many links (too broad) or too many dependents (single point of failure).
- [ ] **L-4: Circular link detection** — Documents A → B → A cycles. Potential merge candidates or confusing navigation.
- [ ] **L-5: Duplicate/similar tag detection** — Tags differing only in casing (`api` vs `API`), hyphenation (`frontend` vs `front-end`), or pluralization.
- [ ] **L-6: Stale index warning** — Warn when DB was last refreshed significantly before the newest file mtime.

### Area 3: Graph-based Document Linking

Ordered by value: Mermaid export > shortest path > neighborhood > centrality > link suggestions > graph stats.

- [ ] **G-1: Graph export (Mermaid/DOT)** — `grepdown graph --format mermaid`. Both humans (paste into docs) and LLMs (render) can use it.
- [ ] **G-2: Shortest path between docs** — `grepdown path <from> <to>` finds the link chain. "How does concept A relate to concept B?"
- [ ] **G-3: Neighborhood view** — `grepdown neighborhood <doc>` returns subgraph around a document (N hops) including inter-connections between neighbors.
- [ ] **G-4: Document centrality / PageRank** — Rank documents by importance (in-degree, PageRank). Useful for KB curation.
- [ ] **G-5: Link suggestions** — Based on shared tags or shared terms, suggest links that don't yet exist. Proactive KB growth via MCP.
- [ ] **G-6: Graph stats summary** — `grepdown stats`: total docs, links, connected components, average degree, most-linked docs.

### Area 4: Output Format Improvements

Ordered by value: include tags in results > Markdown output > result metadata > heading context (covered by S-1).

- [ ] **O-1: Include frontmatter/tags in search results** — JOIN with tags_fts or store tags on documents. Gives LLMs richer context per result without a second query.
- [ ] **O-2: Markdown output mode** — `--format md` renders results as Markdown list. Useful for embedding search results into other Markdown docs or LLM prompts.
- [ ] **O-3: Result metadata** — Show total match count, query time, index freshness. Helps LLMs decide whether to broaden/narrow their query.

---

## Key Questions

1. Should broken links be stored in a new `broken_links` table, or as a `resolve_status` column on `links`? (Lean: separate table, keeps FK integrity clean.)
2. Should the trigram FTS5 table be a separate column-indexed table, or share the `documents_fts` table with a different tokenizer? (Lean: separate table — can't have two tokenizers on one FTS5 virtual table.)
3. Should `get_reachable` results include the link path (breadcrumbs), or just `(node, depth)`?
4. Should Mermaid graph export be the full graph, or support `--from <doc>` subgraph export?
5. Should heading extraction store all headings, or just H1-H3?

## Decisions Made

| Decision | Rationale |
|----------|-----------|
| Quick Wins phase comes first | Highest ROI, gets momentum, validates the planning process |
| Broken links → separate table | Keeps FK integrity clean, consistent with `citations` table pattern |
| Heading-aware snippets before fuzzy search | Heading context benefits every search; fuzzy search is a nice-to-have |
| Tags in search results over Markdown output | LLMs are the primary consumers; tags give immediate filtering context |

## Errors Encountered

| Error | Attempt | Resolution |
|-------|---------|------------|
| *(none yet)* | | |

## Notes

- Re-read this plan before starting each phase
- Each item references a code file/line for context: see `findings.md` for the full architecture map
- Phase ordering within each area is intentional — do not shuffle without justification
