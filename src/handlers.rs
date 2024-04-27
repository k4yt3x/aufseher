use anyhow::Result;
use fancy_regex::Regex;
use teloxide::{
    prelude::*,
    types::{MediaKind, MessageKind, MessageNewChatMembers, UpdateKind, User},
};
use tracing::{info, warn};

use crate::{actions, aufseher::AufseherConfig, matching};

pub async fn handle_updates(bot: Bot, update: Update, config: &AufseherConfig) -> Result<()> {
    if let UpdateKind::Message(message) = &update.kind {
        handle_messages(&bot, &message, config).await?;
    }
    Ok(())
}

async fn handle_messages(bot: &Bot, message: &Message, config: &AufseherConfig) -> Result<()> {
    // get group chat title
    let chat_title = if let Some(title) = &message.chat.title() {
        title
    }
    else {
        "None"
    };

    // handle new chat members
    if let MessageKind::NewChatMembers(message_new_chat_members) = &message.kind {
        handle_message_new_chat_members(
            &bot,
            message,
            chat_title,
            message_new_chat_members,
            &config.name_regexes,
        )
        .await?;
    }
    // handle common messages
    else if let MessageKind::Common(message_common) = &message.kind {
        if let Some(user) = &message_common.from {
            let mut message_text: Option<&str> = None;

            // get message text from different media kinds
            if let MediaKind::Text(media_text) = &message_common.media_kind {
                message_text = Some(&media_text.text);
            }
            else {
                if let Some(caption) = &message.caption() {
                    message_text = Some(&caption);
                }
                else {
                    warn!("Unsupported media kind: {:?}", &message_common.media_kind);
                }
            }

            // handle the message/caption
            if let Some(message_text) = message_text {
                handle_message_common_text(
                    &bot,
                    message,
                    user,
                    chat_title,
                    message_text,
                    &config.message_regexes,
                )
                .await?;
            }
        }
    }

    // handle sender/forwarder names
    if let Some(user) = &message.from() {
        handle_message_user_names(&bot, message, chat_title, user, config.name_regexes.clone())
            .await?;
    }

    Ok(())
}

async fn handle_message_user_names(
    bot: &Bot,
    message: &Message,
    chat_title: &str,
    user: &User,
    name_regexes: Vec<Regex>,
) -> Result<()> {
    // check if the username matches any of the regexes
    if let Some(matched_regex) = matching::is_match(&user.full_name(), &name_regexes)? {
        info!(
            "Username '{}' maches regex '{}'",
            user.full_name(),
            matched_regex.as_str()
        );
        actions::delete_messages_and_ban_user(bot, message, user, chat_title).await?;
    }

    // check if the message's forwarder username matches any of the regexes
    if let Some(forwarder) = &message.forward_from_user() {
        if let Some(matched_regex) = matching::is_match(&forwarder.full_name(), &name_regexes)? {
            info!(
                "Forwarder name '{}' maches regex '{}'",
                forwarder.full_name(),
                matched_regex.as_str()
            );
            actions::delete_messages_and_ban_user(bot, message, user, chat_title).await?;
        }
    }

    // check if the message's forwarder chat name matches any of the regexes
    if let Some(forwarder) = &message.forward_from_chat() {
        if let Some(title) = &forwarder.title() {
            if let Some(matched_regex) = matching::is_match(&title, &name_regexes)? {
                info!(
                    "Forwarder chat name '{}' maches regex '{}'",
                    title,
                    matched_regex.as_str()
                );
                actions::delete_messages_and_ban_user(bot, message, user, chat_title).await?;
            }
        }
    }

    Ok(())
}

async fn handle_message_new_chat_members(
    bot: &Bot,
    message: &Message,
    chat_title: &str,
    message_new_chat_members: &MessageNewChatMembers,
    name_regexes: &Vec<Regex>,
) -> Result<()> {
    for member in &message_new_chat_members.new_chat_members {
        info!(
            "New member '{}' ({}) joined '{}' ({})",
            member.full_name(),
            member.id,
            chat_title,
            &message.chat.id
        );

        // check if the username matches any of the regexes
        if let Some(matched_regex) = matching::is_match(&member.full_name(), name_regexes)? {
            info!(
                "Username '{}' maches regex '{}'",
                member.full_name(),
                matched_regex.as_str()
            );
            actions::delete_messages_and_ban_user(bot, message, member, chat_title).await?;
        }
    }

    Ok(())
}

async fn handle_message_common_text(
    bot: &Bot,
    message: &Message,
    user: &User,
    chat_title: &str,
    message_text: &str,
    message_regexes: &Vec<Regex>,
) -> Result<()> {
    info!(
        "New message '{}' from '{}' ({}) in '{}' ({})",
        message_text,
        user.full_name(),
        user.id,
        chat_title,
        &message.chat.id
    );

    // check if the message text matches any of the regexes
    if let Some(matched_regex) = matching::is_match(&message_text, message_regexes)? {
        info!(
            "Message text '{}' maches regex '{}'",
            message_text,
            matched_regex.as_str()
        );
        actions::delete_messages_and_ban_user(bot, message, user, chat_title).await?;
    }

    // respond to `/aufseher ping` command
    if message_text == "/aufseher ping" {
        actions::send_ping_response(bot, message).await?;
    }

    Ok(())
}
