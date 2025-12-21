use anyhow::Result;
use teloxide::{
    prelude::*,
    types::{MediaKind, MessageEntityKind, MessageKind, MessageNewChatMembers, UpdateKind, User},
};
use tracing::{debug, info, warn};

use crate::{actions, aufseher::Config, matching, openai};

pub async fn handle_updates(bot: Bot, update: Update, config: &Config) -> Result<()> {
    match &update.kind {
        UpdateKind::Message(message) => {
            handle_messages(&bot, &message, config).await?;
        }
        UpdateKind::EditedMessage(message) => {
            handle_messages(&bot, &message, config).await?;
        }
        _ => {} // Ignore other update types
    }
    Ok(())
}

async fn handle_messages(bot: &Bot, message: &Message, config: &Config) -> Result<()> {
    // Get group chat title
    let chat_title = if let Some(title) = &message.chat.title() {
        title
    }
    else {
        "None"
    };

    // Handle new chat members
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
    // Handle common messages
    else if let MessageKind::Common(message_common) = &message.kind {
        if let Some(user) = &message.from {
            let mut message_text: Option<&str> = None;

            // Get message text from different media kinds
            if let MediaKind::Text(media_text) = &message_common.media_kind {
                message_text = Some(&media_text.text);
            }
            else if let Some(caption) = &message.caption() {
                message_text = Some(&caption);
            }
            else if let MediaKind::Sticker(_) = &message_common.media_kind {
                debug!("Sticker message ignored");
            }
            else {
                warn!("Unsupported media kind: {:?}", &message_common.media_kind);
            }

            // Handle the message/caption
            if let Some(message_text) = message_text {
                // Process the original message text
                handle_message_common_text(&bot, message, user, chat_title, message_text, &config)
                    .await?;

                // Process URLs from TextLink entities (regular URLs are already in message text)
                if let Some(entities) = message.entities() {
                    for entity in entities {
                        if let MessageEntityKind::TextLink {
                            url,
                        } = &entity.kind
                        {
                            handle_message_common_text(
                                &bot,
                                message,
                                user,
                                chat_title,
                                url.as_str(),
                                &config,
                            )
                            .await?;
                        }
                    }
                }
            }
        }
    }

    // Handle sender/forwarder names
    if let Some(user) = &message.from {
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
    // Check if the username matches any of the regexes
    if let Some(matched_regex) = matching::is_match(&user.full_name(), &config.name_regexes)? {
        info!(
            "Username '{}' maches regex '{}'",
            user.full_name(),
            matched_regex.as_str()
        );
        actions::delete_messages_and_ban_user(bot, message, user, chat_title).await?;
    }

    // Check if the message's forwarder username matches any of the regexes
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

    // Check if the message's forwarder chat name matches any of the regexes
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

        // Check if the username matches any of the regexes
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

    // Check if the message text matches any of the regexes
    if let Some(matched_regex) = matching::is_match(&message_text, &config.message_regexes)? {
        info!(
            "Message text '{}' maches regex '{}'",
            message_text,
            matched_regex.as_str()
        );
        actions::delete_messages_and_ban_user(bot, message, user, chat_title).await?;
    }
    // Then check if the deobfuscated message text matches any of the regexes
    else if let Some(matched_regex) =
        matching::is_match_obfuscated(&message_text, &config.message_regexes)?
    {
        info!(
            "Deobfuscated message text of '{}' maches regex '{}'",
            message_text,
            matched_regex.as_str()
        );
        actions::delete_messages_and_ban_user(bot, message, user, chat_title).await?;
    }

    // Respond to `/aufseher ping` command
    if message_text == "/aufseher ping" {
        actions::send_ping_response(bot, message).await?;
    }

    // If the OpenAI API key is provided, use GPT-4o to check if the message is spam
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
