//! Stable privacy bucket from a transport-verified address; never from a header.
use crate::adapters::r#in::http::client_identity::VerifiedClientAddress;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::net::IpAddr;

pub(super) struct NetworkBucketKey([u8; 32]);
impl NetworkBucketKey {
    pub(super) fn load() -> anyhow::Result<Option<Self>> {
        let value = match std::env::var("MCR_GIFT_NETWORK_BUCKET_KEY") {
            Ok(value) => value,
            Err(std::env::VarError::NotPresent) => return Ok(None),
            Err(_) => anyhow::bail!("Invalid Gift network bucket key encoding"),
        };
        Self::parse(&value).map(Some)
    }
    fn parse(value: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(
            value.len() == 64
                && value
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "Gift network bucket key requires 32 lowercase hex bytes"
        );
        let mut key = [0; 32];
        for (index, byte) in key.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
                .map_err(|_| anyhow::anyhow!("Invalid Gift network bucket key"))?;
        }
        anyhow::ensure!(key != [0; 32], "Gift network bucket key must be nonzero");
        Ok(Self(key))
    }
    pub(super) fn bucket(&self, address: VerifiedClientAddress) -> Vec<u8> {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.0).expect("fixed HMAC key");
        mac.update(b"mcr/gift/trusted-network/v1\0");
        match address.address() {
            IpAddr::V4(ip) => {
                mac.update(&[4]);
                mac.update(&ip.octets());
            }
            IpAddr::V6(ip) => {
                mac.update(&[6]);
                mac.update(&ip.octets());
            }
        }
        mac.finalize().into_bytes().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::r#in::http::client_identity::normalize_forwarded_for;
    use axum::{
        body::{to_bytes, Body},
        extract::ConnectInfo,
        http::Request,
        routing::get,
        Router,
    };
    use std::{net::SocketAddr, sync::Arc};
    use tower::ServiceExt;

    async fn observed(peer: Option<&str>, forwarded: &str) -> Vec<u8> {
        let key = Arc::new(NetworkBucketKey([41; 32]));
        let app = Router::new()
            .route(
                "/",
                get(move |request: Request<Body>| {
                    let key = key.clone();
                    async move {
                        request
                            .extensions()
                            .get::<VerifiedClientAddress>()
                            .map(|address| key.bucket(*address))
                            .unwrap_or_default()
                    }
                }),
            )
            .layer(axum::middleware::from_fn(normalize_forwarded_for));
        let mut request = Request::builder()
            .uri("/")
            .header("x-forwarded-for", forwarded)
            .body(Body::empty())
            .unwrap();
        if let Some(peer) = peer {
            request
                .extensions_mut()
                .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
        }
        to_bytes(app.oneshot(request).await.unwrap().into_body(), 1024)
            .await
            .unwrap()
            .to_vec()
    }
    #[tokio::test]
    async fn bucket_is_stable_for_real_peer_and_headers_cannot_select_it() {
        let first = observed(Some("192.0.2.10:1234"), "198.51.100.1").await;
        let second = observed(Some("192.0.2.10:5678"), "203.0.113.3").await;
        assert_eq!(first, second);
        assert_eq!(first.len(), 32);
        assert_ne!(first, [0; 32]);
        assert_ne!(first, observed(Some("192.0.2.11:1234"), "192.0.2.10").await);
        assert!(observed(None, "192.0.2.10").await.is_empty());
    }
    #[test]
    fn privacy_key_has_no_zero_or_malformed_fallback() {
        for value in [
            "".into(),
            "0".repeat(64),
            "A".repeat(64),
            "a".repeat(63),
            "a".repeat(65),
        ] {
            assert!(NetworkBucketKey::parse(&value).is_err());
        }
        assert!(NetworkBucketKey::parse(&"a".repeat(64)).is_ok());
    }
}
