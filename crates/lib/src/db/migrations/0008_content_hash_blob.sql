-- Convert content_hash from TEXT (hex) to BLOB (raw bytes).
-- blake3 produces 32 bytes; hex encoding was 64 chars.
-- This halves storage per row (32 bytes vs 64).

CREATE TABLE documents_new (
    path TEXT PRIMARY KEY,
    content_hash BLOB NOT NULL,
    mtime INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 1
);

INSERT INTO documents_new
    SELECT path, unhex(content_hash), mtime, version
    FROM documents;

DROP TABLE documents;

ALTER TABLE documents_new RENAME TO documents;

INSERT INTO schema_migrations (version) VALUES (8) ON CONFLICT(version) DO NOTHING;
