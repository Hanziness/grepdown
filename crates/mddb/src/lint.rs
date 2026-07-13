use rusqlite::Connection;
use crate::error::Result;

pub enum Severity {
    Error,
    Warning,
}

pub struct Diagnostic {
    pub lint_id: &'static str,
    pub severity: Severity,
    pub from_path: String,
    pub to_path: String,
    pub message: String,
}

pub struct LintContext<'a> {
    conn: &'a Connection,
}

pub trait Lint {
    fn id(&self) -> &'static str;
    fn check(&self, ctx: &LintContext) -> Result<Vec<Diagnostic>>;
}

pub struct StaleRef;

impl Lint for StaleRef {
    fn id(&self) -> &'static str {
        "stale-ref"
    }

    fn check(&self, ctx: &LintContext) -> Result<Vec<Diagnostic>> {
        let mut stmt = ctx.conn.prepare(
            "SELECT l.from_id, l.to_id, l.pinned_version, d.version
             FROM links l
             JOIN documents d ON l.to_id = d.path
             WHERE l.pinned_version < d.version"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Diagnostic {
                lint_id: "stale-ref",
                severity: Severity::Warning,
                from_path: row.get(0)?,
                to_path: row.get(1)?,
                message: format!(
                    "pinned version {} is behind current version {}",
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?
                ),
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
    let ctx = LintContext { conn };
    let lints: Vec<Box<dyn Lint>> = vec![Box::new(StaleRef)];
    let mut all = Vec::new();
    for lint in lints {
        all.extend(lint.check(&ctx)?);
    }
    Ok(all)
}
