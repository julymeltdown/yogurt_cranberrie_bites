//! Closed response disclosure allowlist, independent of the producer implementation.
use super::{proto, Operation};
use axum::http::StatusCode;
use serde_json::Value;
use tonic::{Code, Status};
use uuid::Uuid;
fn invalid() -> Status {
    Status::unavailable("Participant response unavailable")
}
fn keys(v: &Value, names: &[&str]) -> bool {
    v.as_object()
        .is_some_and(|o| o.len() == names.len() && names.iter().all(|n| o.contains_key(*n)))
}
fn revision(v: &Value) -> bool {
    v.as_i64()
        .is_some_and(|n| (1..=9_007_199_254_740_991).contains(&n))
}
fn text(v: &Value, min: usize, max: usize) -> bool {
    v.as_str()
        .is_some_and(|s| (min..=max).contains(&s.chars().count()))
}
fn asset(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        !s.is_empty()
            && s.len() <= 64
            && s.as_bytes()[0].is_ascii_lowercase()
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    })
}
fn avatar(v: &Value) -> bool {
    if !keys(v, &["revision", "config"]) || !revision(&v["revision"]) {
        return false;
    }
    let c = &v["config"];
    if c["kind"] != "avatar_config" {
        return false;
    }
    if c["contract_version"] == "2.0.0" {
        if c["manifest_id"] != "mcr_selected_motion_reviewed_20261104" || c["manifest_version"] != 2
        {
            return false;
        }
        if c["representation"] == "motion_preset" {
            return keys(
                c,
                &[
                    "contract_version",
                    "kind",
                    "manifest_id",
                    "manifest_version",
                    "representation",
                    "preset_id",
                ],
            ) && matches!(
                c["preset_id"].as_str(),
                Some(
                    "motion_v2_photo01_front_01"
                        | "motion_v2_photo01_front_02"
                        | "motion_v2_photo01_front_03"
                        | "motion_v2_photo01_front_04"
                        | "motion_v2_photo01_front_05"
                        | "motion_v2_photo01_front_06"
                )
            );
        }
        let items = &c["item_ids"];
        return c["representation"] == "motion_layered"
            && keys(
                c,
                &[
                    "contract_version",
                    "kind",
                    "manifest_id",
                    "manifest_version",
                    "representation",
                    "item_ids",
                ],
            )
            && keys(items, &["body", "eyes", "hair", "outfit", "accessory"])
            && matches!(
                items["body"].as_str(),
                Some(
                    "motion_v2_body_0"
                        | "motion_v2_body_1"
                        | "motion_v2_body_2"
                        | "motion_v2_body_3"
                        | "motion_v2_body_4"
                        | "motion_v2_body_5"
                        | "motion_v2_body_6"
                        | "motion_v2_body_7"
                )
            )
            && matches!(
                items["eyes"].as_str(),
                Some(
                    "motion_v2_eyes_0"
                        | "motion_v2_eyes_1"
                        | "motion_v2_eyes_2"
                        | "motion_v2_eyes_3"
                )
            )
            && matches!(
                items["hair"].as_str(),
                Some(
                    "motion_v2_hair_0"
                        | "motion_v2_hair_1"
                        | "motion_v2_hair_2"
                        | "motion_v2_hair_3"
                        | "motion_v2_hair_4"
                        | "motion_v2_hair_5"
                )
            )
            && (items["outfit"].is_null()
                || matches!(
                    items["outfit"].as_str(),
                    Some(
                        "motion_v2_outfit_0"
                            | "motion_v2_outfit_1"
                            | "motion_v2_outfit_2"
                            | "motion_v2_outfit_3"
                            | "motion_v2_outfit_4"
                            | "motion_v2_outfit_5"
                    )
                ))
            && (items["accessory"].is_null()
                || matches!(
                    items["accessory"].as_str(),
                    Some(
                        "motion_v2_accessory_0"
                            | "motion_v2_accessory_1"
                            | "motion_v2_accessory_2"
                            | "motion_v2_accessory_3"
                    )
                ));
    }
    if c["contract_version"] == "1.1.0" {
        keys(
            c,
            &[
                "contract_version",
                "kind",
                "manifest_id",
                "manifest_version",
                "representation",
                "preset_id",
            ],
        ) && c["manifest_id"] == "mcr_selected_static_20261103"
            && c["manifest_version"] == 1
            && c["representation"] == "whole_character"
            && matches!(
                c["preset_id"].as_str(),
                Some(
                    "photo01_front_01"
                        | "photo01_front_02"
                        | "photo01_front_03"
                        | "photo01_front_04"
                        | "photo01_front_05"
                        | "photo01_front_06"
                )
            )
    } else {
        let items = &c["item_ids"];
        keys(
            c,
            &[
                "contract_version",
                "kind",
                "manifest_id",
                "manifest_version",
                "item_ids",
                "skin_tone_id",
            ],
        ) && c["contract_version"] == "1.0.0"
            && asset(&c["manifest_id"])
            && revision(&c["manifest_version"])
            && asset(&c["skin_tone_id"])
            && keys(items, &["body", "eyes", "hair", "outfit", "accessory"])
            && ["body", "eyes", "hair", "outfit"]
                .iter()
                .all(|k| asset(&items[*k]))
            && (items["accessory"].is_null() || asset(&items["accessory"]))
    }
}
pub fn validate_document(
    bytes: &[u8],
    operation: Operation,
    actor: Uuid,
    event: Uuid,
) -> Result<(), Status> {
    if bytes.is_empty() || bytes.len() > 16384 {
        return Err(invalid());
    }
    let v: Value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if operation == Operation::Avatar {
        return if avatar(&v) { Ok(()) } else { Err(invalid()) };
    }
    let profile = &v["profile"];
    let prefs = &v["preferences"];
    if !keys(
        &v,
        &[
            "event_id",
            "user_id",
            "revision",
            "profile",
            "preferences",
            "age_status",
        ],
    ) || v["event_id"] != event.to_string()
        || v["user_id"] != actor.to_string()
        || !revision(&v["revision"])
        || !keys(profile, &["user_id", "display_name", "bio", "avatar"])
        || profile["user_id"] != actor.to_string()
        || !text(&profile["display_name"], 1, 40)
        || !(profile["bio"].is_null() || text(&profile["bio"], 0, 160))
        || !avatar(&profile["avatar"])
        || !keys(
            prefs,
            &["theme", "low_power", "reduced_motion", "sound_enabled"],
        )
        || !matches!(
            prefs["theme"].as_str(),
            Some("REVENGE" | "PARADE" | "DANGER")
        )
        || !["low_power", "reduced_motion", "sound_enabled"]
            .iter()
            .all(|k| prefs[*k].is_boolean())
        || !matches!(
            v["age_status"].as_str(),
            Some("UNKNOWN" | "VERIFIED" | "EXPIRED" | "PROVIDER_UNAVAILABLE")
        )
    {
        return Err(invalid());
    }
    Ok(())
}
pub fn failure_status(f: &proto::ParticipantFailure) -> Result<(StatusCode, &'static str), Status> {
    let pair = match (f.grpc_code, f.core_reason.as_str()) {
        (x, "INVALID_INPUT") if x == Code::InvalidArgument as u32 => {
            (StatusCode::BAD_REQUEST, "INVALID_INPUT")
        }
        (x, "AUTHENTICATION_REQUIRED") if x == Code::Unauthenticated as u32 => {
            (StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED")
        }
        (x, "PERMISSION_DENIED") if x == Code::PermissionDenied as u32 => {
            (StatusCode::FORBIDDEN, "PERMISSION_DENIED")
        }
        (x, "RESOURCE_UNAVAILABLE") if x == Code::NotFound as u32 => {
            (StatusCode::NOT_FOUND, "RESOURCE_UNAVAILABLE")
        }
        (x, "VERSION_CONFLICT") if x == Code::Aborted as u32 => {
            (StatusCode::CONFLICT, "VERSION_CONFLICT")
        }
        (x, "EVENT_READ_ONLY") if x == Code::FailedPrecondition as u32 => {
            (StatusCode::CONFLICT, "EVENT_READ_ONLY")
        }
        (x, "DEPENDENCY_UNAVAILABLE") if x == Code::Unavailable as u32 => {
            (StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE")
        }
        (x, "DEADLINE_EXCEEDED") if x == Code::DeadlineExceeded as u32 => {
            (StatusCode::GATEWAY_TIMEOUT, "DEADLINE_EXCEEDED")
        }
        (x, "INTERNAL_ERROR") if x == Code::Internal as u32 => {
            (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR")
        }
        _ => return Err(invalid()),
    };
    Ok(pair)
}

#[cfg(test)]
mod reviewed_motion_v2_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn reviewed_motion_v2_closed_response_fixtures_and_immutable_candidate_refusal() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for (folder, expected, count) in [
            ("reviewed/valid", true, 15),
            ("reviewed/invalid", false, 16),
            ("valid", false, 15),
        ] {
            let mut seen = 0;
            for entry in std::fs::read_dir(
                root.join("contracts/fixtures/assets/motion-v2")
                    .join(folder),
            )
            .unwrap()
            {
                let config: Value =
                    serde_json::from_slice(&std::fs::read(entry.unwrap().path()).unwrap()).unwrap();
                assert_eq!(
                    avatar(&json!({"revision":1,"config":config})),
                    expected,
                    "{folder}: {config}"
                );
                seen += 1;
            }
            assert_eq!(seen, count);
        }
    }
    #[test]
    fn reviewed_motion_v2_response_requires_exact_header_keys_and_nullable_inventory() {
        let config = json!({"contract_version":"2.0.0","kind":"avatar_config","manifest_id":"mcr_selected_motion_reviewed_20261104","manifest_version":2,"representation":"motion_layered","item_ids":{"body":"motion_v2_body_7","eyes":"motion_v2_eyes_3","hair":"motion_v2_hair_5","outfit":null,"accessory":null}});
        assert!(avatar(&json!({"revision":1,"config":config})));
        for pointer in [
            "/item_ids/body",
            "/item_ids/eyes",
            "/item_ids/hair",
            "/item_ids/outfit",
            "/item_ids/accessory",
        ] {
            let mut bad = config.clone();
            *bad.pointer_mut(pointer).unwrap() = json!("unknown");
            assert!(!avatar(&json!({"revision":1,"config":bad})), "{pointer}");
        }
        for key in ["body", "eyes", "hair", "outfit", "accessory"] {
            let mut bad = config.clone();
            bad["item_ids"].as_object_mut().unwrap().remove(key);
            assert!(
                !avatar(&json!({"revision":1,"config":bad})),
                "missing {key}"
            );
        }
        for (pointer, value) in [
            ("/manifest_id", json!("mcr_selected_motion_20261103")),
            ("/manifest_version", json!(1)),
            ("/manifest_version", json!(2.0)),
            ("/contract_version", json!("1.0.0")),
            ("/representation", json!("layered")),
        ] {
            let mut bad = config.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(!avatar(&json!({"revision":1,"config":bad})), "{pointer}");
        }
        let mut extra = config.clone();
        extra["skin_tone_id"] = json!("tone_0");
        assert!(!avatar(&json!({"revision":1,"config":extra})));
        assert!(!avatar(
            &json!({"revision":1,"config":config,"private":"hidden"})
        ));
    }
    #[test]
    fn reviewed_motion_v2_branch_preserves_legacy_whole_and_layered_response_allowlists() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let document: Value = serde_json::from_slice(
            &std::fs::read(root.join("contracts/fixtures/core/bootstrap-open.json")).unwrap(),
        )
        .unwrap();
        assert!(avatar(&document["avatar"]));
        let manifest: Value = serde_json::from_slice(
            &std::fs::read(
                root.join("contracts/fixtures/assets/selected-static/avatar-manifest.json"),
            )
            .unwrap(),
        )
        .unwrap();
        for preset in manifest["presets"].as_array().unwrap() {
            assert!(avatar(&json!({"revision":1,"config":preset["config"]})));
        }
    }
}
