use grepdown_lib::GrepdownProject;

#[test]
fn duplicate_broken_targets_do_not_violate_pk() {
    let dir = tempfile::tempdir().unwrap();
    // NOTE: the "" broken target requires no index.md at the project root
    // (resolve_in_set falls back to <target>/index.md).
    std::fs::write(
        dir.path().join("doc.md"),
        "[a](missing.md#x)\n[b](missing.md#y)\n[c](#frag)\n",
    )
    .unwrap();

    let project = GrepdownProject::new(dir.path()).unwrap();
    project.refresh().unwrap();

    let conn = project.get_conn();
    let mut stmt = conn
        .prepare("SELECT raw_target FROM broken_links ORDER BY raw_target")
        .unwrap();
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
    let mut targets: Vec<String> = rows.map(|r| r.unwrap()).collect();
    targets.sort();
    targets.dedup();
    assert_eq!(targets, vec!["", "missing.md"]);
}
