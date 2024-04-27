mod actions;
mod aufseher;
mod handlers;
mod matching;

use std::{path::PathBuf, process};

use anyhow::Result;
use aufseher::{run, Config};
use clap::Parser;
use tracing::{error, Level};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Telegram bot API token
    #[arg(short = 't', long, env = "TELEGRAM_BOT_TOKEN", required = true)]
    token: String,

    /// Path to config file
    #[arg(short = 'c', long, default_value = "/etc/aufseher.yaml")]
    config_file: PathBuf,
}

fn parse() -> Result<Config> {
    let args = Args::parse();
    Ok(Config::new(args.token, args.config_file))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    match parse() {
        Err(error) => {
            error!("Program initialization error: {}", error);
            process::exit(1);
        }
        Ok(config) => process::exit(match run(config).await {
            Ok(_) => 0,
            Err(error) => {
                error!("{}", error);
                1
            }
        }),
    }
}
