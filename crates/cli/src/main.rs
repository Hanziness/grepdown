use clap::{Parser, Subcommand};

mod cmd;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize mddb in the folder
    Init {

    },

    /// Search the folder for the given query string
    Search {
        /// String to search in the database
        query: String,
    },

    /// Explicitly index the folder
    Index {}
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init {  } => {
            println!("You wanted to initialize mddb in this repository");
            cmd::init::init();
        },
        Commands::Search { query } => {
            println!("You wanted to search for {}", query)
        },
        Commands::Index { } => {
            println!("You wanted to index the folder")
        }
    }

    println!("Hello world!")
}
