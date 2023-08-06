use std::{fs, path::PathBuf, process};

use anyhow::Result;
use aufseher::{run, AufseherConfig, Config};
use clap::Parser;
use tracing::{error, Level};
use tracing_subscriber;

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

    // Read config file
    let file_contents = fs::read_to_string(args.config_file)?;
    let regex_config: AufseherConfig = serde_yaml::from_str(&file_contents)?;

    Ok(Config::new(args.token, regex_config))
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
