use rusqlite::Connection;
use serde::Serialize;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum LintId {
    StaleRef,
    Orphan,
}

impl LintId {
    pub fn as_str(&self) -> &'static str {
        match self {
            LintId::StaleRef => "stale-ref",
            LintId::Orphan => "orphan",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum LintData {
    StaleRef {
        pinned_version: i64,
        current_version: i64,
    },
    Orphan,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Diagnostic {
    pub lint_id: LintId,
    pub severity: Severity,
    pub from_path: String,
    pub to_path: String,
    pub data: LintData,
}

pub trait Lint {
    fn id(&self) -> LintId;
    fn title(&self) -> &'static str;
    fn suggestions(&self) -> &'static str;
    fn check(&self, conn: &Connection) -> Result<Vec<Diagnostic>>;
    fn format_group(&self, diags: &[&Diagnostic]) -> String;
}

pub struct StaleRef;

impl Lint for StaleRef {
    fn id(&self) -> LintId {
        LintId::StaleRef
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

    fn format_group(&self, diags: &[&Diagnostic]) -> String {
        let mut out = String::new();
        out.push_str("The following files were updated, but their dependents may need review:\n\n");

        // Group by updated file (to_path)
        let mut by_updated: std::collections::HashMap<&str, Vec<&&Diagnostic>> = std::collections::HashMap::new();
        for d in diags {
            by_updated.entry(d.to_path.as_str()).or_default().push(d);
        }

        for (updated_file, deps) in &by_updated {
            let current_version = match &deps[0].data {
                LintData::StaleRef { current_version, .. } => *current_version,
                _ => unreachable!(),
            };
            out.push_str(&format!("📄 {} (version {})\n", updated_file, current_version));
            out.push_str("   └─ Referenced by:\n");
            for dep in deps {
                let pinned_version = match &dep.data {
                    LintData::StaleRef { pinned_version, .. } => *pinned_version,
                    _ => unreachable!(),
                };
                out.push_str(&format!("      • {} (pinned at version {})\n", dep.from_path, pinned_version));
            }
            out.push('\n');
        }

        out
    }
}

pub struct Orphan;

impl Lint for Orphan {
    fn id(&self) -> LintId {
        LintId::Orphan
    }

    fn title(&self) -> &'static str {
        "ORPHAN DOCUMENTS DETECTED"
    }

    fn suggestions(&self) -> &'static str {
        "💡 These documents have no links. Consider:\n   \
         1. Adding links to related documents\n   \
         2. Linking from other documents to these\n   \
         3. Deleting if they're no longer needed"
    }

    fn check(&self, conn: &Connection) -> Result<Vec<Diagnostic>> {
        let mut stmt = conn.prepare(
            "SELECT d.path
             FROM documents d
             WHERE NOT EXISTS (SELECT 1 FROM links l WHERE l.from_id = d.path)
               AND NOT EXISTS (SELECT 1 FROM links l WHERE l.to_id = d.path)"
        )?;

        let rows = stmt.query_map([], |row| {
            let path: String = row.get(0)?;
            Ok(Diagnostic {
                lint_id: LintId::Orphan,
                severity: Severity::Warning,
                from_path: path.clone(),
                to_path: path,
                data: LintData::Orphan,
            })
        })?;

        let mut diags = Vec::new();
        for row in rows {
            diags.push(row?);
        }
        Ok(diags)
    }

    fn format_group(&self, diags: &[&Diagnostic]) -> String {
        let mut out = String::new();
        for d in diags {
            out.push_str(&format!("  - {}\n", d.from_path));
        }
        out
    }
}

pub fn run_lints(conn: &Connection) -> Result<Vec<Diagnostic>> {
    let lints: &[&dyn Lint] = &[&StaleRef, &Orphan];
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
            _ => unreachable!(),
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

    #[test]
    fn orphan_detection() {
        let conn = setup_test_db();
        insert_test_document(&conn, "orphan.md", 1);
        insert_test_document(&conn, "another-orphan.md", 1);

        let diags = run_lints(&conn).unwrap();
        // Filter only orphan diagnostics
        let orphan_diags: Vec<_> = diags.iter().filter(|d| d.lint_id == LintId::Orphan).collect();
        assert_eq!(orphan_diags.len(), 2);
        let mut paths: Vec<&str> = orphan_diags.iter().map(|d| d.from_path.as_str()).collect();
        paths.sort();
        assert_eq!(paths, vec!["another-orphan.md", "orphan.md"]);
        match &orphan_diags[0].data {
            LintData::Orphan => {}
            _ => panic!("expected Orphan data"),
        }
    }

    #[test]
    fn non_orphan_with_outgoing_link() {
        let conn = setup_test_db();
        insert_test_document(&conn, "doc-a.md", 1);
        insert_test_document(&conn, "doc-b.md", 1);
        insert_test_link(&conn, "doc-a.md", "doc-b.md", 1);

        let diags = run_lints(&conn).unwrap();
        let orphan_diags: Vec<_> = diags.iter().filter(|d| d.lint_id == LintId::Orphan).collect();
        assert_eq!(orphan_diags.len(), 0);
    }

    #[test]
    fn non_orphan_with_incoming_link() {
        let conn = setup_test_db();
        insert_test_document(&conn, "source.md", 1);
        insert_test_document(&conn, "target.md", 1);
        insert_test_link(&conn, "source.md", "target.md", 1);

        let diags = run_lints(&conn).unwrap();
        let orphan_diags: Vec<_> = diags.iter().filter(|d| d.lint_id == LintId::Orphan).collect();
        assert_eq!(orphan_diags.len(), 0);
    }
}
