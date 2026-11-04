//! Request-local authenticated owner facts; never a transferable admission proof.
use super::server::proto;
use crate::application::concert_identity::*;
use crate::config::AppState;
use platform::actor_delegation::{read_authority::VerifiedReadAuthority, VerifiedActor};
use platform::grpc_auth::{VerifiedCaller, Workload};
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub(super) async fn read(
    state: &AppState,
    request: Request<proto::ReadConcertAdmissionRequest>,
) -> Result<Response<proto::ReadConcertAdmissionResponse>, Status> {
    match request.extensions().get::<VerifiedCaller>() {
        Some(VerifiedCaller(Workload::Gateway)) => {}
        Some(_) => return Err(Status::permission_denied("Admission caller denied")),
        None => {
            return Err(Status::unauthenticated(
                "Verified admission caller required",
            ))
        }
    }
    let actor = request
        .extensions()
        .get::<VerifiedActor>()
        .ok_or_else(|| Status::unauthenticated("Verified admission actor required"))?
        .0;
    let authority = request
        .extensions()
        .get::<VerifiedReadAuthority>()
        .copied()
        .ok_or_else(|| Status::unauthenticated("Current read authority required"))?;
    let deadline = authority.deadline(ReadBudget::MAX_DURATION)?;
    let message = request.get_ref();
    let user = canonical_uuid(&message.user_id)?;
    let family = canonical_uuid(&message.session_family_id)?;
    if actor != user || authority.actor() != user {
        return Err(Status::permission_denied("Admission actor mismatch"));
    }
    let binding = message
        .binding
        .as_ref()
        .map(|binding| {
            ProviderBinding::parse(
                &binding.identity_id,
                &binding.provider,
                binding.binding_epoch,
            )
        })
        .transpose()
        .map_err(read_status)?;
    let input = AdmissionRead::new(user, family, binding).map_err(read_status)?;
    deadline.remaining(ReadBudget::MAX_DURATION, deadline.expires_micros())?;
    let cancellation = ReadCancellation::default();
    // Dropping the handler also signals any owner work retaining the token.
    // It does not promise instantaneous database cancellation.
    let _cancel_on_drop = CancelOnDrop(cancellation.clone());
    let budget = ReadBudget::new(deadline.stop().into_std(), cancellation).map_err(read_status)?;
    let result = tokio::time::timeout_at(
        deadline.stop(),
        state.concert_admission.read_current(input, &budget),
    )
    .await;
    // Check both signed wall expiry and the captured monotonic stop, even if a
    // noncooperative owner blocks a poll and returns success after the timeout.
    deadline.remaining(ReadBudget::MAX_DURATION, deadline.expires_micros())?;
    let facts = result
        .map_err(|_| Status::deadline_exceeded("Admission read deadline exceeded"))?
        .map_err(read_status)?;
    match (input.binding().is_some(), facts.binding) {
        (false, BindingEligibility::NotRequested)
        | (true, BindingEligibility::Bound | BindingEligibility::NotBound) => {}
        _ => return Err(Status::unavailable("Admission owner unavailable")),
    }
    let response = proto::ReadConcertAdmissionResponse {
        observed_at_unix_micros: facts.observed_at.timestamp_micros(),
        account_active: facts.account == AccountEligibility::Active,
        session_live: facts.session == SessionEligibility::Live,
        binding_state: match facts.binding {
            BindingEligibility::NotRequested => {
                proto::read_concert_admission_response::BindingState::BindingNotRequested
            }
            BindingEligibility::Bound => {
                proto::read_concert_admission_response::BindingState::BindingBound
            }
            BindingEligibility::NotBound => {
                proto::read_concert_admission_response::BindingState::BindingNotBound
            }
        } as i32,
    };
    let mut response = Response::new(response);
    response.metadata_mut().insert(
        "cache-control",
        tonic::metadata::MetadataValue::from_static("no-store"),
    );
    deadline.remaining(ReadBudget::MAX_DURATION, deadline.expires_micros())?;
    Ok(response)
}

struct CancelOnDrop(ReadCancellation);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

#[allow(clippy::result_large_err)]
fn canonical_uuid(value: &str) -> Result<Uuid, Status> {
    let id = Uuid::parse_str(value)
        .map_err(|_| Status::invalid_argument("Invalid admission identity"))?;
    if id.is_nil() || id.hyphenated().to_string() != value {
        return Err(Status::invalid_argument("Invalid admission identity"));
    }
    Ok(id)
}

fn read_status(error: ReadError) -> Status {
    match error {
        ReadError::InvalidInput => Status::invalid_argument("Invalid admission input"),
        ReadError::Cancelled => Status::cancelled("Admission read cancelled"),
        ReadError::DeadlineExceeded => {
            Status::deadline_exceeded("Admission read deadline exceeded")
        }
        ReadError::ConsentRequired => Status::failed_precondition("CONSENT_REQUIRED"),
        ReadError::PolicyUnavailable => Status::unavailable("POLICY_UNAVAILABLE"),
        ReadError::Unavailable => Status::unavailable("Admission owner unavailable"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_identity_shapes_and_safe_errors() {
        for value in [
            "",
            "private-identity",
            "00000000-0000-0000-0000-000000000000",
            "123456781234123412341234567890ab",
            "12345678-1234-1234-1234-1234567890AB",
        ] {
            let error = canonical_uuid(value).unwrap_err();
            assert_eq!(error.code(), tonic::Code::InvalidArgument);
            assert_eq!(error.message(), "Invalid admission identity");
        }
        assert!(canonical_uuid(&Uuid::new_v4().to_string()).is_ok());
    }
    #[test]
    fn all_owner_errors_have_fixed_statuses() {
        for (error, code, message) in [
            (
                ReadError::InvalidInput,
                tonic::Code::InvalidArgument,
                "Invalid admission input",
            ),
            (
                ReadError::Cancelled,
                tonic::Code::Cancelled,
                "Admission read cancelled",
            ),
            (
                ReadError::DeadlineExceeded,
                tonic::Code::DeadlineExceeded,
                "Admission read deadline exceeded",
            ),
            (
                ReadError::Unavailable,
                tonic::Code::Unavailable,
                "Admission owner unavailable",
            ),
        ] {
            let status = read_status(error);
            assert_eq!(status.code(), code);
            assert_eq!(status.message(), message);
        }
    }
}
