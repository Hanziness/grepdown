# Link Graph Design for mddb

## 1. Schema

```sql
CREATE TABLE links (
    from_id   TEXT NOT NULL REFERENCES documents(path) ON DELETE CASCADE,
    to_id     TEXT NOT NULL REFERENCES documents(path) ON DELETE CASCADE,
    link_type TEXT NOT NULL CHECK(link_type IN ('cross-ref', 'citation')),
    raw_target TEXT,          -- unresolved string for debugging
    PRIMARY KEY (from_id, to_id, link_type)
);

CREATE INDEX idx_links_from ON links(from_id, link_type);
CREATE INDEX idx_links_to   ON links(to_id,   link_type);
```

**Why adjacency list:** SQLite has no native graph type. Adjacency list is the simplest structure that supports recursive CTEs (BFS/DFS) and bidirectional traversal with two covering indexes. At 10k nodes / 100k edges this is trivial for SQLite.

**Why FK to `documents(path)`:** Consistent with existing schema (`metadata` already FKs to `path`). CASCADE DELETE removes links automatically when a document is deleted or re-indexed.

**Why `raw_target`:** Stores the unresolved link string (e.g. `tables/users`). Useful for debugging broken links and for re-resolution if the bundle structure changes.

---

## 2. Link Resolution (Index-Time)

**Rule: resolve at index time, never at query time.**

When parsing a markdown file, extract links and resolve them immediately:

```rust
fn resolve_link(base_path: &str, target: &str, root: &Path) -> Option<String> {
    // 1. If target starts with http:// or https:// -> citation, skip graph
    if target.starts_with("http://") || target.starts_with("https://") {
        return None; // stored as citation with to_id = URL
    }

    // 2. Resolve relative to base_path's directory
    let base_dir = Path::new(base_path).parent()?;
    let mut resolved = base_dir.join(target);

    // 3. Try exact .md file
    let with_md = resolved.with_extension("md");
    if with_md.exists() {
        return Some(with_md.to_string_lossy().into_owned());
    }

    // 4. Try index.md inside directory
    resolved.push("index.md");
    if resolved.exists() {
        return Some(resolved.to_string_lossy().into_owned());
    }

    // 5. Broken link — store raw_target but leave to_id = raw_target
    //    (or skip, depending on whether you want dangling edges)
    None
}
```

**Why index-time:** Query-time resolution would require filesystem access or complex SQL for every traversal step. Index-time makes the graph a pure relational structure.

**Citations:** For external URLs, `to_id` stores the URL directly. Since URLs don't reference `documents(path)`, we have two options:

- **Option A (recommended):** Store citations in `links` with `to_id = URL` and **remove the FK constraint on `to_id`**. This is the simplest, but loses CASCADE for citations.
- **Option B:** Keep FK, add a `citations` table for URLs, and link to it. More normalized but overkill for URLs that never need reverse traversal.

If you want to keep the FK (my recommendation for cross-refs), handle citations separately:

```sql
CREATE TABLE citations (
    from_id TEXT NOT NULL REFERENCES documents(path) ON DELETE CASCADE,
    url     TEXT NOT NULL,
    PRIMARY KEY (from_id, url)
);
CREATE INDEX idx_citations_from ON citations(from_id);
```

---

## 3. Traversal Queries

### Forward: what does X link to?
```sql
SELECT to_id, link_type, raw_target
FROM links
WHERE from_id = ? AND link_type = 'cross-ref';
```

### Reverse: what links to X?
```sql
SELECT from_id, raw_target
FROM links
WHERE to_id = ? AND link_type = 'cross-ref';
```

### Multi-hop BFS (recursive CTE)
```sql
WITH RECURSIVE bfs AS (
    -- anchor: start node
    SELECT to_id AS node, 1 AS depth
    FROM links
    WHERE from_id = ? AND link_type = 'cross-ref'

    UNION ALL

    -- recursive step
    SELECT l.to_id, bfs.depth + 1
    FROM links l
    JOIN bfs ON l.from_id = bfs.node
    WHERE l.link_type = 'cross-ref'
      AND bfs.depth < ?        -- max depth param
)
SELECT DISTINCT node, MIN(depth) AS depth
FROM bfs
GROUP BY node
ORDER BY depth;
```

