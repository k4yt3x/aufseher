mod actions;
mod config;
mod handlers;
mod matching;
mod openai;

use std::{path::PathBuf, process};

use anyhow::Result;
use clap::Parser;
use config::Config;
use teloxide::prelude::*;
use tracing::{Level, error, info};

pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Telegram bot API token
    #[arg(short = 't', long, env = "TELEGRAM_BOT_TOKEN", required = true)]
    token: String,

    /// OpenAI API key
    #[arg(short = 'o', long, env = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,

    /// Path to config file
    #[arg(short = 'c', long, default_value = "/etc/aufseher.yaml")]
    config_file: PathBuf,
}

fn parse() -> Result<Config> {
    let args = Args::parse();
    Ok(Config::new(
        args.token,
        args.openai_api_key,
        args.config_file,
    )?)
}

async fn handle_wrapper(bot: Bot, update: Update, config: Config) -> Result<()> {
    if let Err(error) = handlers::handle_updates(bot, update, &config).await {
        error!("{}", error);
    }

    Ok(())
}

pub async fn run(config: Config) -> Result<()> {
    info!("Aufseher {version} initializing", version = VERSION);

    // Initialize the bot with token
    let bot = Bot::new(&config.telegram_bot_token);

    // Initialize the dispatcher
    let config_messages = config.clone();
    let config_edited = config.clone();
    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .endpoint(move |bot, update| handle_wrapper(bot, update, config_messages.clone())),
        )
        .branch(
            Update::filter_edited_message()
                .endpoint(move |bot, update| handle_wrapper(bot, update, config_edited.clone())),
        );

    // Start the dispatcher
    info!("Initialization complete, starting to handle updates");
    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
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
