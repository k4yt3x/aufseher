use anyhow::Result;
use teloxide::{
    prelude::*,
    types::{MediaKind, MessageKind, MessageNewChatMembers, UpdateKind, User},
};
use tracing::{info, warn};

use crate::{actions, aufseher::Config, matching, openai};

pub async fn handle_updates(bot: Bot, update: Update, config: &Config) -> Result<()> {
    if let UpdateKind::Message(message) = &update.kind {
        handle_messages(&bot, &message, config).await?;
    }
    Ok(())
}

async fn handle_messages(bot: &Bot, message: &Message, config: &Config) -> Result<()> {
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
            &config,
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
                handle_message_common_text(&bot, message, user, chat_title, message_text, &config)
                    .await?;
            }
        }
    }

    // handle sender/forwarder names
    if let Some(user) = &message.from() {
        handle_message_user_names(&bot, message, chat_title, user, config).await?;
    }

    Ok(())
}

async fn handle_message_user_names(
    bot: &Bot,
    message: &Message,
    chat_title: &str,
    user: &User,
    config: &Config,
) -> Result<()> {
    // check if the username matches any of the regexes
    if let Some(matched_regex) = matching::is_match(&user.full_name(), &config.name_regexes)? {
        info!(
            "Username '{}' maches regex '{}'",
            user.full_name(),
            matched_regex.as_str()
        );
        actions::delete_messages_and_ban_user(bot, message, user, chat_title).await?;
    }

    // check if the message's forwarder username matches any of the regexes
    if let Some(forwarder) = &message.forward_from_user() {
        if let Some(matched_regex) =
            matching::is_match(&forwarder.full_name(), &config.name_regexes)?
        {
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
            if let Some(matched_regex) = matching::is_match(&title, &config.name_regexes)? {
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
    config: &Config,
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
        if let Some(matched_regex) = matching::is_match(&member.full_name(), &config.name_regexes)?
        {
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
    config: &Config,
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
    if let Some(matched_regex) = matching::is_match(&message_text, &config.message_regexes)? {
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

    // if the OpenAI API key is provided, use GPT-4o to check if the message is spam
    if let Some(openai_api_key) = &config.openai_api_key {
        info!("Checking if message is spam using GPT-4o");
        let openai_is_spam =
            openai::openai_check_is_message_spam(&message_text, openai_api_key).await?;

        if openai_is_spam {
            info!("Message '{}' is recognized as spam by GPT-4o", message_text);
            actions::delete_messages_and_ban_user(bot, message, user, chat_title).await?;
        }
        else {
            info!(
                "Message '{}' is recognized as NOT spam by GPT-4o",
                message_text
            );
        }
    }

    Ok(())
}
