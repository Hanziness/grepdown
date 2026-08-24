-- ponytail: link graph stored as adjacency list.
-- Resolution happens at index time so traversal is a pure graph query.

CREATE TABLE IF NOT EXISTS links (
    from_id  TEXT NOT NULL REFERENCES documents(path) ON DELETE CASCADE,
    to_id    TEXT NOT NULL REFERENCES documents(path) ON DELETE CASCADE,
    link_type TEXT NOT NULL DEFAULT 'cross-ref',
    -- unresolved target string for debugging / re-resolution
    raw_target TEXT,
    PRIMARY KEY (from_id, to_id)
);

-- forward traversal: what does X link to?
CREATE INDEX IF NOT EXISTS idx_links_from ON links(from_id);

-- reverse traversal: what links to X?
CREATE INDEX IF NOT EXISTS idx_links_to   ON links(to_id);

-- Citations (external URLs) in separate table to keep FK integrity clean
CREATE TABLE IF NOT EXISTS citations (
    from_id    TEXT NOT NULL REFERENCES documents(path) ON DELETE CASCADE,
    url        TEXT NOT NULL,
    raw_target TEXT,
    PRIMARY KEY (from_id, url)
);

CREATE INDEX IF NOT EXISTS idx_citations_from ON citations(from_id);

INSERT INTO schema_migrations (version) VALUES (3) ON CONFLICT(version) DO NOTHING;
