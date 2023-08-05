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
use chrono::Utc;
use regex::Regex;
use serde::Deserialize;
use teloxide::{
    adaptors::throttle::{Limits, Throttle},
    dispatching::UpdateFilterExt,
    prelude::*,
    requests::{Request, RequesterExt},
    types::{MessageKind, Update, UpdateKind},
};
use tracing::{error, info};

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
    spam_name_regexes: Vec<String>,
}

pub async fn handle(bot: Throttle<Bot>, update: Update, regexes: Vec<Regex>) -> Result<()> {
    if let UpdateKind::Message(message) = &update.kind {
        if let MessageKind::NewChatMembers(data) = &message.kind {
            let chat = message.chat.clone();
            for member in &data.new_chat_members {
                info!(
                    "New member '{}' ({}) joined '{}' ({})",
                    member.full_name(),
                    member.id,
                    {
                        if let Some(title) = &chat.title() {
                            title
                        }
                        else {
                            "None"
                        }
                    },
                    chat.id
                );
                for regex in &regexes {
                    if regex.is_match(&member.full_name()) {
                        info!(
                            "Username '{}' maches regex '{}'",
                            member.full_name(),
                            regex.as_str()
                        );
                        bot.kick_chat_member(chat.id, member.id)
                            .until_date(Utc::now())
                            .revoke_messages(true)
                            .send()
                            .await?;
                        info!(
                            "User '{}' ({}) has been banned from '{}' ({})",
                            member.full_name(),
                            member.id,
                            {
                                if let Some(title) = &chat.title() {
                                    title
                                }
                                else {
                                    "None"
                                }
                            },
                            chat.id
                        );
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

async fn handle_wrapper(bot: Throttle<Bot>, update: Update, regexes: Vec<Regex>) -> Result<()> {
    if let Err(error) = handle(bot, update, regexes).await {
        error!("{}", error);
    }

    Ok(())
}

pub async fn run(config: Config) -> Result<()> {
    info!("Aufseher {version} initializing", version = VERSION);

    let bot = Bot::new(&config.token).throttle(Limits::default());

    let regexes_parsed: Result<Vec<Regex>, regex::Error> = config
        .regex_config
        .spam_name_regexes
        .iter()
        .try_fold(Vec::new(), |mut acc, r| {
            acc.push(Regex::new(r)?);
            Ok(acc)
        });

    let regexes = regexes_parsed?;

    let handler = dptree::entry().branch(Update::filter_message().endpoint({
        let regexes = regexes.clone();
        move |bot, update| handle_wrapper(bot, update, regexes.clone())
    }));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
