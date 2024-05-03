use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author = "Oliver Winks (@ohuu), William Kimbell (@seal)", version, about, long_about = None)]
pub struct Args {
    /// Verbose mode (-v, --verbose)
    #[arg(short, long, action = clap::ArgAction::SetFalse)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run a scenario
    Run {
        /// Name of config name
        #[arg(short, long)]
        name: String,
    },
}

pub fn parse() -> Args {
    Args::parse()
}
