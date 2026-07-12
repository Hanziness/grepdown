-- ponytail: link graph stored as adjacency list.
-- Resolution happens at index time so traversal is a pure graph query.

CREATE TABLE IF NOT EXISTS links (
    from_id  TEXT NOT NULL REFERENCES documents(path) ON DELETE CASCADE,
    to_id    TEXT NOT NULL REFERENCES documents(path) ON DELETE CASCADE,
    -- 'cross-ref' = bundle-relative markdown link
    -- 'citation'  = external URL
    link_type TEXT NOT NULL CHECK(link_type IN ('cross-ref', 'citation')),
    -- unresolved target string for debugging / re-resolution
    raw_target TEXT,
    PRIMARY KEY (from_id, to_id, link_type)
);

-- forward traversal: what does X link to?
CREATE INDEX IF NOT EXISTS idx_links_from ON links(from_id, link_type);

-- reverse traversal: what links to X?
CREATE INDEX IF NOT EXISTS idx_links_to   ON links(to_id,   link_type);

INSERT INTO schema_migrations (version) VALUES (3) ON CONFLICT(version) DO NOTHING;
