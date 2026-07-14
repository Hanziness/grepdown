use rusqlite::Connection;
use crate::error::Result;

pub enum Severity {
    Error,
    Warning,
}

pub enum LintData {
    StaleRef {
        pinned_version: i64,
        current_version: i64,
    },
}

pub struct Diagnostic {
    pub lint_id: &'static str,
    pub severity: Severity,
    pub from_path: String,
    pub to_path: String,
    pub data: LintData,
}

pub trait Lint {
    fn id(&self) -> &'static str;
    fn title(&self) -> &'static str;
    fn suggestions(&self) -> &'static str;
    fn check(&self, conn: &Connection) -> Result<Vec<Diagnostic>>;
}

pub struct StaleRef;

impl Lint for StaleRef {
    fn id(&self) -> &'static str {
        "stale-ref"
    }

    fn title(&self) -> &'static str {
        "STALE REFERENCES DETECTED"
    }

    fn suggestions(&self) -> &'static str {
        "💡 Suggested actions:\n    1. Update them if needed\n    2. Run `grepdown approve-edits <filenames>` to mark them as reviewed"
    }

    fn check(&self, conn: &Connection) -> Result<Vec<Diagnostic>> {
        let mut stmt = conn.prepare(
            "SELECT l.from_id, l.to_id, l.pinned_version, d.version
             FROM links l
             JOIN documents d ON l.to_id = d.path
             WHERE l.pinned_version < d.version"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Diagnostic {
                lint_id: self.id(),
                severity: Severity::Warning,
                from_path: row.get(0)?,
                to_path: row.get(1)?,
                data: LintData::StaleRef {
                    pinned_version: row.get(2)?,
                    current_version: row.get(3)?,
                },
            })
        })?;

        let mut diags = Vec::new();
        for row in rows {
            diags.push(row?);
        }
        Ok(diags)
    }
}

pub fn run_lints(conn: &Connection) -> Result<Vec<Diagnostic>> {
    let lints: &[&dyn Lint] = &[&StaleRef];
    let mut all = Vec::new();
    for lint in lints {
        all.extend(lint.check(conn)?);
    }
    Ok(all)
}

pub fn approve_edits(conn: &Connection, paths: &[String]) -> Result<usize> {
    let rows = if paths.is_empty() {
        // Approve all stale references using CTE to avoid redundant subqueries
        conn.execute(
            "WITH stale AS (
                SELECT l.rowid as link_rowid, d.version as current_version
                FROM links l
                JOIN documents d ON l.to_id = d.path
                WHERE l.pinned_version < d.version
            )
            UPDATE links SET pinned_version = (SELECT current_version FROM stale WHERE stale.link_rowid = links.rowid)
            WHERE rowid IN (SELECT link_rowid FROM stale)",
            []
        )?
    } else {
        // Approve only links TO the specified paths
        let placeholders: Vec<String> = paths.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
        let sql = format!(
            "WITH stale AS (
                SELECT l.rowid as link_rowid, d.version as current_version
                FROM links l
                JOIN documents d ON l.to_id = d.path
                WHERE l.pinned_version < d.version
                AND l.to_id IN ({})
            )
            UPDATE links SET pinned_version = (SELECT current_version FROM stale WHERE stale.link_rowid = links.rowid)
            WHERE rowid IN (SELECT link_rowid FROM stale)",
            placeholders.join(", ")
        );
        let params: Vec<&dyn rusqlite::ToSql> = paths.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
        conn.execute(&sql, params.as_slice())?
    };
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::bootstrap;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        bootstrap(&conn).unwrap();
        conn
    }

    fn insert_test_document(conn: &Connection, path: &str, version: i64) {
        conn.execute(
            "INSERT INTO documents (path, mtime, content_hash, version) VALUES (?1, 0, X'00', ?2)",
            rusqlite::params![path, version],
        ).unwrap();
    }

    fn insert_test_link(conn: &Connection, from_path: &str, to_path: &str, pinned_version: i64) {
        conn.execute(
            "INSERT INTO links (from_id, to_id, pinned_version) VALUES (?1, ?2, ?3)",
            rusqlite::params![from_path, to_path, pinned_version],
        ).unwrap();
    }

    #[test]
    fn test_stale_ref_detection() {
        let conn = setup_test_db();
        insert_test_document(&conn, "/a.md", 1);
        insert_test_document(&conn, "/b.md", 2);
        insert_test_link(&conn, "/a.md", "/b.md", 1);

        let lint = StaleRef;
        let diags = lint.check(&conn).unwrap();

        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].from_path, "/a.md");
        assert_eq!(diags[0].to_path, "/b.md");
        match &diags[0].data {
            LintData::StaleRef { pinned_version, current_version } => {
                assert_eq!(*pinned_version, 1);
                assert_eq!(*current_version, 2);
            }
        }
    }

    #[test]
    fn test_no_stale_refs_when_up_to_date() {
        let conn = setup_test_db();
        insert_test_document(&conn, "/a.md", 1);
        insert_test_document(&conn, "/b.md", 2);
        insert_test_link(&conn, "/a.md", "/b.md", 2);

        let lint = StaleRef;
        let diags = lint.check(&conn).unwrap();

        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_approve_edits_all() {
        let conn = setup_test_db();
        insert_test_document(&conn, "/a.md", 1);
        insert_test_document(&conn, "/b.md", 2);
        insert_test_link(&conn, "/a.md", "/b.md", 1);

        let rows = approve_edits(&conn, &[]).unwrap();
        assert_eq!(rows, 1);

        // Verify the link was updated
        let pinned: i64 = conn.query_row(
            "SELECT pinned_version FROM links WHERE from_id = '/a.md'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(pinned, 2);
    }

    #[test]
    fn test_approve_edits_specific_path() {
        let conn = setup_test_db();
        insert_test_document(&conn, "/a.md", 1);
        insert_test_document(&conn, "/b.md", 2);
        insert_test_document(&conn, "/c.md", 3);
        insert_test_link(&conn, "/a.md", "/b.md", 1);
        insert_test_link(&conn, "/a.md", "/c.md", 1);

        let paths = vec!["/b.md".to_string()];
        let rows = approve_edits(&conn, &paths).unwrap();
        assert_eq!(rows, 1);

        // Verify only the link to /b.md was updated
        let pinned_b: i64 = conn.query_row(
            "SELECT pinned_version FROM links WHERE to_id = '/b.md'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(pinned_b, 2);

        let pinned_c: i64 = conn.query_row(
            "SELECT pinned_version FROM links WHERE to_id = '/c.md'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(pinned_c, 1);
    }
}
