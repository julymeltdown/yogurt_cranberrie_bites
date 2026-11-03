//! Exact public JWKS projection shared by HTTP and authenticated RPC.
use crate::application::dto::token_dto::AuthJwkSetDto;
use serde_json::{json, Value};

pub fn public_jwks(set: &AuthJwkSetDto) -> Value {
    let keys: Vec<Value> = set
        .keys
        .iter()
        .map(|key| {
            json!({
                "kty": key.kty,
                "use": key.key_use,
                "alg": key.alg,
                "kid": key.kid,
                "n": key.n,
                "e": key.e,
            })
        })
        .collect();
    json!({ "keys": keys })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::dto::token_dto::AuthPublicJwkDto;

    #[test]
    fn projection_preserves_exact_public_fields_and_configured_key_order() {
        let set = AuthJwkSetDto {
            keys: ["signing", "next", "previous"]
                .into_iter()
                .map(|kid| AuthPublicJwkDto {
                    kty: "RSA".into(),
                    key_use: "sig".into(),
                    alg: "RS256".into(),
                    kid: kid.into(),
                    n: format!("modulus-{kid}"),
                    e: "AQAB".into(),
                })
                .collect(),
        };
        let actual = public_jwks(&set);
        assert_eq!(
            actual,
            json!({"keys": [
                {"kty":"RSA","use":"sig","alg":"RS256","kid":"signing","n":"modulus-signing","e":"AQAB"},
                {"kty":"RSA","use":"sig","alg":"RS256","kid":"next","n":"modulus-next","e":"AQAB"},
                {"kty":"RSA","use":"sig","alg":"RS256","kid":"previous","n":"modulus-previous","e":"AQAB"}
            ]})
        );
        assert_eq!(
            public_jwks(&AuthJwkSetDto { keys: vec![] }),
            json!({"keys": []})
        );
    }
}
