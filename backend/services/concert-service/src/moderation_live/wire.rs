//! Closed five-operation C04 parser, adapted from Operations 8f9197b9 wire binding.
use crate::{
    adapters::out::relationship_authority::guarded::GuardBinding,
    product_rpc::proto::ProductRequest,
};
use chrono::{DateTime, Utc};
use concert_application::{
    admin::{
        moderation::{Change, ContentAction, ContentKind, UserAction},
        Action,
    },
    commands::CommandError,
};
use prost::Message;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::OnceLock};
use uuid::Uuid;
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Query {
    pub limit: u16,
    pub cursor: Option<String>,
    pub filters: BTreeMap<String, String>,
}
pub struct ModerationRequest {
    pub action: Action,
    pub change: Option<Change>,
    pub key: Option<Uuid>,
    pub query: Query,
    pub digest: [u8; 32],
}
impl ModerationRequest {
    pub fn from_owner_scope(
        scope: &GuardBinding,
        f: &ProductRequest,
    ) -> Result<Self, CommandError> {
        scope.check().map_err(|_| CommandError::Denied)?;
        if scope.parent_method.as_ref() != "/concert.product.v1.ProductService/Dispatch"
            || canonical(&f.user_id) != Some(scope.actor())
            || canonical(&f.event_id) != Some(scope.event())
            || canonical(&f.session_family_id) != Some(scope.session_family())
            || canonical(&f.request_id) != Some(scope.request_id())
            || f.http_method != scope.http_method()
            || f.http_target != scope.http_target()
            || Sha256::digest(&f.http_body).as_slice() != scope.body_digest()
            || parent_frame_digest(f)? != scope.parent_body_digest
        {
            return Err(CommandError::Denied);
        }
        let now = Utc::now().timestamp_micros();
        if scope.deadline().expires_micros()
            > f.authority_expires_unix_micros
                .min(f.access_expires_unix_micros)
            || f.authority_expires_unix_micros <= now
            || f.access_expires_unix_micros <= now
            || f.auth_observed_at_unix_micros <= 0
            || f.auth_observed_at_unix_micros > now
            || f.profile_observed_at_unix_micros <= 0
            || f.profile_observed_at_unix_micros > now
        {
            return Err(CommandError::Denied);
        }
        let mut r = parse(
            scope.event(),
            &f.http_method,
            &f.http_target,
            &f.http_body,
            &f.content_type,
            &f.idempotency_key,
        )?;
        if r.action.operation() != scope.operation() {
            return Err(CommandError::Denied);
        }
        let mut h = Sha256::new();
        h.update(b"MCR-moderation-request-v1\0");
        h.update(scope.digest());
        h.update(f.encode_to_vec());
        r.digest = h.finalize().into();
        Ok(r)
    }
}
// VerifiedParentAuthority::request_digest hashes the admitted gRPC body,
// including its uncompressed flag and big-endian unary message length.
fn parent_frame_digest(f: &ProductRequest) -> Result<[u8; 32], CommandError> {
    let body = f.encode_to_vec();
    let length = u32::try_from(body.len()).map_err(|_| CommandError::InvalidInput)?;
    let mut hash = Sha256::new();
    hash.update([0]);
    hash.update(length.to_be_bytes());
    hash.update(body);
    Ok(hash.finalize().into())
}
pub fn parse(
    event: Uuid,
    method: &str,
    target: &str,
    body: &[u8],
    content_type: &str,
    key: &str,
) -> Result<ModerationRequest, CommandError> {
    if event.is_nil()
        || target.len() > 4096
        || !target.is_ascii()
        || target.chars().any(char::is_control)
        || body.len() > 65536
    {
        return Err(CommandError::InvalidInput);
    }
    let (path, raw_query) = target
        .split_once('?')
        .map_or((target, None), |(p, q)| (p, Some(q)));
    let root = format!("/api/concert/events/{event}/admin/");
    let route = path.strip_prefix(&root).ok_or(CommandError::InvalidInput)?;
    let mut query = Query {
        limit: 20,
        ..Default::default()
    };
    let (action, change, key) = match method {
        "GET" => {
            if !body.is_empty() || !content_type.is_empty() || !key.is_empty() {
                return Err(CommandError::InvalidInput);
            }
            let (action, schema) = match route {
                "users" => (Action::AdminUsers, "Domain_admin_UserQuery"),
                "content" => (Action::AdminContent, "Domain_admin_ContentQuery"),
                "audit" => (Action::AuditRead, "Domain_admin_AuditQuery"),
                _ => return Err(CommandError::InvalidInput),
            };
            let mut values = serde_json::Map::new();
            if let Some(raw) = raw_query {
                if raw.is_empty() {
                    return Err(CommandError::InvalidInput);
                }
                for pair in raw.split('&') {
                    let (k, v) = pair.split_once('=').ok_or(CommandError::InvalidInput)?;
                    let (k, mut v) = (decode(k)?, decode(v)?);
                    if matches!(k.as_str(), "user_id" | "actor_id") && canonical(&v).is_none() {
                        return Err(CommandError::InvalidInput);
                    }
                    if matches!(k.as_str(), "from" | "until") {
                        v = timestamp(&v)?.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);
                    }
                    if values.contains_key(&k) {
                        return Err(CommandError::InvalidInput);
                    }
                    values.insert(
                        k.clone(),
                        if k == "limit" {
                            Value::Number(
                                v.parse::<u16>()
                                    .map_err(|_| CommandError::InvalidInput)?
                                    .into(),
                            )
                        } else {
                            Value::String(v)
                        },
                    );
                }
            }
            validate_value(schema, &Value::Object(values.clone()))?;
            for (k, v) in values {
                if k == "limit" {
                    query.limit = v.as_u64().ok_or(CommandError::InvalidInput)? as u16;
                } else if k == "cursor" {
                    query.cursor = Some(v.as_str().ok_or(CommandError::InvalidInput)?.into());
                } else {
                    query
                        .filters
                        .insert(k, v.as_str().ok_or(CommandError::InvalidInput)?.into());
                }
            }
            if let (Some(from), Some(until)) =
                (query.filters.get("from"), query.filters.get("until"))
            {
                if timestamp(from)? >= timestamp(until)? {
                    return Err(CommandError::InvalidInput);
                }
            }
            (action, None, None)
        }
        "POST" => {
            if raw_query.is_some() || content_type != "application/json" {
                return Err(CommandError::InvalidInput);
            }
            let key = canonical(key).ok_or(CommandError::InvalidInput)?;
            let value: Value =
                serde_json::from_slice(body).map_err(|_| CommandError::InvalidInput)?;
            let change = if route == "content/moderation" {
                validate_value("Domain_admin_ModerateContent", &value)?;
                let b: ContentBody =
                    serde_json::from_slice(body).map_err(|_| CommandError::InvalidInput)?;
                Change::Content {
                    kind: ContentKind::parse(&b.target_type)?,
                    target: canonical(&b.target_id).ok_or(CommandError::InvalidInput)?,
                    expected_revision: b.expected_revision,
                    action: match b.action.as_str() {
                        "HIDE" => ContentAction::Hide,
                        "RESTORE" => ContentAction::Restore,
                        _ => return Err(CommandError::InvalidInput),
                    },
                    reason: b.reason,
                }
            } else {
                let id = route
                    .strip_prefix("users/")
                    .and_then(|v| v.strip_suffix("/moderation"))
                    .and_then(canonical)
                    .ok_or(CommandError::InvalidInput)?;
                validate_value("Domain_admin_ModerateUser", &value)?;
                let b: UserBody =
                    serde_json::from_slice(body).map_err(|_| CommandError::InvalidInput)?;
                Change::User {
                    target: id,
                    expected_revision: b.expected_revision,
                    action: match b.action.as_str() {
                        "SUSPEND" => UserAction::Suspend,
                        "UNSUSPEND" => UserAction::Unsuspend,
                        _ => return Err(CommandError::InvalidInput),
                    },
                    expires_at: b.expires_at,
                    reason: b.reason,
                }
            };
            concert_application::admin::event::reason(change.reason())?;
            (change.action(), Some(change), Some(key))
        }
        _ => return Err(CommandError::InvalidInput),
    };
    Ok(ModerationRequest {
        action,
        change,
        key,
        query,
        digest: [0; 32],
    })
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UserBody {
    expected_revision: i64,
    action: String,
    expires_at: Option<DateTime<Utc>>,
    reason: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContentBody {
    expected_revision: i64,
    target_type: String,
    target_id: String,
    action: String,
    reason: String,
}
pub(crate) fn canonical(v: &str) -> Option<Uuid> {
    Uuid::parse_str(v)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == v)
}
fn timestamp(v: &str) -> Result<DateTime<Utc>, CommandError> {
    DateTime::parse_from_rfc3339(v)
        .map(|v| v.with_timezone(&Utc))
        .map_err(|_| CommandError::InvalidInput)
}
fn decode(v: &str) -> Result<String, CommandError> {
    let mut out = Vec::new();
    let bytes = v.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(CommandError::InvalidInput);
            };
            let h = std::str::from_utf8(&bytes[i + 1..i + 3])
                .map_err(|_| CommandError::InvalidInput)?;
            out.push(u8::from_str_radix(h, 16).map_err(|_| CommandError::InvalidInput)?);
            i += 3;
        } else {
            out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
            i += 1;
        }
    }
    let value = String::from_utf8(out).map_err(|_| CommandError::InvalidInput)?;
    if value.chars().any(char::is_control) {
        return Err(CommandError::InvalidInput);
    };
    Ok(value)
}
pub fn change_value(c: &Change) -> Value {
    match c {
        Change::User {
            expected_revision,
            action,
            expires_at,
            reason,
            ..
        } => {
            json!({"expected_revision":expected_revision,"action":action.as_str(),"expires_at":expires_at.map(|v|v.to_rfc3339_opts(chrono::SecondsFormat::AutoSi,true)),"reason":reason})
        }
        Change::Content {
            kind,
            target,
            expected_revision,
            action,
            reason,
        } => {
            json!({"expected_revision":expected_revision,"target_type":kind.as_str(),"target_id":target,"action":action.as_str(),"reason":reason})
        }
    }
}
fn validators() -> Result<&'static BTreeMap<&'static str, jsonschema::Validator>, CommandError> {
    static V: OnceLock<Result<BTreeMap<&'static str, jsonschema::Validator>, ()>> = OnceLock::new();
    V.get_or_init(||{
        let source:Value=serde_json::from_str(include_str!("../../../../../contracts/generated/mcr-domain.openapi.json")).map_err(|_|())?;
        ["Domain_admin_ModerateUser","Domain_admin_ModerateContent","Domain_admin_UserQuery","Domain_admin_ContentQuery","Domain_admin_AuditQuery","Domain_admin_UserReview","Domain_admin_ContentReview","Domain_admin_UserReviewList","Domain_admin_ContentReviewList","Domain_admin_AuditList","Domain_common_CommandReceipt"].into_iter().map(|name|{
            if source["components"]["schemas"][name].is_null(){return Err(());}
            let schema=json!({"$schema":"https://json-schema.org/draft/2020-12/schema","$ref":format!("#/components/schemas/{name}"),"components":source["components"]});
            Ok((name,jsonschema::options().should_validate_formats(true).build(&schema).map_err(|_|())?))
        }).collect()
    }).as_ref().map_err(|_|CommandError::Unavailable)
}
pub fn warm_contracts() -> Result<(), CommandError> {
    validators().map(|_| ())
}
pub fn validate_value(schema: &str, value: &Value) -> Result<(), CommandError> {
    if validators()?
        .get(schema)
        .ok_or(CommandError::Unavailable)?
        .is_valid(value)
    {
        Ok(())
    } else {
        Err(CommandError::InvalidInput)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_routes_queries_and_required_fields() {
        warm_contracts().unwrap();
        let event = Uuid::new_v4();
        let target = Uuid::new_v4();
        let key = Uuid::new_v4().to_string();
        let root = format!("/api/concert/events/{event}/admin");
        let post = |v: Value| {
            parse(
                event,
                "POST",
                &format!("{root}/users/{target}/moderation"),
                &serde_json::to_vec(&v).unwrap(),
                "application/json",
                &key,
            )
        };
        let good = json!({"expected_revision":1,"action":"SUSPEND","expires_at":null,"reason":"Private reason"});
        assert!(post(good.clone()).is_ok());
        for field in ["reason", "expires_at", "expected_revision", "action"] {
            let mut v = good.clone();
            v.as_object_mut().unwrap().remove(field);
            assert!(post(v).is_err());
        }
        let mut unknown = good;
        unknown["role"] = json!("Operator");
        assert!(post(unknown).is_err());
        let get = |suffix: &str| parse(event, "GET", &format!("{root}/{suffix}"), b"", "", "");
        assert!(get(&format!("users/{target}")).is_err()); // No invented get-user operation.
        for suffix in [
            "users?limit=101",
            "users?limit=0",
            "audit?role=Operator",
            "content?target_type=SECRET",
            "audit?limit=1&limit=2",
            "users?cursor=%zz",
            "audit?from=2026-01-02T00%3A00%3A00Z&until=2026-01-01T00%3A00%3A00Z",
        ] {
            assert!(get(suffix).is_err(), "{suffix}");
        }
        assert_eq!(get("users").unwrap().query.limit, 20);
        assert_eq!(
            get("audit?from=2026-01-01T01%3A00%3A00%2B01%3A00")
                .unwrap()
                .query
                .filters["from"],
            "2026-01-01T00:00:00Z"
        );
        assert!(parse(Uuid::new_v4(), "GET", &format!("{root}/audit"), b"", "", "").is_err());
        assert!(parse(event, "GET", &format!("{root}/audit"), b"", "", &key).is_err());
        let duplicate=br#"{"expected_revision":1,"action":"SUSPEND","action":"UNSUSPEND","expires_at":null,"reason":"r"}"#;
        assert!(parse(
            event,
            "POST",
            &format!("{root}/users/{target}/moderation"),
            duplicate,
            "application/json",
            &key
        )
        .is_err());
    }
    #[test]
    fn content_kinds_and_audit_projection_are_closed() {
        let event = Uuid::new_v4();
        let target = Uuid::new_v4();
        for kind in ["POST", "COMMENT", "PARTY", "GIVEAWAY", "INTRO"] {
            let body = json!({"expected_revision":1,"target_type":kind,"target_id":target,"action":"RESTORE","reason":"Reviewed"});
            let r = parse(
                event,
                "POST",
                &format!("/api/concert/events/{event}/admin/content/moderation"),
                &serde_json::to_vec(&body).unwrap(),
                "application/json",
                &Uuid::new_v4().to_string(),
            )
            .unwrap();
            assert_eq!(r.action, Action::ModerateContent);
        }
        let row = json!({"audit_id":Uuid::new_v4(),"event_id":event,"actor_id":Uuid::new_v4(),"operation_id":"moderateConcertContent","target_id":target,"outcome":"PENDING","receipt_id":Uuid::new_v4(),"created_at":Utc::now()});
        assert!(validate_value(
            "Domain_admin_AuditList",
            &json!({"items":[row.clone()],"next_cursor":null})
        )
        .is_ok());
        for field in ["reason", "evidence", "reporter_id", "native_identity"] {
            let mut v = row.clone();
            v[field] = json!("private");
            assert!(validate_value(
                "Domain_admin_AuditList",
                &json!({"items":[v],"next_cursor":null})
            )
            .is_err());
        }
    }
}
