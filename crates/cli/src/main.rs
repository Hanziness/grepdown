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
        
        /// Maximum number of results to return
        #[arg(short, long, default_value = "20")]
        limit: usize,
        
        /// Skip refreshing the index before searching
        #[arg(long)]
        no_refresh: bool,
    },

    /// Explicitly index the folder
    Index {},

    /// Run lints on the knowledge base
    Lint {},

    /// Approve stale references for specific files or all files
    ApproveEdits {
        /// Approve all stale references
        #[arg(long)]
        all: bool,

        /// Specific file or folder paths to approve
        #[arg(required_unless_present = "all")]
        paths: Vec<String>,
    },
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
        Commands::Search { query, limit, no_refresh } => {
            log::debug!("Searching for: {}", query);
            if let Err(e) = cmd::search::search(query, *limit, *no_refresh) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        Commands::Index { } => {
            log::debug!("Indexing folder");
            let project = mddb::MDDBProject::new(".").unwrap();
            project.refresh().unwrap();
        },
        Commands::Lint {} => {
            log::debug!("Running lints");
            if let Err(e) = cmd::lint::lint() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        Commands::ApproveEdits { all, paths } => {
            log::debug!("Approving edits");
            if let Err(e) = cmd::lint::approve(*all, paths) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
    }
}
