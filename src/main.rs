use cardamon::{config, data_access::LocalDataAccessService, run};
use clap::{Parser, Subcommand};
use tracing::Level;

#[derive(Parser, Debug)]
#[command(author = "Oliver Winks (@ohuu), William Kimbell (@seal)", version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, action = clap::ArgAction::SetFalse)]
    pub verbose: bool,

    #[arg(short, long)]
    pub file: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Run { name: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse clap args
    let args = Cli::parse();

    // Initialize tracing
    let level = if args.verbose {
        Level::TRACE
    } else {
        Level::INFO
    };
    let subscriber = tracing_subscriber::fmt().with_max_level(level).finish();
    tracing::subscriber::set_global_default(subscriber)?;

    match args.command {
        Commands::Run { name } => {
            // set up local data access
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(4)
                .connect("sqlite://cardamon.db")
                .await?;
            let data_access_service = LocalDataAccessService::new(pool);

            // create an execution plan
            let config = config::Config::from_path(std::path::Path::new("./cardamon.toml"))?;
            let execution_plan = config.create_execution_plan(&name)?;

            // run it!
            run(execution_plan, &data_access_service).await
        }
    }
}
