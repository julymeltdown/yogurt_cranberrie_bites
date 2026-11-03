//! Closed bounds and service-side DTO shaping for frozen C04 introduction
//! objects. These functions only shape already-authorized values; public output
//! never carries bounds, expiry or a provider/claim.
use super::*;

pub struct Query {
    pub limit: u16,
    pub cursor: Option<String>,
    pub q: Option<String>,
}
impl Query {
    pub fn validate(
        limit: Option<u16>,
        cursor: Option<String>,
        q: Option<String>,
    ) -> Result<Self, IntroError> {
        let limit = limit.unwrap_or(20);
        let cursor = cursor.filter(|value| !value.is_empty());
        let q = q.map(|value| value.trim().to_owned());
        if !(1..=INTRO_PAGE_MAX).contains(&limit)
            || cursor
                .as_ref()
                .is_some_and(|value| value.len() > INTRO_CURSOR_MAX)
            || q.as_ref().is_some_and(|v| {
                v.is_empty()
                    || v.chars().count() > INTRO_QUERY_MAX
                    || v.chars().any(char::is_control)
            })
        {
            return Err(IntroError::Invalid);
        }
        Ok(Self { limit, cursor, q })
    }
}
