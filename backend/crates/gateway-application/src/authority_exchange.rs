//! Pure internal authority boundaries. No Debug/Serde for authority material.
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, thiserror::Error)]
pub enum AuthorityExchangeErrorDto {
    #[error("invalid authority exchange binding")]
    Invalid,
    #[error("authority exchange denied")]
    Denied,
    #[error("authority exchange unavailable")]
    Unavailable,
}
#[derive(Clone)]
pub struct ParentAuthorityDto {
    token: String,
    method: String,
    framed_body: Vec<u8>,
    operation_id: Option<Uuid>,
}
impl ParentAuthorityDto {
    pub fn new(
        token: String,
        method: String,
        framed_body: Vec<u8>,
        operation_id: Option<Uuid>,
    ) -> Result<Self, AuthorityExchangeErrorDto> {
        if token.is_empty()
            || token.len() > 4096
            || method.is_empty()
            || method.len() > 256
            || framed_body.len() > 4 * 1024 * 1024
            || operation_id.is_some_and(|id| id.is_nil())
        {
            return Err(AuthorityExchangeErrorDto::Invalid);
        }
        Ok(Self {
            token,
            method,
            framed_body,
            operation_id,
        })
    }
    pub fn token(&self) -> &str {
        &self.token
    }
    pub fn method(&self) -> &str {
        &self.method
    }
    pub fn body(&self) -> &[u8] {
        &self.framed_body
    }
    pub fn operation_id(&self) -> Option<Uuid> {
        self.operation_id
    }
}
pub struct AuthorityExchangeRequestDto {
    parent: ParentAuthorityDto,
    audience: String,
    method: String,
    framed_body: Vec<u8>,
    operation_id: Option<Uuid>,
    witness_id: Option<Uuid>,
}
impl AuthorityExchangeRequestDto {
    pub fn new(
        parent: ParentAuthorityDto,
        audience: String,
        method: String,
        framed_body: Vec<u8>,
        operation_id: Option<Uuid>,
        witness_id: Option<Uuid>,
    ) -> Result<Self, AuthorityExchangeErrorDto> {
        if !matches!(audience.as_str(), "friend" | "profile")
            || method.is_empty()
            || method.len() > 256
            || framed_body.len() > 4 * 1024 * 1024
            || operation_id.is_some_and(|id| id.is_nil())
            || witness_id.is_some_and(|id| id.is_nil())
        {
            return Err(AuthorityExchangeErrorDto::Invalid);
        }
        Ok(Self {
            parent,
            audience,
            method,
            framed_body,
            operation_id,
            witness_id,
        })
    }
    pub fn parent(&self) -> &ParentAuthorityDto {
        &self.parent
    }
    pub fn audience(&self) -> &str {
        &self.audience
    }
    pub fn method(&self) -> &str {
        &self.method
    }
    pub fn body(&self) -> &[u8] {
        &self.framed_body
    }
    pub fn operation_id(&self) -> Option<Uuid> {
        self.operation_id
    }
    pub fn witness_id(&self) -> Option<Uuid> {
        self.witness_id
    }
}
pub struct OwnerResourceLookupDto {
    parent: ParentAuthorityDto,
    witness_id: Uuid,
    targets: Vec<Uuid>,
}
impl OwnerResourceLookupDto {
    pub fn new(
        parent: ParentAuthorityDto,
        witness_id: Uuid,
        targets: Vec<Uuid>,
    ) -> Result<Self, AuthorityExchangeErrorDto> {
        if witness_id.is_nil()
            || targets.is_empty()
            || targets.len() > 100
            || targets.iter().any(|id| id.is_nil())
            || targets
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != targets.len()
        {
            return Err(AuthorityExchangeErrorDto::Invalid);
        }
        Ok(Self {
            parent,
            witness_id,
            targets,
        })
    }
    pub fn parent(&self) -> &ParentAuthorityDto {
        &self.parent
    }
    pub fn witness_id(&self) -> Uuid {
        self.witness_id
    }
    pub fn targets(&self) -> &[Uuid] {
        &self.targets
    }
}
pub struct OwnerResourceEvidenceDto(String);
impl OwnerResourceEvidenceDto {
    pub fn from_owner_response(value: String) -> Result<Self, AuthorityExchangeErrorDto> {
        if value.is_empty() || value.len() > 32 * 1024 {
            return Err(AuthorityExchangeErrorDto::Invalid);
        }
        Ok(Self(value))
    }
    pub fn token(&self) -> &str {
        &self.0
    }
}
pub struct IssuedAuthorityDto {
    token: String,
    expires_at: i64,
}
impl IssuedAuthorityDto {
    pub fn from_issuer(token: String, expires_at: i64) -> Result<Self, AuthorityExchangeErrorDto> {
        if token.is_empty() || token.len() > 4096 || expires_at <= 0 {
            return Err(AuthorityExchangeErrorDto::Invalid);
        }
        Ok(Self { token, expires_at })
    }
    pub fn into_parts(self) -> (String, i64) {
        (self.token, self.expires_at)
    }
}
#[async_trait]
pub trait OwnerResourceAuthorityPort: Send + Sync {
    async fn resolve(
        &self,
        request: &OwnerResourceLookupDto,
    ) -> Result<OwnerResourceEvidenceDto, AuthorityExchangeErrorDto>;
}
#[async_trait]
pub trait AuthorityExchangePort: Send + Sync {
    async fn exchange(
        &self,
        request: AuthorityExchangeRequestDto,
    ) -> Result<IssuedAuthorityDto, AuthorityExchangeErrorDto>;
}
#[derive(Clone)]
pub struct ExchangeAuthorityUseCase {
    issuer: Arc<dyn AuthorityExchangePort>,
}
impl ExchangeAuthorityUseCase {
    pub fn new(issuer: Arc<dyn AuthorityExchangePort>) -> Self {
        Self { issuer }
    }
    pub async fn exchange_authority(
        &self,
        request: AuthorityExchangeRequestDto,
    ) -> Result<IssuedAuthorityDto, AuthorityExchangeErrorDto> {
        self.issuer.exchange(request).await
    }
}
