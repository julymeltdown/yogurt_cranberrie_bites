use super::{model, proto, OperationBudget};
use async_trait::async_trait;
use platform::{
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
    grpc_tls::TlsMaterial,
};
use std::collections::BTreeMap;
use tonic::{transport::Channel, Request, Status};

/// Only configured owning producers can supply terminal evidence. A producer
/// must resolve its real transaction/receipt and seal its response scope itself.
#[async_trait]
pub trait SettlementResolver: Send + Sync {
    fn configured(&self, producer: proto::Producer) -> bool;
    async fn resolve_scope(
        &self,
        query: proto::ResolveScopeRequest,
        budget: &OperationBudget,
    ) -> Result<proto::ResolveScopeResponse, Status>;
    async fn resolve_transaction(
        &self,
        query: proto::ResolveTransactionRequest,
        budget: &OperationBudget,
    ) -> Result<proto::ResolveTransactionResponse, Status>;
}
/// No optional TLS, caller-supplied host, alternate store or guessed endpoint.
pub struct RpcSettlementResolver {
    channels: BTreeMap<i32, Channel>,
    identity: ServiceIdentityClientInterceptor,
}
impl RpcSettlementResolver {
    pub fn new(
        tls: &TlsMaterial,
        credential: &str,
        endpoints: &[(proto::Producer, String)],
    ) -> Result<Self, Status> {
        let identity = ServiceIdentityClientInterceptor::new(super::owner::WORKLOAD, credential)
            .map_err(|_| model::unavailable())?;
        let mut channels = BTreeMap::new();
        for (producer, url) in endpoints {
            let peer = match producer {
                proto::Producer::Concert => Workload::Concert,
                proto::Producer::Post => Workload::Post,
                proto::Producer::Profile => Workload::Profile,
                proto::Producer::Notification => Workload::Notification,
                _ => return Err(model::invalid()),
            };
            if !url.starts_with("https://") || channels.contains_key(&(*producer as i32)) {
                return Err(model::invalid());
            }
            channels.insert(
                *producer as i32,
                tls.endpoint(url, peer)
                    .map_err(|_| model::unavailable())?
                    .connect_lazy(),
            );
        }
        if !channels.contains_key(&(proto::Producer::Concert as i32)) {
            return Err(model::unavailable());
        }
        Ok(Self { channels, identity })
    }
    fn client(
        &self,
        producer: proto::Producer,
    ) -> Result<
        proto::producer_settlement_service_client::ProducerSettlementServiceClient<
            tonic::service::interceptor::InterceptedService<
                Channel,
                ServiceIdentityClientInterceptor,
            >,
        >,
        Status,
    > {
        let channel = self
            .channels
            .get(&(producer as i32))
            .ok_or_else(model::unavailable)?
            .clone();
        Ok(proto::producer_settlement_service_client::ProducerSettlementServiceClient::with_interceptor(channel,self.identity.clone()).max_encoding_message_size(64*1024).max_decoding_message_size(64*1024))
    }
}
#[async_trait]
impl SettlementResolver for RpcSettlementResolver {
    fn configured(&self, producer: proto::Producer) -> bool {
        self.channels.contains_key(&(producer as i32))
    }
    async fn resolve_scope(
        &self,
        query: proto::ResolveScopeRequest,
        budget: &OperationBudget,
    ) -> Result<proto::ResolveScopeResponse, Status> {
        let mut request = Request::new(query);
        request.set_timeout(budget.remaining()?);
        budget
            .run(async {
                self.client(proto::Producer::Concert)?
                    .resolve_scope(request)
                    .await
                    .map(tonic::Response::into_inner)
                    .map_err(|_| model::unavailable())
            })
            .await
    }
    async fn resolve_transaction(
        &self,
        query: proto::ResolveTransactionRequest,
        budget: &OperationBudget,
    ) -> Result<proto::ResolveTransactionResponse, Status> {
        let producer = proto::Producer::try_from(
            query
                .transaction
                .as_ref()
                .ok_or_else(model::invalid)?
                .producer,
        )
        .map_err(|_| model::invalid())?;
        let mut request = Request::new(query);
        request.set_timeout(budget.remaining()?);
        budget
            .run(async {
                self.client(producer)?
                    .resolve_transaction(request)
                    .await
                    .map(tonic::Response::into_inner)
                    .map_err(|_| model::unavailable())
            })
            .await
    }
}
