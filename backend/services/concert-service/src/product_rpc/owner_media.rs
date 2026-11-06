//! Transport mapping only; canonical owner fences and local policy stay in the
//! media runtime. No fixture or successful fallback authority is composed here.
use super::{proto, Operation, ProductOwner, VerifiedProduct};
use crate::media_runtime::{
    OwnerMediaIdentity, OwnerMediaReply, OwnerMediaRequest, OwnerMediaRuntime,
};
use concert_application::commands::CommandError;
use tonic::{Response, Status};

fn failure(context: &VerifiedProduct, error: CommandError) -> proto::ProductResponse {
    let (status, reason) = match error {
        CommandError::InvalidInput => (400, "INVALID_INPUT"),
        CommandError::Denied => (403, "PERMISSION_DENIED"),
        CommandError::Unavailable => (503, "DEPENDENCY_UNAVAILABLE"),
        CommandError::Conflict => (409, "IDEMPOTENCY_CONFLICT"),
        // Frozen C04 uses 409 for expired idempotency, not HTTP 410.
        CommandError::Expired => (409, "IDEMPOTENCY_EXPIRED"),
        CommandError::Capacity => (429, "RATE_LIMITED"),
        CommandError::InvalidState => (409, "INVALID_TRANSITION"),
    };
    let mut details = serde_json::json!({"reason":reason});
    if error == CommandError::Capacity {
        details["retry_after_ms"] = 1000.into();
    }
    proto::ProductResponse {
        status,
        delivery: None,
        content_expires_unix_micros: 0,
        content_type: "application/json".into(),
        body: serde_json::to_vec(&serde_json::json!({
            "code":format!("CONCERT_{status}"), "message":"Concert media request unavailable",
            "traceId":context.request().to_string(),
            "timestamp":chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true),
            "details":details,
        }))
        .expect("closed media error"),
    }
}

#[async_trait::async_trait]
impl ProductOwner for OwnerMediaRuntime {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        request: proto::ProductRequest,
    ) -> Result<Response<proto::ProductResponse>, Status> {
        let operation = match context.operation() {
            Operation::InitializeUpload => "initConcertUpload",
            Operation::UploadBytes(_) => "putConcertUploadBytes",
            Operation::FinalizeUpload(_) => "finalizeConcertUpload",
            Operation::MediaMetadata(_) => "getConcertMedia",
            Operation::PrivateContent(_) => "readConcertMedia",
            Operation::CommandStatus => "getConcertCommandStatus",
            Operation::Social(_)
            | Operation::Intro(_)
            | Operation::Notification(_)
            | Operation::Age(_)
            | Operation::Report(_)
            | Operation::Operations(_)
            | Operation::Wall(_)
            | Operation::Meeting(_)
            | Operation::Feed
            | Operation::Post(_)
            | Operation::Comments(_)
            | Operation::ListParties
            | Operation::GetParty(_)
            | Operation::CreateParty => {
                return Err(Status::unavailable("Product producer unavailable"));
            }
            Operation::ApplyParty(_)
            | Operation::DecidePartyApplication(_)
            | Operation::CancelPartyApplication(_)
            | Operation::RemovePartyMember(_, _)
            | Operation::CancelParty(_) => {
                return Err(Status::unavailable("Product producer unavailable"));
            }
        };
        context.check()?;
        let input = OwnerMediaRequest::from_verified(
            OwnerMediaIdentity {
                actor: context.actor(),
                event: context.event(),
                session_family: context.session(),
                request_id: context.request(),
                deadline: context.deadline(),
            },
            operation,
            &request.http_method,
            &request.http_target,
            &request.content_type,
            &request.idempotency_key,
            request.http_body,
        );
        let input = match input {
            Ok(input) => input,
            Err(error) => return Ok(Response::new(failure(context, error))),
        };
        let result = OwnerMediaRuntime::dispatch(self, input).await;
        context.check()?;
        let body = match result {
            Ok(OwnerMediaReply::Json { status, body }) => proto::ProductResponse {
                status: status.into(),
                delivery: None,
                content_expires_unix_micros: 0,
                content_type: "application/json".into(),
                body: serde_json::to_vec(&body)
                    .map_err(|_| Status::internal("Invalid media representation"))?,
            },
            Ok(OwnerMediaReply::Image(image)) => proto::ProductResponse {
                status: 200,
                delivery: None,
                content_expires_unix_micros: 0,
                content_type: image.metadata.mime.as_str().into(),
                body: image.bytes,
            },
            Err(error) => failure(context, error),
        };
        Ok(Response::new(body))
    }
}
