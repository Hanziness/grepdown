use clap::{Parser, Subcommand};

mod cmd;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Verbosity level (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
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

    let log_level = match cli.verbose {
        0 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        2 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };
    env_logger::Builder::new()
        .filter_level(log_level)
        .parse_default_env()
        .init();

    match &cli.command {
        Commands::Init {  } => {
            log::debug!("Initializing mddb");
            cmd::init::init();
        },
        Commands::Search { query } => {
            log::debug!("Searching for: {}", query)
        },
        Commands::Index { } => {
            log::debug!("Indexing folder")
        }
    }
}
