-- ponytail: broken links stored separately to keep FK integrity clean.
-- Unresolvable internal links are captured here during indexing.

CREATE TABLE IF NOT EXISTS broken_links (
    from_id    TEXT NOT NULL REFERENCES documents(path) ON DELETE CASCADE,
    raw_target TEXT NOT NULL,
    PRIMARY KEY (from_id, raw_target)
);

CREATE INDEX IF NOT EXISTS idx_broken_links_from ON broken_links(from_id);

INSERT INTO schema_migrations (version) VALUES (6) ON CONFLICT(version) DO NOTHING;
