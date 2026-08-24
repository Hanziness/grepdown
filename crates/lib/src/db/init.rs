use crate::error::Result;
use rusqlite::Connection;

const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init", include_str!("migrations/0001_init.sql")),
    (
        "0002_tags_fts",
        include_str!("migrations/0002_tags_fts.sql"),
    ),
    (
        "0003_link_graph",
        include_str!("migrations/0003_link_graph.sql"),
    ),
    (
        "0004_drop_metadata",
        include_str!("migrations/0004_drop_metadata.sql"),
    ),
    (
        "0005_lint_versioning",
        include_str!("migrations/0005_lint_versioning.sql"),
    ),
    (
        "0006_broken_links",
        include_str!("migrations/0006_broken_links.sql"),
    ),
    (
        "0007_drop_link_type",
        include_str!("migrations/0007_drop_link_type.sql"),
    ),
    (
        "0008_content_hash_blob",
        include_str!("migrations/0008_content_hash_blob.sql"),
    ),
    (
        "0009_headings",
        include_str!("migrations/0009_headings.sql"),
    ),
    (
        "0010_add_anchor_to_links",
        include_str!("migrations/0010_add_anchor_to_links.sql"),
    ),
];

pub fn bootstrap(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL DEFAULT (unixepoch()));")?;
    let current: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )?;

    for (i, (_name, sql)) in MIGRATIONS.iter().enumerate() {
        let version = (i + 1) as i64;
        if version > current {
            conn.execute_batch(sql)?;
            log::info!("Applied migration: {}", _name);
        }
    }

    Ok(())
}
