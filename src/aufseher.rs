use std::{fs, path::PathBuf};

use anyhow::Result;
use fancy_regex::Regex;
use serde::Deserialize;
use teloxide::prelude::*;
use tracing::{error, info};

use crate::handlers;

pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct Config {
    token: String,
    config_file: PathBuf,
}

impl Config {
    pub fn new(token: String, config_file: PathBuf) -> Config {
        Config {
            token,
            config_file,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct AufseherConfigFile {
    name_regexes: Vec<String>,
    message_regexes: Vec<String>,
}

#[derive(Clone)]
pub struct AufseherConfig {
    pub name_regexes: Vec<Regex>,
    pub message_regexes: Vec<Regex>,
}

async fn handle_wrapper(bot: Bot, update: Update, config: AufseherConfig) -> Result<()> {
    if let Err(error) = handlers::handle_updates(bot, update, &config).await {
        error!("{}", error);
    }

    Ok(())
}

pub async fn run(config: Config) -> Result<()> {
    info!("Aufseher {version} initializing", version = VERSION);

    // initialize the bot with token
    let bot = Bot::new(&config.token);

    let file_contents = fs::read_to_string(config.config_file)?;
    let regex_config: AufseherConfigFile = serde_yaml::from_str(&file_contents)?;

    // load user name regexes
    let name_regexes: Vec<Regex> =
        regex_config
            .name_regexes
            .iter()
            .try_fold(Vec::new(), |mut acc, r| {
                acc.push(Regex::new(r)?);
                Ok::<_, fancy_regex::Error>(acc)
            })?;

    // load message regexes
    let message_regexes: Vec<Regex> =
        regex_config
            .message_regexes
            .iter()
            .try_fold(Vec::new(), |mut acc, r| {
                acc.push(Regex::new(r)?);
                Ok::<_, fancy_regex::Error>(acc)
            })?;

    // create runtime config struct
    let config = AufseherConfig {
        name_regexes,
        message_regexes,
    };

    // initialize the dispatcher
    let handler = dptree::entry().branch(
        Update::filter_message()
            .endpoint(move |bot, update| handle_wrapper(bot, update, config.clone())),
    );

    // start the dispatcher
    info!("Initialization complete, starting to handle updates");
    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
