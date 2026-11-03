//! C04 reveal response adapter. Gateway/root route wiring is integrator-owned.
//! actor must come from verified server context; the service rechecks fenced authority.
use super::{
    social::{SocialEligibility, SocialService},
    FencedRelationshipAuthority,
};
use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{SecondsFormat, Utc};
use concert_domain::{
    relationship::{social::Platform, RelationshipError},
    ParticipantKey,
};
use uuid::Uuid;

pub async fn reveal_response<A: FencedRelationshipAuthority, E: SocialEligibility>(
    service: &SocialService<A, E>,
    actor: ParticipantKey,
    request: Uuid,
    platform: Platform,
) -> Response {
    let result = service.reveal(actor, request, platform).await;
    let mut response=match result {
  Ok(value)=>(StatusCode::OK,Json(serde_json::json!({
   "request_id":value.request_id.to_string(),"platform":value.context.platform.as_str(),
   "owner_id":value.context.owner.uuid().to_string(),"handle_version":value.context.version.get(),
   "handle":value.handle.expose_for_reveal(),"grant_revision":value.grant_revision.get(),
   "expires_at":value.expires_at.to_rfc3339_opts(SecondsFormat::Millis,true)
  }))).into_response(),
  Err(error)=>{
   let (status,reason)=match error{RelationshipError::Denied|RelationshipError::Stale=>(StatusCode::NOT_FOUND,"RESOURCE_UNAVAILABLE"),RelationshipError::Unavailable=>(StatusCode::SERVICE_UNAVAILABLE,"DEPENDENCY_UNAVAILABLE")};
   (status,Json(serde_json::json!({"code":format!("GATEWAY_{}",status.as_u16()),"message":"The requested action is unavailable.","traceId":Uuid::new_v4().to_string(),"timestamp":Utc::now().to_rfc3339_opts(SecondsFormat::Millis,true),"details":{"reason":reason}}))).into_response()
  }
 };
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}
