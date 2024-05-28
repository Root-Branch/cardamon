use anyhow::Context;
use cardamon::{clap_args, settings};
use dotenv::dotenv;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // dotenv
    dotenv().ok();
    // DB_URL will be over-ridden in config parse if set
    env_logger::init();

    // Parse clap args
    let args = clap_args::parse();
    println!("{:?}", args);
    // Log level, toml parsing and validation
    println!("Verbose mode: {}", args.verbose);
    match args.command {
        clap_args::Commands::Run { name } => {
            println!("Running with config name {} ", name);
            let _settings =
                settings::parse(name, args.verbose).context("Failed to parse settings:")?;
        }
    }
    Ok(())
}
