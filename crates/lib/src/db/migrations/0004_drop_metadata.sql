DROP TABLE IF EXISTS metadata;
INSERT INTO schema_migrations (version) VALUES (4) ON CONFLICT(version) DO NOTHING;
