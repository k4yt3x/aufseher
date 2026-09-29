//! Telegram operations the bot performs on a chat.

use std::{fmt, time::Duration};

use anyhow::Result;
use teloxide::{
    prelude::*,
    types::{Chat, ParseMode, ReplyParameters, UserId},
    utils::markdown::escape,
};
use tracing::warn;

use crate::detection::Offender;

/// How long the ping reply stays up before the bot deletes it along with the command.
const PING_REPLY_LIFETIME: Duration = Duration::from_secs(1);

/// Why an offender is spared the ban.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Exemption {
    /// The offender is this bot, which cannot ban itself.
    ThisBot,
    /// The user administers the chat.
    Administrator,
    /// The message was sent on behalf of the chat itself, which only its administrators can do.
    AnonymousAdministrator,
    /// The message was sent on behalf of the chat's linked channel, as its posts are when Telegram
    /// forwards them into its discussion group.
    LinkedChannel,
}

impl fmt::Display for Exemption {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ThisBot => "is this bot",
            Self::Administrator => "is an administrator",
            Self::AnonymousAdministrator => "is an anonymous administrator",
            Self::LinkedChannel => "is the linked channel",
        })
    }
}

/// Returns why `offender` is exempt from bans in `chat`, if they are. `this_bot` is the bot's own
/// user ID.
pub(crate) async fn exemption(
    bot: &Bot,
    chat: &Chat,
    offender: Offender<'_>,
    this_bot: UserId,
) -> Result<Option<Exemption>> {
    match offender {
        Offender::User(user) if user.id == this_bot => Ok(Some(Exemption::ThisBot)),
        Offender::User(user) => {
            let member = bot.get_chat_member(chat.id, user.id).await?;
            Ok(member.is_privileged().then_some(Exemption::Administrator))
        }
        Offender::SenderChat(sender) if sender.id == chat.id => {
            Ok(Some(Exemption::AnonymousAdministrator))
        }
        Offender::SenderChat(sender) => {
            let linked = bot.get_chat(chat.id).await?.linked_chat_id();
            Ok((linked == Some(sender.id.0)).then_some(Exemption::LinkedChannel))
        }
    }
}

/// Bans `offender` from `chat` and posts a silent notice with their name behind a spoiler. A user's
/// messages are revoked; a sender chat's owner can no longer post on behalf of any of their
/// channels. A failed notice is logged rather than returned, since the ban has already happened.
pub(crate) async fn ban(bot: &Bot, chat: ChatId, offender: Offender<'_>) -> Result<()> {
    let kind = match offender {
        Offender::User(user) => {
            bot.ban_chat_member(chat, user.id)
                .revoke_messages(true)
                .await?;
            "User"
        }
        Offender::SenderChat(sender) => {
            bot.ban_chat_sender_chat(chat, sender.id).await?;
            "Channel"
        }
    };
    let notice = bot
        .send_message(
            chat,
            format!(
                r"{kind} {} \(||{}||\) has been banned\.",
                escape(&offender.id().to_string()),
                escape(&offender.name())
            ),
        )
        .parse_mode(ParseMode::MarkdownV2)
        .disable_notification(true)
        .await;
    if let Err(error) = notice {
        warn!("Failed to post the ban notice in chat {chat}: {error}");
    }
    Ok(())
}

/// Replies "pong!" to `message`, then deletes both after [`PING_REPLY_LIFETIME`]. The deletion runs
/// on its own task, since the chat's next update waits for this one to finish.
pub(crate) async fn answer_ping(bot: &Bot, message: &Message) -> Result<()> {
    let reply = bot
        .send_message(message.chat.id, "pong!")
        .reply_parameters(ReplyParameters::new(message.id))
        .await?;
    tokio::spawn({
        let bot = bot.clone();
        let chat = message.chat.id;
        let exchange = [message.id, reply.id];
        async move {
            tokio::time::sleep(PING_REPLY_LIFETIME).await;
            if let Err(error) = bot.delete_messages(chat, exchange).await {
                warn!("Failed to delete the ping exchange in chat {chat}: {error}");
            }
        }
    });
    Ok(())
}
