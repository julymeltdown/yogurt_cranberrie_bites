//! Closed conversion and original HTTP intent matching for qualified commands.
use crate::adapters::r#in::grpc::concert_feed::{proto, read_wire};
use platform::concert_feed_command::FeedCommandRoute as Route;
use post_application::concert_feed::Context;
use post_domain::concert_feed::{Command, Content, Error, Mutation, Outcome};
use serde_json::{json, Value};
use uuid::Uuid;

pub(crate) fn context(value: Option<proto::Context>) -> Result<Context, Error> {
    read_wire::context(value).map_err(|_| Error::Authority)
}
pub(crate) fn command(value: proto::Command) -> Result<Command, Error> {
    use proto::command::Mutation as Wire;
    let mutation = match value.mutation.ok_or(Error::Invalid)? {
        Wire::CreatePost(v) => Mutation::CreatePost {
            content: content(v.content)?,
        },
        Wire::EditPost(v) => Mutation::EditPost {
            expected_revision: v.expected_revision,
            content: content(v.content)?,
        },
        Wire::DeletePost(v) => Mutation::DeletePost {
            expected_revision: v.expected_revision,
        },
        Wire::CreateComment(v) => Mutation::CreateComment {
            comment_id: id(&v.comment_id)?,
            post_revision: v.post_revision,
            text: v.text,
        },
        Wire::EditComment(v) => Mutation::EditComment {
            comment_id: id(&v.comment_id)?,
            expected_revision: v.expected_revision,
            text: v.text,
        },
        Wire::DeleteComment(v) => Mutation::DeleteComment {
            comment_id: id(&v.comment_id)?,
            expected_revision: v.expected_revision,
        },
        Wire::SetReaction(v) => Mutation::SetReaction {
            expected_revision: v.expected_revision,
            active: v.active,
        },
    };
    let command = Command {
        operation_id: id(&value.operation_id)?,
        actor_id: id(&value.actor_id)?,
        event_id: id(&value.event_id)?,
        post_id: id(&value.post_id)?,
        idempotency_key: value.idempotency_key,
        mutation,
    };
    command.validate()?;
    Ok(command)
}
fn id(value: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(value)
        .ok()
        .filter(|v| !v.is_nil() && v.to_string() == value)
        .ok_or(Error::Invalid)
}
fn content(value: Option<proto::Content>) -> Result<Content, Error> {
    let v = value.ok_or(Error::Invalid)?;
    let c = serde_json::from_value::<Content>(json!({
        "text": v.text,
        "media_ids": v.media_ids,
        "links": v.links.into_iter().map(|l| json!({
            "target_type": l.target_type,
            "target_id": l.target_id
        })).collect::<Vec<_>>()
    }))
    .map_err(|_| Error::Invalid)?;
    c.validate()?;
    Ok(c)
}
pub(crate) fn target(actor: Uuid, event: Uuid, key: Uuid, operation: &str) -> Uuid {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"mcr/feed/target/v1");
    for id in [actor, event, key] {
        hash.update(id.as_bytes());
    }
    hash.update(operation.as_bytes());
    let digest = hash.finalize();
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
/// Typed command must mean exactly the original admitted request. Receipt IDs
/// are assigned by Concert's durable journal, never a second client operation.
pub(crate) fn intent(
    route: Route,
    command: &Command,
    authority: &proto::CommandAuthority,
) -> Result<(), Error> {
    route
        .validate_body(&authority.content_type, &authority.original_body)
        .map_err(|_| Error::Invalid)?;
    let key = id(&authority.idempotency_key)?;
    if command.idempotency_key != authority.idempotency_key {
        return Err(Error::Authority);
    }
    let expected = match (&route, &command.mutation) {
        (Route::CreatePost, Mutation::CreatePost { content })
            if command.post_id
                == target(command.actor_id, command.event_id, key, route.operation()) =>
        {
            serde_json::to_value(content).map_err(|_| Error::Invalid)?
        }
        (
            Route::EditPost(post),
            Mutation::EditPost {
                expected_revision,
                content,
            },
        ) if *post == command.post_id => {
            json!({
                "expected_revision": expected_revision,
                "text": content.text,
                "media_ids": content.media_ids,
                "links": content.links
            })
        }
        (Route::DeletePost(post, revision), Mutation::DeletePost { expected_revision })
            if *post == command.post_id && revision == expected_revision =>
        {
            return Ok(())
        }
        (
            Route::CreateComment(post),
            Mutation::CreateComment {
                comment_id,
                post_revision,
                text,
            },
        ) if *post == command.post_id
            && *comment_id
                == target(command.actor_id, command.event_id, key, route.operation()) =>
        {
            json!({"post_revision": post_revision, "text": text})
        }
        (
            Route::EditComment(id),
            Mutation::EditComment {
                comment_id,
                expected_revision,
                text,
            },
        ) if id == comment_id => json!({
            "expected_revision": expected_revision,
            "text": text
        }),
        (
            Route::DeleteComment(id, revision),
            Mutation::DeleteComment {
                comment_id,
                expected_revision,
            },
        ) if id == comment_id && revision == expected_revision => return Ok(()),
        (Route::Like(post), Mutation::SetReaction { active: true, .. })
            if *post == command.post_id =>
        {
            json!({})
        }
        (Route::Unlike(post), Mutation::SetReaction { active: false, .. })
            if *post == command.post_id =>
        {
            return Ok(())
        }
        _ => return Err(Error::Authority),
    };
    let actual = strict_json(&authority.original_body)?;
    if actual != expected {
        return Err(Error::Authority);
    }
    Ok(())
}
/// Decode keys before uniqueness checking, including escaped aliases. Value's
/// default object decoder keeps the last duplicate and is insufficient here.
fn strict_json(bytes: &[u8]) -> Result<Value, Error> {
    use serde::Deserialize;
    struct Unique(Value);
    impl<'de> Deserialize<'de> for Unique {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = Unique;
                fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str("closed Feed JSON without duplicate keys")
                }
                fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Unique, E> {
                    Ok(Unique(Value::Bool(v)))
                }
                fn visit_unit<E: serde::de::Error>(self) -> Result<Unique, E> {
                    Ok(Unique(Value::Null))
                }
                fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Unique, E> {
                    if !(-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&v) {
                        return Err(E::custom("unsafe integer"));
                    }
                    Ok(Unique(Value::from(v)))
                }
                fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Unique, E> {
                    if v > 9_007_199_254_740_991 {
                        return Err(E::custom("unsafe integer"));
                    }
                    Ok(Unique(Value::from(v)))
                }
                fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Unique, E> {
                    Ok(Unique(Value::String(v.into())))
                }
                fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Unique, E> {
                    Ok(Unique(Value::String(v)))
                }
                fn visit_seq<A: serde::de::SeqAccess<'de>>(
                    self,
                    mut seq: A,
                ) -> Result<Unique, A::Error> {
                    let mut values = Vec::new();
                    while let Some(Unique(v)) = seq.next_element()? {
                        values.push(v);
                    }
                    Ok(Unique(Value::Array(values)))
                }
                fn visit_map<A: serde::de::MapAccess<'de>>(
                    self,
                    mut map: A,
                ) -> Result<Unique, A::Error> {
                    let mut values = serde_json::Map::new();
                    while let Some((key, Unique(value))) = map.next_entry::<String, Unique>()? {
                        if values.insert(key, value).is_some() {
                            return Err(serde::de::Error::custom("duplicate key"));
                        }
                    }
                    Ok(Unique(Value::Object(values)))
                }
            }
            d.deserialize_any(Visitor)
        }
    }
    serde_json::from_slice::<Unique>(bytes)
        .map(|v| v.0)
        .map_err(|_| Error::Invalid)
}
pub(crate) fn receipt(v: Outcome, key: String) -> proto::Receipt {
    proto::Receipt {
        operation_id: v.operation_id.to_string(),
        event_id: v.event_id.to_string(),
        post_id: v.post_id.to_string(),
        comment_id: v.comment_id.map(|id| id.to_string()),
        revision: v.revision,
        post_revision: v.post_revision,
        deleted: v.deleted,
        reacted_by_me: v.reacted_by_me,
        reaction_count: v.reaction_count,
        committed_at: v.committed_at.to_rfc3339(),
        actor_id: v.actor_id.to_string(),
        idempotency_key: key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit() -> (Command, proto::CommandAuthority) {
        let command = Command {
            operation_id: Uuid::from_u128(1),
            actor_id: Uuid::from_u128(2),
            event_id: Uuid::from_u128(3),
            post_id: Uuid::from_u128(4),
            idempotency_key: Uuid::from_u128(5).to_string(),
            mutation: Mutation::EditPost {
                expected_revision: 7,
                content: Content {
                    text: "actual".into(),
                    media_ids: vec![],
                    links: vec![],
                },
            },
        };
        let authority = proto::CommandAuthority {
            owners: None,
            original_body: br#"{"expected_revision":7,"text":"actual","media_ids":[],"links":[]}"#
                .to_vec(),
            content_type: "application/json".into(),
            idempotency_key: command.idempotency_key.clone(),
            original_http_method: "PATCH".into(),
            original_http_target: format!(
                "/api/concert/events/{}/posts/{}",
                command.event_id, command.post_id
            ),
        };
        (command, authority)
    }

    #[test]
    fn exact_original_intent_rejects_duplicates_escaped_aliases_and_extra_identity() {
        let (command, mut a) = edit();
        let route = Route::EditPost(command.post_id);
        assert!(intent(route, &command, &a).is_ok());
        for body in [
            br#"{"expected_revision":7,"text":"wrong","text":"actual","media_ids":[],"links":[]}"#
                .as_slice(),
            br#"{"expected_revision":7,"text":"wrong","te\u0078t":"actual","media_ids":[],"links":[]}"#
                .as_slice(),
            br#"{"expected_revision":7,"text":"actual","media_ids":[],"links":[],"actor_id":"forged"}"#
                .as_slice(),
            br#"{"expected_revision":7.0,"text":"actual","media_ids":[],"links":[]}"#.as_slice(),
            br#"{"expected_revision":8,"text":"actual","media_ids":[],"links":[]}"#.as_slice(),
        ] {
            a.original_body = body.to_vec();
            assert!(intent(route, &command, &a).is_err());
        }
    }

    #[test]
    fn typed_command_cannot_rebind_target_key_or_delete_revision() {
        let (mut command, mut a) = edit();
        assert!(intent(Route::EditPost(Uuid::from_u128(6)), &command, &a).is_err());
        a.idempotency_key = Uuid::from_u128(7).to_string();
        assert!(intent(Route::EditPost(command.post_id), &command, &a).is_err());
        a.idempotency_key = command.idempotency_key.clone();
        a.original_body.clear();
        a.content_type.clear();
        command.mutation = Mutation::DeletePost {
            expected_revision: 7,
        };
        assert!(intent(Route::DeletePost(command.post_id, 7), &command, &a).is_ok());
        assert!(intent(Route::DeletePost(command.post_id, 8), &command, &a).is_err());
        a.original_body = b"{}".to_vec();
        assert!(intent(Route::DeletePost(command.post_id, 7), &command, &a).is_err());
    }
}
