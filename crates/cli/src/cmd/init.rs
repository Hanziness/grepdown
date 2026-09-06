pub fn init() {
    let res = grepdown_lib::GrepdownProject::new(".");

    match res {
        Ok(project) => {
            log::info!("Project: {}", project.get_root());
            log::info!("Starting indexing...");
            project.refresh().unwrap();
            log::info!("Indexing complete");
        }
        Err(e) => {
            eprintln!("Error: {:#}", e);
            std::process::exit(1);
        }
    }
}
