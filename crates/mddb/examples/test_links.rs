use mddb::MDDBProject;

fn main() {
    let project = MDDBProject::new("/tmp/okf_test").expect("open project");
    project.refresh().expect("refresh");
    
    println!("=== Links from /tmp/okf_test/index.md ===");
    for link in project.get_links_from("/tmp/okf_test/index.md").unwrap() {
        println!("  -> {} ({})", link.target, link.link_type);
    }
    
    println!("\n=== Links from /tmp/okf_test/tables/users.md ===");
    for link in project.get_links_from("/tmp/okf_test/tables/users.md").unwrap() {
        println!("  -> {} ({}) [raw: {:?}]", link.target, link.link_type, link.raw_target);
    }
    
    println!("\n=== Citations from /tmp/okf_test/tables/users.md ===");
    for citation in project.get_citations_from("/tmp/okf_test/tables/users.md").unwrap() {
        println!("  -> {}", citation);
    }
    
    println!("\n=== Backlinks to /tmp/okf_test/datasets/sales.md ===");
    for link in project.get_links_to("/tmp/okf_test/datasets/sales.md").unwrap() {
        println!("  <- {} ({})", link.target, link.link_type);
    }
    
    println!("\n=== Reachable from /tmp/okf_test/index.md (depth 2) ===");
    for node in project.get_reachable("/tmp/okf_test/index.md", 2).unwrap() {
        println!("  depth {}: {}", node.depth, node.path);
    }
}
