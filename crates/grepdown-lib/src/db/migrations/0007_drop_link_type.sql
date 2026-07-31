-- Drop the unused link_type column. Always defaulted to 'cross-ref',
-- never queried or inserted. Citations have their own table.
-- Can be re-added via a future migration if graph-based relationship
-- typing is needed.
ALTER TABLE links DROP COLUMN link_type;

INSERT INTO schema_migrations (version) VALUES (7) ON CONFLICT(version) DO NOTHING;
