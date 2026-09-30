//! Deciding which senders a message reveals as spammers.

use std::fmt;

use teloxide::types::{Chat, ChatId, Message, MessageEntityKind, MessageOrigin, User};

use crate::rules::{Match, Rules};

/// The part of a message a pattern matched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Field {
    /// The name of a user who joined the chat.
    MemberName,
    /// The message text or media caption.
    Text,
    /// The URL behind a text link.
    LinkUrl,
    /// The sender's name.
    SenderName,
    /// The title of the chat the message was sent on behalf of.
    SenderChatTitle,
    /// The name of the user the message was forwarded from, including one who hides their account.
    ForwarderName,
    /// The title of the channel or chat the message was forwarded from.
    ForwarderChatTitle,
    /// The name of the inline bot the message was sent through.
    ViaBotName,
}

impl fmt::Display for Field {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MemberName => "new member name",
            Self::Text => "message text",
            Self::LinkUrl => "link URL",
            Self::SenderName => "sender name",
            Self::SenderChatTitle => "sender chat title",
            Self::ForwarderName => "forwarder name",
            Self::ForwarderChatTitle => "forwarder chat title",
            Self::ViaBotName => "via bot name",
        })
    }
}

/// Who a detection condemns, which decides how they are banned.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Offender<'a> {
    /// A user, banned along with their messages.
    User(&'a User),
    /// A chat a message was sent on behalf of, banned so that its owner can no longer post on
    /// behalf of any of their channels.
    SenderChat(&'a Chat),
}

impl Offender<'_> {
    /// Returns the offender's ID. User IDs are positive and group or channel IDs negative, so the
    /// two never collide.
    pub(crate) fn id(self) -> ChatId {
        match self {
            Self::User(user) => user.id.into(),
            Self::SenderChat(chat) => chat.id,
        }
    }
}

/// An offender to ban, and the match that condemns them.
#[derive(Debug)]
pub(crate) struct Detection<'a> {
    /// The offender to ban.
    pub(crate) offender: Offender<'a>,
    /// The part of the message that matched.
    pub(crate) field: Field,
    /// The text that matched, as it appeared in the message.
    pub(crate) value: String,
    /// The pattern that matched it.
    pub(crate) found: Match<'a>,
}

impl fmt::Display for Detection<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} {:?} matches pattern {:?}",
            self.field, self.value, self.found.pattern
        )?;
        if self.found.deobfuscated {
            formatter.write_str(" once deobfuscated")?;
        }
        Ok(())
    }
}

/// Returns the sender of `message`: the chat it was sent on behalf of if any, since `from` then
/// holds a placeholder user, and otherwise the user who sent it.
pub(crate) fn sender(message: &Message) -> Option<Offender<'_>> {
    message
        .sender_chat
        .as_ref()
        .map(Offender::SenderChat)
        .or_else(|| message.from.as_ref().map(Offender::User))
}

/// Returns a detection for each offender `message` reveals, at most one each: every new member
/// whose name matches, and the sender if their text, links, or names do.
pub(crate) fn detect<'a>(message: &'a Message, rules: &'a Rules) -> Vec<Detection<'a>> {
    let mut detections: Vec<Detection<'a>> = message
        .new_chat_members()
        .unwrap_or_default()
        .iter()
        .filter_map(|member| {
            let name = member.full_name();
            let found = rules.match_name(&name)?;
            Some(Detection {
                offender: Offender::User(member),
                field: Field::MemberName,
                value: name,
                found,
            })
        })
        .collect();

    // A user who joined by themselves is also the sender of the join message.
    if let Some(sender) = sender(message)
        && !detections
            .iter()
            .any(|detection| detection.offender.id() == sender.id())
        && let Some(detection) = detect_sender(message, sender, rules)
    {
        detections.push(detection);
    }

    detections
}

