-- Heading extraction for anchor resolution and heading search.
-- Structured table for fast anchor validation (linting).
-- FTS table for heading search (mirrors tag pattern).

CREATE TABLE IF NOT EXISTS headings (
    path TEXT NOT NULL REFERENCES documents(path) ON DELETE CASCADE,
    level INTEGER NOT NULL,
    text TEXT NOT NULL,
    anchor TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_headings_path ON headings(path);
CREATE INDEX IF NOT EXISTS idx_headings_anchor ON headings(path, anchor);

CREATE VIRTUAL TABLE IF NOT EXISTS headings_fts USING fts5(
    path UNINDEXED,
    headings,
    tokenize = 'porter unicode61'
);

INSERT INTO schema_migrations (version) VALUES (9) ON CONFLICT(version) DO NOTHING;
