use super::{super::grpc::proto, budget::ReplayBudget, model::*, validation};
use platform::{
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
    grpc_tls::TlsMaterial,
};
use prost::Message;
use std::time::Instant;
use tonic::{transport::Channel, Request, Status};
use uuid::Uuid;

/// A source pins its expected TLS workload and method family. It never observes
/// another owner's database, performs World effects or advances a durable cursor.
#[derive(Clone)]
pub struct InvalidationSource {
    owner: InvalidationOwner,
    channel: Channel,
    identity: ServiceIdentityClientInterceptor,
}
impl InvalidationSource {
    pub fn new(
        owner: InvalidationOwner,
        endpoint: &str,
        service_credential: &str,
        tls: &TlsMaterial,
    ) -> Result<Self, ReadError> {
        let uri: endpoint_uri::Uri = endpoint
            .parse()
            .map_err(|_| ReadError::InvalidConfiguration)?;
        if uri.scheme_str() != Some("https")
            || uri.host().is_none()
            || service_credential.trim().is_empty()
        {
            return Err(ReadError::InvalidConfiguration);
        }
        let channel = tls
            .endpoint(endpoint, owner.workload())
            .map_err(|_| ReadError::InvalidConfiguration)?
            .connect_lazy();
        let identity = ServiceIdentityClientInterceptor::new(Workload::Concert, service_credential)
            .map_err(|_| ReadError::InvalidConfiguration)?;
        Ok(Self {
            owner,
            channel,
            identity,
        })
    }
    pub fn owner(&self) -> InvalidationOwner {
        self.owner
    }
    pub async fn read_head(
        &self,
        consumer: Uuid,
        budget: &ReplayBudget,
    ) -> Result<VerifiedOwnerHead, ReadError> {
        let stop = budget.operation_stop()?;
        if consumer.is_nil() {
            return Err(ReadError::InvalidInput);
        }
        let query = proto::ReadInvalidationHeadRequest {
            consumer_id: consumer.to_string(),
            request_id: Uuid::new_v4().to_string(),
        };
        budget.run(stop,async {
            let request=request(query.clone(),stop)?;
            let response=match self.owner {
                InvalidationOwner::Auth=>{
                    let mut client=proto::auth_concert_invalidation_service_client::AuthConcertInvalidationServiceClient::with_interceptor(self.channel.clone(),self.identity.clone()).max_encoding_message_size(MAX_REQUEST_BYTES).max_decoding_message_size(MAX_RESPONSE_BYTES);
                    client.read_head(request).await
                },
                InvalidationOwner::Friend=>{
                    let mut client=proto::friend_concert_invalidation_service_client::FriendConcertInvalidationServiceClient::with_interceptor(self.channel.clone(),self.identity.clone()).max_encoding_message_size(MAX_REQUEST_BYTES).max_decoding_message_size(MAX_RESPONSE_BYTES);
                    client.read_head(request).await
                },
            }.map_err(status)?;
            budget.check(stop)?;let verified=validation::head(self.owner,query,response)?;budget.check(stop)?;Ok(verified)
        }).await
    }
    pub async fn read_after(
        &self,
        consumer: Uuid,
        after: OwnerCursor,
        limit: u16,
        budget: &ReplayBudget,
    ) -> Result<VerifiedOwnerPage, ReadError> {
        let stop = budget.operation_stop()?;
        if consumer.is_nil() || !(1..=MAX_PAGE_SIZE).contains(&limit) {
            return Err(ReadError::InvalidInput);
        }
        if after.owner() != self.owner {
            return Err(ReadError::OwnerMismatch);
        }
        let query = proto::ReadInvalidationsRequest {
            consumer_id: consumer.to_string(),
            request_id: Uuid::new_v4().to_string(),
            after: Some(proto::InvalidationCursor {
                owner_epoch: after.epoch().to_string(),
                sequence: after.sequence(),
            }),
            limit: u32::from(limit),
        };
        budget.run(stop,async {
            let request=request(query.clone(),stop)?;
            let response=match self.owner {
                InvalidationOwner::Auth=>{
                    let mut client=proto::auth_concert_invalidation_service_client::AuthConcertInvalidationServiceClient::with_interceptor(self.channel.clone(),self.identity.clone()).max_encoding_message_size(MAX_REQUEST_BYTES).max_decoding_message_size(MAX_RESPONSE_BYTES);
                    client.read_after(request).await
                },
                InvalidationOwner::Friend=>{
                    let mut client=proto::friend_concert_invalidation_service_client::FriendConcertInvalidationServiceClient::with_interceptor(self.channel.clone(),self.identity.clone()).max_encoding_message_size(MAX_REQUEST_BYTES).max_decoding_message_size(MAX_RESPONSE_BYTES);
                    client.read_after(request).await
                },
            }.map_err(status)?;
            budget.check(stop)?;let verified=validation::page(self.owner,query,response)?;budget.check(stop)?;Ok(verified)
        }).await
    }
}
// Use the retained canonical HTTP URI parser, with no new direct dependency.
mod endpoint_uri {
    pub type Uri = tonic::codegen::http::Uri;
}
fn request<T: Message>(message: T, stop: Instant) -> Result<Request<T>, ReadError> {
    let remaining = stop.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(ReadError::DeadlineExceeded);
    }
    if message.encoded_len() > MAX_REQUEST_BYTES {
        return Err(ReadError::InvalidInput);
    }
    let mut request = Request::new(message);
    request.set_timeout(remaining);
    Ok(request)
}
fn status(value: Status) -> ReadError {
    match value.code() {
        tonic::Code::Cancelled => ReadError::Cancelled,
        tonic::Code::DeadlineExceeded => ReadError::DeadlineExceeded,
        tonic::Code::Unauthenticated => ReadError::Unauthenticated,
        tonic::Code::PermissionDenied => ReadError::PermissionDenied,
        tonic::Code::FailedPrecondition => ReadError::CheckpointMismatch,
        tonic::Code::OutOfRange | tonic::Code::ResourceExhausted => ReadError::ResponseTooLarge,
        tonic::Code::InvalidArgument | tonic::Code::DataLoss | tonic::Code::Internal => {
            ReadError::MalformedResponse
        }
        _ => ReadError::Unavailable,
    }
}
