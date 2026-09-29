//! The dispatcher endpoint for new and edited messages.

use std::sync::Arc;

use anyhow::{Context, Result};
use teloxide::{
    prelude::*,
    types::{Chat, Me, User, UserId},
};
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

use crate::{
    actions,
    detection::{self, Detection, Offender},
    rules::Rules,
};

/// The command that makes the bot reply, to show it is running.
const PING_COMMAND: &str = "/aufseher ping";

/// Handles a new or edited message, attaching the message to any error.
pub(crate) async fn handle_message(
    bot: Bot,
    message: Message,
    me: Me,
    rules: watch::Receiver<Arc<Rules>>,
) -> Result<()> {
    // A snapshot, so that a reload mid-message cannot mix two sets of rules.
    let rules = Arc::clone(&rules.borrow());
    process(&bot, &message, me.user.id, &rules)
        .await
        .with_context(|| {
            format!(
                "failed to handle message {} in {}",
                message.id,
                chat_label(&message.chat)
            )
        })
}

/// Bans the spammers `message` reveals, then answers the ping command if nobody was banned.
async fn process(bot: &Bot, message: &Message, this_bot: UserId, rules: &Rules) -> Result<()> {
    log_message(message);

    // A private chat has nobody to ban.
    if !message.chat.is_private() {
        let detections = detection::detect(message, rules);
        if enforce(bot, message, &detections, this_bot).await {
            return Ok(());
        }
    }

    if message.text() == Some(PING_COMMAND) && message.edit_date().is_none() {
        actions::answer_ping(bot, message).await?;
    }

    Ok(())
}

/// Deletes `message` and bans each offender without an [`actions::Exemption`]. Returns whether
/// anyone was banned. Each offender's failure is logged rather than returned, so that it cannot
/// spare the offenders after it.
async fn enforce(
    bot: &Bot,
    message: &Message,
    detections: &[Detection<'_>],
    this_bot: UserId,
) -> bool {
    let chat = chat_label(&message.chat);
    let mut deletion_attempted = false;
    let mut banned_anyone = false;

    for detection in detections {
        let offender = detection.offender;
        let label = offender_label(offender);
        info!("{label} in {chat}: {detection}");

        match actions::exemption(bot, &message.chat, offender, this_bot).await {
            Ok(None) => {}
            Ok(Some(exemption)) => {
                warn!("{label} {exemption} in {chat}, skipping ban");
                continue;
            }
            Err(error) => {
                error!("Failed to check whether {label} is exempt in {chat}: {error:#}");
                continue;
            }
        }

        // The ban matters more than the deletion, so a failed deletion does not stop it.
        if !deletion_attempted {
            deletion_attempted = true;
            if let Err(error) = bot.delete_message(message.chat.id, message.id).await {
                warn!("Failed to delete message {} in {chat}: {error}", message.id);
            }
        }

        match actions::ban(bot, message.chat.id, offender).await {
            Ok(()) => {
                banned_anyone = true;
                warn!("{label} has been banned from {chat}");
            }
            Err(error) => error!("Failed to ban {label} from {chat}: {error:#}"),
        }
    }

    banned_anyone
}

/// Logs arrivals at info and message content at debug.
fn log_message(message: &Message) {
    let chat = chat_label(&message.chat);
    for member in message.new_chat_members().unwrap_or_default() {
        info!("New member {} joined {chat}", user_label(member));
    }
    if let Some(sender) = detection::sender(message)
        && let Some(text) = message.text().or_else(|| message.caption())
    {
        debug!(
            "Message from {} in {chat}: {text:?}",
            offender_label(sender)
        );
    }
}

/// Describes `offender` for the log.
fn offender_label(offender: Offender<'_>) -> String {
    match offender {
        Offender::User(user) => user_label(user),
        Offender::SenderChat(chat) => chat_label(chat),
    }
}

/// Describes `chat` for the log. The title is quoted and escaped, since anyone can set it.
fn chat_label(chat: &Chat) -> String {
    match chat.title() {
        Some(title) => format!("{title:?} ({})", chat.id),
        None => chat.id.to_string(),
    }
}

/// Describes `user` for the log. The name is quoted and escaped, since anyone can set it.
fn user_label(user: &User) -> String {
    format!("{:?} ({})", user.full_name(), user.id)
}
