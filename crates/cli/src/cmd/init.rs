pub fn init() {
    let res = grepdown_lib::MDDBProject::new(".".to_string());

    match res {
        Ok(project) => {
            log::info!("Project: {}", project.get_root());
            log::info!("Starting indexing...");
            project.refresh().unwrap();
            log::info!("Indexing complete");
        },
        Err(_) => todo!(),
    }
}