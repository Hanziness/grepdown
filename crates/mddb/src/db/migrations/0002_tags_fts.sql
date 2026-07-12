CREATE VIRTUAL TABLE IF NOT EXISTS tags_fts USING fts5(
    path UNINDEXED,
    tags,
    tokenize = 'porter unicode61'
);

INSERT INTO schema_migrations (version) VALUES (2) ON CONFLICT(version) DO NOTHING;
