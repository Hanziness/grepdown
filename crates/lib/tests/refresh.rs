use grepdown_lib::GrepdownProject;

#[test]
fn duplicate_broken_targets_do_not_violate_pk() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("doc.md"),
        "[a](missing.md#x)\n[b](missing.md#y)\n[c](#frag)\n",
    )
    .unwrap();

    let project = GrepdownProject::new(dir.path()).unwrap();
    project.refresh().unwrap();

    // same missing target via 2 anchors collapses to 1 row -> in-page anchor (#frag) skipped
    assert_eq!(broken_targets(&project, "doc.md"), vec!["missing.md"]);
}

#[test]
fn in_page_anchors_and_schemeless_urls_are_not_broken_links() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("doc.md"),
        "[c](#frag)\n[x](www.example.com/a#p1)\n[y](www.example.com/a#p2)\n",
    )
    .unwrap();

    let project = GrepdownProject::new(dir.path()).unwrap();
    // would violate the citations PK before www. was treated as external
    project.refresh().unwrap();

    assert_eq!(broken_targets(&project, "doc.md"), Vec::<String>::new());
    let count: i64 = project
        .get_conn()
        .query_row(
            "SELECT COUNT(*) FROM citations WHERE from_id = 'doc.md'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    // both anchors dedupe to one citation row for www.example.com/a
    assert_eq!(count, 1);
}

#[test]
fn broken_links_revalidate_when_targets_appear_or_disappear() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("doc.md"), "[x](missing.md)\n").unwrap();

    let project = GrepdownProject::new(dir.path()).unwrap();
    project.refresh().unwrap();
    assert_eq!(broken_targets(&project, "doc.md"), vec!["missing.md"]);
    assert_eq!(link_targets(&project, "doc.md"), Vec::<String>::new());

    // target appears -> broken row resolves into a links row
    std::fs::write(dir.path().join("missing.md"), "target\n").unwrap();
    project.refresh().unwrap();
    assert_eq!(broken_targets(&project, "doc.md"), Vec::<String>::new());
    assert_eq!(link_targets(&project, "doc.md"), vec!["missing.md"]);

    // target disappears -> links row cascades away, broken row regenerates
    std::fs::remove_file(dir.path().join("missing.md")).unwrap();
    project.refresh().unwrap();
    assert_eq!(broken_targets(&project, "doc.md"), vec!["missing.md"]);
    assert_eq!(link_targets(&project, "doc.md"), Vec::<String>::new());
}

fn broken_targets(project: &GrepdownProject, from: &str) -> Vec<String> {
    query_col(
        project.get_conn(),
        "SELECT raw_target FROM broken_links WHERE from_id = ?1 ORDER BY raw_target",
        from,
    )
}

fn link_targets(project: &GrepdownProject, from: &str) -> Vec<String> {
    query_col(
        project.get_conn(),
        "SELECT to_id FROM links WHERE from_id = ?1 ORDER BY to_id",
        from,
    )
}

fn query_col(conn: &rusqlite::Connection, sql: &str, param: &str) -> Vec<String> {
    let mut stmt = conn.prepare(sql).unwrap();
    stmt.query_map([param], |r| r.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}
