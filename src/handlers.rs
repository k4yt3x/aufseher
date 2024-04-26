use anyhow::Result;
use fancy_regex::Regex;
use teloxide::{
    payloads::SendMessageSetters,
    prelude::*,
    types::{MediaKind, MediaText, MessageKind, MessageNewChatMembers, UpdateKind, User},
};
use tracing::{info, warn};

pub async fn handle_updates(
    bot: Bot,
    update: Update,
    name_regexes: Vec<Regex>,
    message_regexes: Vec<Regex>,
) -> Result<()> {
    if let UpdateKind::Message(message) = &update.kind {
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
                name_regexes,
            )
            .await?;
        }
        // handle common messages
        else if let MessageKind::Common(message_common) = &message.kind {
            if let Some(user) = &message_common.from {
                // handle text messages sent by users in group chats
                if let MediaKind::Text(media_text) = &message_common.media_kind {
                    handle_message_common_text(
                        &bot,
                        message,
                        user,
                        chat_title,
                        media_text,
                        message_regexes,
                    )
                    .await?;
                }
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
    name_regexes: Vec<Regex>,
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
        for regex in &name_regexes {
            if regex.is_match(&member.full_name())? {
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

    Ok(())
}

async fn handle_message_common_text(
    bot: &Bot,
    message: &Message,
    user: &User,
    chat_title: &str,
    media_text: &MediaText,
    message_regexes: Vec<Regex>,
) -> Result<()> {
    info!(
        "New message '{}' from '{}' ({}) in '{}' ({})",
        media_text.text,
        user.full_name(),
        user.id,
        chat_title,
        &message.chat.id
    );

    // check if the message text matches any of the regexes
    for regex in &message_regexes {
        if regex.is_match(&media_text.text)? {
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
    Ok(())
}
