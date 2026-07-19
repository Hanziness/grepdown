use anyhow::{Context, Result};

pub fn search(
    query: &str,
    limit: usize,
    no_refresh: bool,
    literal: bool,
    json: bool,
    path: Option<&str>,
) -> Result<()> {
    let project = grepdown_lib::MDDBProject::open(".").context("Failed to open project")?;

    if !no_refresh {
        log::info!("Refreshing index...");
        project.refresh().context("Failed to refresh index")?;
    }

    let effective_query = if literal {
        grepdown_lib::escape_fts5_query(query)
    } else {
        query.to_string()
    };

    let resolved_path = match path {
        Some(p) => {
            let full = std::path::Path::new(project.get_root()).join(p);
            Some(full.to_string_lossy().into_owned())
        }
        None => None,
    };

    let results = match project.search(&effective_query, limit, resolved_path.as_deref()) {
        Ok(r) => r,
        Err(e) => {
            let err: anyhow::Error = e.into();
            if !literal && is_fts5_syntax_error(&err) {
                let escaped = grepdown_lib::escape_fts5_query(query);
                match project.search(&escaped, limit, resolved_path.as_deref()) {
                    Ok(r) => {
                        eprintln!(
                            "Note: original query was not valid FTS5 syntax; treated as literal. \
                             Use `--literal` to suppress this message."
                        );
                        r
                    }
                    Err(_) => return Err(friendly_fts5_error(query, err)),
                }
            } else if is_fts5_syntax_error(&err) {
                return Err(friendly_fts5_error(query, err));
            } else {
                return Err(err.context("Search failed"));
            }
        }
    };

    if results.is_empty() {
        println!("No results found.");
        return Ok(());
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&results).context("Failed to pretty-print results")?
        )
    } else {
        for result in results {
            // Strip HTML tags and apply ANSI bold
            let snippet = result
                .snippet
                .replace("<b>", "\x1b[1m")
                .replace("</b>", "\x1b[0m");

            println!("\x1b[1;32m{}\x1b[0m", result.path);
            println!("  {}", snippet);
            println!();
        }
    }

    Ok(())
}

fn is_fts5_syntax_error(e: &anyhow::Error) -> bool {
    e.chain().any(|c| {
        let s = c.to_string();
        s.contains("fts5: syntax error")
            || s == "unterminated string"
            || s.starts_with("fts5:") && s.contains("syntax")
    })
}

fn friendly_fts5_error(query: &str, original: anyhow::Error) -> anyhow::Error {
    let escaped = grepdown_lib::escape_fts5_query(query);
    anyhow::anyhow!(
        "Invalid FTS5 search query: {:?}\n  Cause: {}\n\n\
         FTS5 reserved syntax:\n  \
         - `-term` = NOT, `+term` = required, `\"phrase\"` = exact phrase, `prefix*` = suffix wildcard\n  \
         - Keywords: AND, OR, NOT, NEAR (case-insensitive)\n\n\
         To search for this exact string, rerun with `--literal` or use the quoted form: {}.",
        query,
        original,
        escaped,
    )
}
