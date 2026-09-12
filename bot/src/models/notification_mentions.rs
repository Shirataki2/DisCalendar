//! API と同じ予定単位のメンション。許可対象を明示し、自由入力からの通知を防ぐ。
use poise::serenity_prelude::CreateAllowedMentions;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum NotificationMention {
    Everyone,
    Role { id: String },
    User { id: String },
}

pub fn message_mentions(raw: &Value) -> (String, CreateAllowedMentions) {
    let empty = || (String::new(), CreateAllowedMentions::new());
    let Ok(mentions) = serde_json::from_value::<Vec<NotificationMention>>(raw.clone()) else {
        return empty();
    };
    if mentions.len() > 10 {
        return empty();
    }
    let mut content = Vec::new();
    let mut everyone = false;
    let mut roles = Vec::new();
    let mut users = Vec::new();
    for mention in mentions {
        match mention {
            NotificationMention::Everyone => {
                everyone = true;
                content.push("@everyone".to_owned());
            }
            NotificationMention::Role { ref id } | NotificationMention::User { ref id } => {
                let Ok(parsed) = id.parse::<std::num::NonZeroU64>() else {
                    return empty();
                };
                if parsed.to_string() != *id {
                    return empty();
                }
                if matches!(mention, NotificationMention::Role { .. }) {
                    roles.push(parsed.get());
                    content.push(format!("<@&{id}>"));
                } else {
                    users.push(parsed.get());
                    content.push(format!("<@{id}>"));
                }
            }
        }
    }
    (
        content.join(" "),
        CreateAllowedMentions::new()
            .everyone(everyone)
            .roles(roles)
            .users(users),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_saved_targets_are_allowed_and_bad_data_does_not_stop_notifications() {
        let (content, allowed) = message_mentions(&json!([
            {"type":"everyone"}, {"type":"role","id":"123456789012345678"},
            {"type":"user","id":"234567890123456789"}
        ]));
        assert_eq!(
            content,
            "@everyone <@&123456789012345678> <@234567890123456789>"
        );
        assert_eq!(
            serde_json::to_value(allowed).unwrap(),
            json!({
                "parse":["everyone"], "roles":["123456789012345678"], "users":["234567890123456789"]
            })
        );
        for raw in [
            json!([]),
            json!(null),
            json!([{"type":"here"}]),
            json!([{"type":"user","id":"0"}]),
            json!([{"type":"role","id":"01"}]),
            json!([{"type":"user","id":"1><@everyone"}]),
            json!([{"type":"user","id":123}]),
            json!([{"type":"role","id":"18446744073709551616"}]),
            json!(vec![json!({"type":"everyone"}); 11]),
        ] {
            let (content, allowed) = message_mentions(&raw);
            assert!(content.is_empty());
            assert_eq!(allowed, CreateAllowedMentions::new());
        }
    }
}
