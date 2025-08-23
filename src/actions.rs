use anyhow::Result;
use teloxide::{
    prelude::*,
    types::{Message, User},
};
use tokio::{time, time::Duration};
use tracing::warn;

pub async fn delete_messages_and_ban_user(
    bot: &Bot,
    message: &Message,
    user: &User,
    chat_title: &str,
) -> Result<()> {
    // Get the member status of the user
    let member = bot
        .get_chat_member(message.chat.id.clone(), user.id)
        .send()
        .await?;

    // Skip the ban if the user is an admin or creator
    if member.is_administrator() || member.is_owner() {
        warn!(
            "User '{}' ({}) is an admin or creator in '{}'. Skipping ban.",
            user.full_name(),
            user.id,
            chat_title,
        );
        return Ok(());
    }

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

    Ok(())
}

pub async fn send_ping_response(bot: &Bot, message: &Message) -> Result<()> {
    let pong_message = bot
        .send_message(message.chat.id.clone(), "pong!")
        .reply_to_message_id(message.id)
        .send()
        .await?;

    // Sleep for 1 second
    time::sleep(Duration::from_secs(1)).await;

    // Delete the ping message and the pong message
    bot.delete_message(message.chat.id.clone(), message.id.clone())
        .send()
        .await?;
    bot.delete_message(message.chat.id.clone(), pong_message.id.clone())
        .send()
        .await?;

    Ok(())
}
