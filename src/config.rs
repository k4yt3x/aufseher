use std::{fs, path::PathBuf};

use anyhow::Result;
use fancy_regex::Regex;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AufseherConfigFile {
    name_regexes: Vec<String>,
    message_regexes: Vec<String>,
}

#[derive(Clone)]
pub struct Config {
    pub telegram_bot_token: String,
    pub openai_api_key: Option<String>,
    pub name_regexes: Vec<Regex>,
    pub message_regexes: Vec<Regex>,
}

impl Config {
    pub fn new(
        token: String,
        openai_api_key: Option<String>,
        config_file: PathBuf,
    ) -> Result<Config> {
        let file_contents = fs::read_to_string(&config_file)?;
        let regex_config: AufseherConfigFile = serde_yaml::from_str(&file_contents)?;

        // Load user name regexes
        let name_regexes: Vec<Regex> =
            regex_config
                .name_regexes
                .iter()
                .try_fold(Vec::new(), |mut acc, r| {
                    acc.push(Regex::new(r)?);
                    Ok::<_, fancy_regex::Error>(acc)
                })?;

        // Load message regexes
        let message_regexes: Vec<Regex> =
            regex_config
                .message_regexes
                .iter()
                .try_fold(Vec::new(), |mut acc, r| {
                    acc.push(Regex::new(r)?);
                    Ok::<_, fancy_regex::Error>(acc)
                })?;

        Ok(Config {
            telegram_bot_token: token,
            openai_api_key,
            name_regexes,
            message_regexes,
        })
    }
}
