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
    /// Initialize grepdown in the folder
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

        /// Treat the query as a literal string (no FTS5 operators)
        #[arg(long)]
        literal: bool,

        /// Output query results as compact JSON
        #[arg(long)]
        json: bool,

        /// Filter results to files under this subfolder path
        #[arg(long)]
        path: Option<String>,

        /// Number of tokens in search snippets
        #[arg(long, default_value = "32")]
        snippet_length: i64,
    },

    /// Explicitly index the folder
    Index {},

    /// Run lints on the knowledge base
    Lint {
        /// Output results as compact JSON
        #[arg(long)]
        json: bool,
    },

    /// Approve stale references for specific files or all files
    ApproveEdits {
        /// Approve all stale references
        #[arg(long)]
        all: bool,

        /// Specific file or folder paths to approve
        #[arg(required_unless_present = "all")]
        paths: Vec<String>,
    },

    /// Show documents reachable from a given document via link graph
    Reach {
        /// Starting document (typically a relative path)
        doc: String,

        /// Maximum hop depth for traversal
        #[arg(short, long, default_value = "2")]
        depth: i64,

        /// Output results as compact JSON
        #[arg(long)]
        json: bool,
    },

    /// Read a document's content from the knowledge base
    Read {
        /// Document path relative to the project root
        path: String,
    },

    /// List all indexed documents
    List {
        /// Output results as compact JSON
        #[arg(long)]
        json: bool,
    },

    /// Show knowledge base statistics
    Stats {
        /// Output results as compact JSON
        #[arg(long)]
        json: bool,
    },

    /// List all tags with document counts
    Tags {
        /// Output results as compact JSON
        #[arg(long)]
        json: bool,
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
            log::debug!("Initializing grepdown");
            cmd::init::init();
        },
        Commands::Search { query, limit, no_refresh, literal, json, path, snippet_length } => {
            log::debug!("Searching for: {}", query);
            if let Err(e) = cmd::search::search(query, *limit, *no_refresh, *literal, *json, path.as_deref(), Some(*snippet_length)) {
                eprintln!("Error: {:#}", e);
                std::process::exit(1);
            }
        },
        Commands::Index { } => {
            log::debug!("Indexing folder");
            let project = grepdown_lib::MDDBProject::new(".").unwrap();
            project.refresh().unwrap();
        },
        Commands::Lint { json } => {
            log::debug!("Running lints");
            if let Err(e) = cmd::lint::lint(*json) {
                eprintln!("Error: {:#}", e);
                std::process::exit(1);
            }
        },
        Commands::ApproveEdits { all, paths } => {
            log::debug!("Approving edits");
            if let Err(e) = cmd::lint::approve(*all, paths) {
                eprintln!("Error: {:#}", e);
                std::process::exit(1);
            }
        },
        Commands::Reach { doc, depth, json } => {
            log::debug!("Computing reachability from: {}", doc);
            if let Err(e) = cmd::reach::reach(doc, *depth, *json) {
                eprintln!("Error: {:#}", e);
                std::process::exit(1);
            }
        },
        Commands::Read { path } => {
            log::debug!("Reading document: {}", path);
            if let Err(e) = cmd::read::read(path) {
                eprintln!("Error: {:#}", e);
                std::process::exit(1);
            }
        },
        Commands::List { json } => {
            log::debug!("Listing documents");
            if let Err(e) = cmd::list::execute(cmd::list::ListArgs { json: *json }) {
                eprintln!("Error: {:#}", e);
                std::process::exit(1);
            }
        },
        Commands::Stats { json } => {
            log::debug!("Showing statistics");
            if let Err(e) = cmd::stats::execute(cmd::stats::StatsArgs { json: *json }) {
                eprintln!("Error: {:#}", e);
                std::process::exit(1);
            }
        },
        Commands::Tags { json } => {
            log::debug!("Listing tags");
            if let Err(e) = cmd::tags::execute(cmd::tags::TagsArgs { json: *json }) {
                eprintln!("Error: {:#}", e);
                std::process::exit(1);
            }
        },
    }
}
