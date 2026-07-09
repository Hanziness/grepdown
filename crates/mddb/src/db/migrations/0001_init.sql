PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
-- PRAGMA mmap_size = 268435456; -- 256MB, tune per corpus size

-- Track schema version so future migrations (0002_*.sql, ...) know where to start
CREATE TABLE IF NOT EXISTS schema_migrations (
    version     INTEGER PRIMARY KEY,
    applied_at  INTEGER NOT NULL DEFAULT (unixepoch())
);


CREATE TABLE IF NOT EXISTS documents (
    path TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    mtime INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_documents_mtime ON documents(mtime);

CREATE TABLE IF NOT EXISTS metadata (
    document_id TEXT NOT NULL REFERENCES documents(path) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT,
    PRIMARY KEY (document_id, key)
);
CREATE INDEX IF NOT EXISTS idx_metadata_key_value ON metadata(key, value);

-- Freestanding FTS5 table (NOT external-content, since there's no `content`
-- column on `documents` to shadow). The app writes to this directly at
-- index time by reading the file, extracting body text, and inserting here.
CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(
    path,
    body,
    tokenize = 'porter unicode61'
);

INSERT INTO schema_migrations (version) VALUES (1) ON CONFLICT(version) DO NOTHING;