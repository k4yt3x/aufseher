/*
 * Copyright (C) 2023 K4YT3X.
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU General Public License
 * as published by the Free Software Foundation; only version 2
 * of the License.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 */

use anyhow::Result;
use regex::Regex;
use serde::Deserialize;
use teloxide::{
    adaptors::throttle::{Limits, Throttle},
    dispatching::UpdateFilterExt,
    prelude::*,
    requests::{Request, RequesterExt},
    types::{MediaKind, MessageKind, Update, UpdateKind},
};
use tracing::{error, info, warn};

pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct Config {
    token: String,
    regex_config: AufseherConfig,
}

impl Config {
    pub fn new(token: String, regex_config: AufseherConfig) -> Config {
        Config {
            token,
            regex_config,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct AufseherConfig {
    name_regexes: Vec<String>,
    message_regexes: Vec<String>,
}

pub async fn handle(
    bot: Throttle<Bot>,
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
    bot: Throttle<Bot>,
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

    let bot = Bot::new(&config.token).throttle(Limits::default());

    let name_regexes: Vec<Regex> =
        config
            .regex_config
            .name_regexes
            .iter()
            .try_fold(Vec::new(), |mut acc, r| {
                acc.push(Regex::new(r)?);
                Ok::<_, regex::Error>(acc)
            })?;

    let message_regexes: Vec<Regex> =
        config
            .regex_config
            .message_regexes
            .iter()
            .try_fold(Vec::new(), |mut acc, r| {
                acc.push(Regex::new(r)?);
                Ok::<_, regex::Error>(acc)
            })?;

    let handler = dptree::entry().branch(Update::filter_message().endpoint({
        move |bot, update| {
            handle_wrapper(bot, update, name_regexes.clone(), message_regexes.clone())
        }
    }));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