/// Returns the first match against `sender`: their text and link URLs, then the names attached to
/// the message.
fn detect_sender<'a>(
    message: &'a Message,
    sender: Offender<'a>,
    rules: &'a Rules,
) -> Option<Detection<'a>> {
    let text = message.text().or_else(|| message.caption());
    let link_urls = message
        .entities()
        .or_else(|| message.caption_entities())
        .unwrap_or_default()
        .iter()
        .filter_map(|entity| match &entity.kind {
            MessageEntityKind::TextLink { url } => Some(url.as_str()),
            _ => None,
        });
    let texts = text
        .map(|text| (Field::Text, text))
        .into_iter()
        .chain(link_urls.map(|url| (Field::LinkUrl, url)));
    for (field, text) in texts {
        if let Some(found) = rules.match_message(text) {
            return Some(Detection {
                offender: sender,
                field,
                value: text.to_owned(),
                found,
            });
        }
    }

    let sender_name = match sender {
        Offender::User(user) => Some((Field::SenderName, user.full_name())),
        Offender::SenderChat(chat) => chat
            .title()
            .map(|title| (Field::SenderChatTitle, title.to_owned())),
    };
    let forwarder_name = message.forward_origin().and_then(|origin| match origin {
        MessageOrigin::User { sender_user, .. } => {
            Some((Field::ForwarderName, sender_user.full_name()))
        }
        MessageOrigin::HiddenUser {
            sender_user_name, ..
        } => Some((Field::ForwarderName, sender_user_name.clone())),
        MessageOrigin::Chat {
            sender_chat: chat, ..
        }
        | MessageOrigin::Channel { chat, .. } => chat
            .title()
            .map(|title| (Field::ForwarderChatTitle, title.to_owned())),
    });
    let via_bot_name = message
        .via_bot
        .as_ref()
        .map(|bot| (Field::ViaBotName, bot.full_name()));
    [sender_name, forwarder_name, via_bot_name]
        .into_iter()
        .flatten()
        .find_map(|(field, name)| {
            let found = rules.match_name(&name)?;
            Some(Detection {
                offender: sender,
                field,
                value: name,
                found,
            })
        })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use teloxide::types::{ChatId, Message};

    use super::{Field, Offender, detect};
    use crate::rules::Rules;

    const RULES: &str = "{name_regexes: ['(?i)spam ?bot'], message_regexes: ['(?i)buynow']}";

    fn rules() -> Rules {
        Rules::from_yaml(RULES).unwrap()
    }

    fn user(id: u64, name: &str) -> Value {
        json!({ "id": id, "is_bot": false, "first_name": name })
    }

    fn channel(id: i64, title: &str) -> Value {
        json!({ "id": id, "type": "channel", "title": title })
    }

    /// Builds a supergroup message from Alice (user 1), with `fields` added or overriding.
    fn message(fields: Value) -> Message {
        let mut message = json!({
            "message_id": 1,
            "date": 0,
            "chat": { "id": -1001, "type": "supergroup", "title": "Group" },
            "from": user(1, "Alice"),
        });
        let Value::Object(fields) = fields else {
            panic!("message fields are not an object");
        };
        message.as_object_mut().unwrap().extend(fields);
        serde_json::from_value(message).unwrap()
    }

    #[test]
    fn a_clean_message_detects_nobody() {
        let message = message(json!({ "text": "hello there" }));
        assert!(detect(&message, &rules()).is_empty());
    }

    #[test]
    fn matching_text_detects_the_sender() {
        let message = message(json!({ "text": "buynow" }));
        let rules = rules();
        let detections = detect(&message, &rules);
        assert_eq!(detections.len(), 1);
        assert!(matches!(detections[0].offender, Offender::User(user) if user.id.0 == 1));
        assert_eq!(detections[0].field, Field::Text);
    }

    #[test]
    fn a_text_link_in_a_caption_detects_the_sender() {
        let message = message(json!({
            "photo": [{ "file_id": "a", "file_unique_id": "b", "width": 1, "height": 1 }],
            "caption": "look",
            "caption_entities": [
                { "type": "text_link", "offset": 0, "length": 4, "url": "https://buynow.example/" },
            ],
        }));
        let rules = rules();
        let detections = detect(&message, &rules);
        assert_eq!(detections[0].field, Field::LinkUrl);
    }

    #[test]
    fn a_message_forwarded_from_a_spam_channel_detects_the_sender() {
        let message = message(json!({
            "text": "hello there",
            "forward_origin": {
                "type": "channel", "date": 0, "message_id": 7, "chat": channel(-1003, "SpamBot Deals"),
            },
        }));
        let rules = rules();
        let detections = detect(&message, &rules);
        assert_eq!(detections[0].offender.id(), ChatId(1));
        assert_eq!(detections[0].field, Field::ForwarderChatTitle);
    }

    #[test]
    fn matching_text_sent_as_a_channel_detects_the_channel_rather_than_the_placeholder() {
        // Telegram puts this placeholder in `from` when a message is sent on behalf of a channel.
        let placeholder = json!({ "id": 136817688, "is_bot": true, "first_name": "Channel", "username": "Channel_Bot" });
        let message = message(json!({
            "from": placeholder,
            "sender_chat": channel(-1002, "Deals"),
            "text": "buynow",
        }));
        let rules = rules();
        let detections = detect(&message, &rules);
        assert_eq!(detections.len(), 1);
        assert!(
            matches!(detections[0].offender, Offender::SenderChat(chat) if chat.id == ChatId(-1002))
        );
    }

    #[test]
    fn a_member_who_joins_by_themselves_is_detected_once() {
        let message = message(json!({
            "from": user(2, "Spam Bot"),
            "new_chat_members": [user(2, "Spam Bot")],
        }));
        let rules = rules();
        let detections = detect(&message, &rules);
        assert_eq!(detections.len(), 1);
        assert_eq!(detections[0].field, Field::MemberName);
    }

    #[test]
    fn every_matching_new_member_is_detected() {
        let message = message(json!({
            "new_chat_members": [user(2, "Spam Bot"), user(3, "Bob"), user(4, "SpamBot")],
        }));
        let rules = rules();
        let offenders: Vec<ChatId> = detect(&message, &rules)
            .iter()
            .map(|detection| detection.offender.id())
            .collect();
        assert_eq!(offenders, [ChatId(2), ChatId(4)]);
    }
}