### Reverse multi-hop (what links to X, transitively?)
```sql
WITH RECURSIVE reverse_bfs AS (
    SELECT from_id AS node, 1 AS depth
    FROM links
    WHERE to_id = ? AND link_type = 'cross-ref'

    UNION ALL

    SELECT l.from_id, reverse_bfs.depth + 1
    FROM links l
    JOIN reverse_bfs ON l.to_id = reverse_bfs.node
    WHERE l.link_type = 'cross-ref'
      AND reverse_bfs.depth < ?
)
SELECT DISTINCT node, MIN(depth) AS depth
FROM reverse_bfs
GROUP BY node
ORDER BY depth;
```

### Cycle detection (for validation)
```sql
WITH RECURSIVE path AS (
    SELECT from_id, to_id, 1 AS depth, CAST(from_id AS TEXT) AS path_str
    FROM links
    WHERE from_id = ?

    UNION ALL

    SELECT p.from_id, l.to_id, p.depth + 1,
           p.path_str || ' -> ' || l.to_id
    FROM links l
    JOIN path p ON l.from_id = p.to_id
    WHERE l.link_type = 'cross-ref'
      AND p.depth < 10
      AND p.path_str NOT LIKE '%' || l.to_id || '%'  -- crude cycle guard
)
SELECT * FROM path WHERE from_id = to_id;  -- cycles
```

---

## 4. CTEs vs Rust Iteration

| Approach | Pros | Cons | When to use |
|----------|------|------|-------------|
| **Recursive CTE** | Single query, SQLite optimizes joins, simple code | Hard to inject mid-traversal logic, cycle detection is awkward | Default for all traversal |
| **Rust iteration** | Full control over traversal order, easy cycle detection, can apply filters per-hop | N+1 queries, more code | When you need custom stop conditions or complex per-node logic |

**Recommendation:** Use recursive CTEs for 95% of cases. They are efficient in SQLite up to thousands of hops. Switch to Rust iteration only if you need:
- Custom pruning rules per node
- Parallel traversal
- Very large result sets where streaming matters

---

## 5. Performance Considerations

### At 10k nodes / 100k edges:
- Single-hop queries: **sub-millisecond** (indexed adjacency list)
- 3-hop BFS CTE: **< 10ms** on modern hardware
- SQLite handles this easily; the bottleneck is parsing markdown, not graph queries

### Scaling beyond 100k edges:
1. **Add `link_type` to indexes** (already done above) — filters half the table for citation queries
2. **Consider materialized transitive closure** if you run 5+ hop queries constantly:
   ```sql
   CREATE TABLE link_closure (
       from_id TEXT, to_id TEXT, depth INTEGER,
       PRIMARY KEY (from_id, to_id, depth)
   );
   ```
   Rebuild on index refresh. Only do this if profiling shows CTEs are slow.
3. **WAL mode** is already enabled — good for concurrent reads during indexing

### Index refresh strategy:
During `refresh()`, after detecting changed files:
```sql
-- In the same transaction as FTS updates:
DELETE FROM links WHERE from_id IN (changed_paths);
DELETE FROM citations WHERE from_id IN (changed_paths);
-- Then re-insert resolved links for changed files
```
This keeps the graph consistent with the FTS index in a single transaction.

---

## 6. Migration Integration

Add to `crates/mddb/src/db/init.rs`:
```rust
const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init",      include_str!("migrations/0001_init.sql")),
    ("0002_tags_fts",    include_str!("migrations/0002_tags_fts.sql")),
    ("0003_link_graph",  include_str!("migrations/0003_link_graph.sql")), // <-- new
];
```

---

## 7. Open Decisions

1. **Broken links:** Store as `to_id = raw_target` (no FK) or skip them? Skipping keeps the graph clean; storing them lets you query for "all broken links."
2. **Citations table:** Do you need reverse traversal of citations ("who cited this URL?")? If yes, keep the separate `citations` table. If no, store citations in `links` with `to_id = URL` and drop the FK.
3. **Link extraction:** Will you parse `[[wiki-links]]`, standard `[text](path)` markdown links, or both? This affects the regex/parser in `db/parse.rs`.

**Lazy default:** Start with cross-refs only in `links`, citations in a separate minimal table, resolve at index time, use recursive CTEs for traversal. Add complexity only when a concrete use case demands it.
