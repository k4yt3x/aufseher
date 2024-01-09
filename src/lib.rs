use std::{fs, path::PathBuf};

use anyhow::Result;
use regex::Regex;
use serde::Deserialize;
use teloxide::{
    dispatching::UpdateFilterExt,
    payloads::SendMessageSetters,
    prelude::*,
    requests::Request,
    types::{MediaKind, MessageKind, Update, UpdateKind},
};
use tracing::{error, info, warn};

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
pub struct AufseherConfig {
    name_regexes: Vec<String>,
    message_regexes: Vec<String>,
}

pub async fn handle(
    bot: Bot,
    update: Update,
    name_regexes: Vec<Regex>,
    message_regexes: Vec<Regex>,
) -> Result<()> {
    if let UpdateKind::Message(message) = &update.kind {
        let chat_title = if let Some(title) = &message.chat.title() {
            title
        }
        else {
            "None"
        };
        if let MessageKind::NewChatMembers(message_new_chat_members) = &message.kind {
            for member in &message_new_chat_members.new_chat_members {
                info!(
                    "New member '{}' ({}) joined '{}' ({})",
                    member.full_name(),
                    member.id,
                    chat_title,
                    &message.chat.id
                );
                for regex in &name_regexes {
                    if regex.is_match(&member.full_name()) {
                        info!(
                            "Username '{}' maches regex '{}'",
                            member.full_name(),
                            regex.as_str()
                        );
                        bot.delete_message(message.chat.id.clone(), message.id.clone())
                            .send()
                            .await?;
                        bot.ban_chat_member(message.chat.id.clone(), member.id)
                            .revoke_messages(true)
                            .send()
                            .await?;
                        warn!(
                            "User '{}' ({}) has been banned from '{}' ({})",
                            member.full_name(),
                            member.id,
                            chat_title,
                            &message.chat.id
                        );
                        break;
                    }
                }
            }
        }
        else if let MessageKind::Common(message_common) = &message.kind {
            if let Some(user) = &message_common.from {
                if let MediaKind::Text(media_text) = &message_common.media_kind {
                    info!(
                        "New message '{}' from '{}' ({}) in '{}' ({})",
                        media_text.text,
                        user.full_name(),
                        user.id,
                        chat_title,
                        &message.chat.id
                    );
                    for regex in &message_regexes {
                        if regex.is_match(&media_text.text) {
                            info!(
                                "Message text '{}' maches regex '{}'",
                                media_text.text,
                                regex.as_str()
                            );
                            bot.delete_message(message.chat.id.clone(), message.id.clone())
                                .send()
                                .await?;
                            bot.ban_chat_member(message.chat.id.clone(), user.id)
                                .revoke_messages(true)
                                .send()
                                .await?;
                            bot.send_message(
                                message.chat.id.clone(),
                                format!(
                                    "User [{} \\({}\\)](tg://user?id\\={}) has been banned\\.",
                                    user.full_name(),
                                    user.id,
                                    user.id
                                ),
                            )
                            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                            .await?;
                            warn!(
                                "User '{}' ({}) has been banned from '{}' ({})",
                                user.full_name(),
                                user.id,
                                chat_title,
                                &message.chat.id
                            );
                            break;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

async fn handle_wrapper(
    bot: Bot,
    update: Update,
    name_regexes: Vec<Regex>,
    message_regexes: Vec<Regex>,
) -> Result<()> {
    if let Err(error) = handle(bot, update, name_regexes, message_regexes).await {
        error!("{}", error);
    }

    Ok(())
}

pub async fn run(config: Config) -> Result<()> {
    info!("Aufseher {version} initializing", version = VERSION);

    // initialize the bot with token
    let bot = Bot::new(&config.token);

    let file_contents = fs::read_to_string(config.config_file)?;
    let regex_config: AufseherConfig = serde_yaml::from_str(&file_contents)?;

    // load user name regexes
    let name_regexes: Vec<Regex> =
        regex_config
            .name_regexes
            .iter()
            .try_fold(Vec::new(), |mut acc, r| {
                acc.push(Regex::new(r)?);
                Ok::<_, regex::Error>(acc)
            })?;

    // load message regexes
    let message_regexes: Vec<Regex> =
        regex_config
            .message_regexes
            .iter()
            .try_fold(Vec::new(), |mut acc, r| {
                acc.push(Regex::new(r)?);
                Ok::<_, regex::Error>(acc)
            })?;

    // initialize the dispatcher
    let handler = dptree::entry().branch(Update::filter_message().endpoint({
        move |bot, update| {
            handle_wrapper(bot, update, name_regexes.clone(), message_regexes.clone())
        }
    }));

    // start the dispatcher
    info!("Initialization complete, starting to handle updates");
    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
