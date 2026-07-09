pub fn init() {
    println!("Hey hoo");
    let res = mddb::MDDBProject::new(".".to_string());

    match res {
        Ok(project) => {
            println!("Folder: {}", project.get_root());
            print!("Starting indexing... ");
            project.refresh().unwrap();
            println!("success!")
        },
        Err(_) => todo!(),
    }

    println!("We're done here for now.");
}