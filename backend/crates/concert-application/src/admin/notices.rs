//! A valid source is not proof of review. The service requires an owning review fence.
use crate::commands::{CanonicalValue, CommandError};
use chrono::{DateTime, Utc};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeState {
    Draft,
    Published,
    Withdrawn,
}
impl NoticeState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Published => "PUBLISHED",
            Self::Withdrawn => "WITHDRAWN",
        }
    }
}
#[derive(Clone)]
pub struct NoticeInput {
    pub title: String,
    pub text: String,
    pub source_uri: String,
    pub confirmed_at: DateTime<Utc>,
    pub state: NoticeState,
}
impl NoticeInput {
    pub fn validate(&self, now: DateTime<Utc>) -> Result<(), CommandError> {
        if self.title.trim().is_empty()
            || self.title.chars().count() > 120
            || self.text.trim().is_empty()
            || self.text.chars().count() > 4000
            || !https_source(&self.source_uri)
            || self
                .source_uri
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
            || self.confirmed_at > now
            || self.title.chars().any(char::is_control)
            || self
                .text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\t' | '\r'))
        {
            return Err(CommandError::InvalidInput);
        }
        Ok(())
    }
    pub fn canonical(&self) -> Vec<(String, CanonicalValue)> {
        vec![
            ("title".into(), CanonicalValue::String(self.title.clone())),
            ("text".into(), CanonicalValue::String(self.text.clone())),
            (
                "source_uri".into(),
                CanonicalValue::String(self.source_uri.clone()),
            ),
            (
                "confirmed_at".into(),
                CanonicalValue::String(
                    self.confirmed_at
                        .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
                ),
            ),
            (
                "state".into(),
                CanonicalValue::String(self.state.as_str().into()),
            ),
        ]
    }
}

fn https_source(value: &str) -> bool {
    if value.len() > 2048
        || !value.is_ascii()
        || value.contains('\\')
        || value.matches('#').count() > 1
    {
        return false;
    }
    let Some(rest) = value.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return false;
    }
    let (host, port) = if authority.starts_with('[') {
        let Some(end) = authority.find(']') else {
            return false;
        };
        if authority[1..end].parse::<std::net::Ipv6Addr>().is_err() {
            return false;
        }
        (&authority[..=end], authority.get(end + 1..).unwrap_or(""))
    } else {
        let mut parts = authority.split(':');
        let host = parts.next().unwrap_or("");
        let port = parts.next();
        if parts.next().is_some()
            || host.is_empty()
            || !host.trim_end_matches('.').split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && label.as_bytes()[0].is_ascii_alphanumeric()
                    && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                    && label
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            })
        {
            return false;
        }
        (
            host,
            if port.is_some() {
                &authority[host.len()..]
            } else {
                ""
            },
        )
    };
    if host.is_empty()
        || !port.is_empty()
            && (!port.starts_with(':')
                || port.len() < 2
                || !port[1..].bytes().all(|c| c.is_ascii_digit()))
    {
        return false;
    }
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'%' {
            if i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit()
            {
                return false;
            }
            i += 3;
            continue;
        }
        if !(c.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&'()*+,;=".contains(&c)) {
            return false;
        }
        i += 1;
    }
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sources_are_real_uri_syntax() {
        for bad in [
            "https://",
            "https://x@host/path",
            "https://:443/x",
            "https://host/%zz",
            "https://ho^st/x",
            "http://host/x",
            "https://host/a\\b",
        ] {
            assert!(!https_source(bad), "{bad}")
        }
        for good in [
            "https://example.org/source?a=1",
            "https://[::1]:443/x",
            "https://example.org/a%20b",
        ] {
            assert!(https_source(good), "{good}")
        }
    }
}
