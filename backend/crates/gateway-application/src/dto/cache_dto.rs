//! A response-cache transfer contains an already allocated JSON value.
//! This record does not imply a retained or process-wide memory bound.
#[derive(Clone, PartialEq)]
pub struct CachePayloadDto {
    pub value: serde_json::Value,
}
impl std::fmt::Debug for CachePayloadDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachePayloadDto").finish_non_exhaustive()
    }
}
impl From<serde_json::Value> for CachePayloadDto {
    fn from(value: serde_json::Value) -> Self {
        Self { value }
    }
}
impl From<CachePayloadDto> for serde_json::Value {
    fn from(value: CachePayloadDto) -> Self {
        value.value
    }
}
