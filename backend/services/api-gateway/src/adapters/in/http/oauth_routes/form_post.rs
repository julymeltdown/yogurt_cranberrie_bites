//! Apple cross-site form POST adaptation. The first request cannot rely on a
//! SameSite=Lax cookie. Only the subsequent Gateway-origin form submission may
//! exchange the code, after checking the original provider-specific cookie.
use super::*;
use crate::application::dto::facade_dto::IdentityProviderDto;
use axum::body::{to_bytes, Body};
use axum::extract::{FromRequest, Request};
use axum::http::header;
use axum::response::Html;
use base64ct::{Base64UrlUnpadded, Encoding};
use ring::rand::{SecureRandom, SystemRandom};

const MAX_BODY_BYTES: usize = 32 * 1024;
const MAX_CODE_BYTES: usize = 4096;
const MAX_STATE_BYTES: usize = 512;

// Deliberately no Debug: these values are provider credentials.
struct CallbackForm {
    code: String,
    state: String,
}

pub(super) async fn receive(Path(provider): Path<String>, request: Request) -> Response {
    let mut response = receive_inner(provider, request).await.into_response();
    protect_response(&mut response);
    response
}

async fn receive_inner(provider: String, request: Request) -> Result<Response, GatewayError> {
    require_apple(&provider)?;
    let form = read_form(request, true).await?;
    let mut random = [0_u8; 24];
    SystemRandom::new()
        .fill(&mut random)
        .map_err(|_| GatewayError::Internal(anyhow::anyhow!("OAuth page nonce unavailable")))?;
    let nonce = Base64UrlUnpadded::encode_string(&random);
    let page = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>Complete sign-in</title></head><body>\
         <form id=\"oauth-completion\" method=\"post\" action=\"/auth/oauth/apple/callback/complete\">\
         <input type=\"hidden\" name=\"code\" value=\"{}\">\
         <input type=\"hidden\" name=\"state\" value=\"{}\">\
         <button type=\"submit\">Continue sign-in</button></form>\
         <script nonce=\"{nonce}\">document.getElementById('oauth-completion').submit();</script>\
         </body></html>",
        escape_attribute(&form.code),
        escape_attribute(&form.state),
    );
    let mut response = Html(page).into_response();
    response.headers_mut().insert(
        "content-security-policy",
        super::super::header_value(&format!(
            "default-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'; script-src 'nonce-{nonce}'"
        ))?,
    );
    protect_response(&mut response);
    Ok(response)
}

pub(super) async fn complete(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    request: Request,
) -> Response {
    let mut response = complete_inner(state, provider, request)
        .await
        .into_response();
    protect_response(&mut response);
    response
}

async fn complete_inner(
    state: Arc<AppState>,
    provider: String,
    request: Request,
) -> Result<Response, GatewayError> {
    let provider = require_apple(&provider)?;
    let headers = request.headers().clone();
    reject_ambiguous_state_cookie(&headers)?;
    let form = read_form(request, false).await?;
    verify_oauth_state(&state, provider, &form.state, &headers)?;
    // The owning Auth service consumes provider/state atomically before code
    // exchange. A timeout is uncertain and must not trigger an automatic retry.
    let response = state
        .auth_service
        .oauth_callback(
            provider,
            form.code,
            form.state,
            client_ip(&headers),
            user_agent(&headers),
        )
        .await
        .map_err(map_service_error)?;
    let mut response_headers = HeaderMap::new();
    append_set_cookie(
        &mut response_headers,
        build_refresh_cookie(&state, &response.refresh_token),
    )?;
    append_set_cookie(
        &mut response_headers,
        clear_oauth_state_cookie(&state, provider),
    )?;
    let mut response = (
        StatusCode::OK,
        response_headers,
        Json(strip_refresh_token(response)),
    )
        .into_response();
    protect_response(&mut response);
    Ok(response)
}

fn require_apple(provider: &str) -> Result<IdentityProviderDto, GatewayError> {
    let provider = parse_provider(provider)?;
    if provider != IdentityProviderDto::Apple {
        return Err(GatewayError::bad_request(
            "Form POST is supported only for Apple",
        ));
    }
    Ok(provider)
}

async fn read_form(
    request: Request,
    provider_response: bool,
) -> Result<CallbackForm, GatewayError> {
    let (parts, body) = request.into_parts();
    // Read the stream under a hard limit, including requests without a length.
    let bytes = to_bytes(body, MAX_BODY_BYTES).await.map_err(|_| {
        GatewayError::bad_request("OAuth form exceeds the body limit or is unreadable")
    })?;
    validate_encoding(&bytes)?;
    let request = Request::from_parts(parts, Body::from(bytes));
    let axum::Form(fields) = axum::Form::<Vec<(String, String)>>::from_request(request, &())
        .await
        .map_err(|_| GatewayError::bad_request("Expected a URL-encoded OAuth form"))?;
    let mut values = HashMap::new();
    for (name, value) in fields {
        let allowed = matches!(name.as_str(), "code" | "state")
            || (provider_response
                && matches!(
                    name.as_str(),
                    "error" | "error_description" | "id_token" | "user"
                ));
        if !allowed || values.insert(name, value).is_some() {
            return Err(GatewayError::bad_request(
                "Unknown or duplicate OAuth form field",
            ));
        }
    }
    if values.contains_key("error") || values.contains_key("error_description") {
        // Do not reflect arbitrary provider error text, consume state, or alter
        // the browser's cookie when authorization was cancelled or denied.
        return Err(GatewayError::bad_request(
            "OAuth authorization was not completed",
        ));
    }
    let code = values.remove("code").unwrap_or_default();
    let state = values.remove("state").unwrap_or_default();
    for (value, limit) in [(&code, MAX_CODE_BYTES), (&state, MAX_STATE_BYTES)] {
        if value.trim().is_empty() || value.len() > limit || value.chars().any(char::is_control) {
            return Err(GatewayError::bad_request("Invalid OAuth code or state"));
        }
    }
    Ok(CallbackForm { code, state })
}

fn validate_encoding(bytes: &[u8]) -> Result<(), GatewayError> {
    // URL form decoders may tolerate invalid percent escapes or replace invalid
    // UTF-8. Reject both instead of accepting an altered credential.
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'%' {
            let pair = bytes
                .get(index + 1..index + 3)
                .ok_or_else(|| GatewayError::bad_request("Invalid OAuth form encoding"))?;
            let hex = |byte: u8| (byte as char).to_digit(16).map(|digit| digit as u8);
            let (Some(high), Some(low)) = (hex(pair[0]), hex(pair[1])) else {
                return Err(GatewayError::bad_request("Invalid OAuth form encoding"));
            };
            decoded.push(high * 16 + low);
            index += 3;
        } else {
            decoded.push(byte);
            index += 1;
        }
    }
    std::str::from_utf8(&decoded)
        .map_err(|_| GatewayError::bad_request("Invalid OAuth form encoding"))?;
    Ok(())
}

fn reject_ambiguous_state_cookie(headers: &HeaderMap) -> Result<(), GatewayError> {
    let mut count = 0;
    for header in headers.get_all(header::COOKIE) {
        let header = header
            .to_str()
            .map_err(|_| GatewayError::bad_request("Invalid OAuth state"))?;
        count += header
            .split(';')
            .filter(|cookie| {
                cookie
                    .trim()
                    .split_once('=')
                    .is_some_and(|(name, _)| name == "oauth_state_apple")
            })
            .count();
    }
    if count != 1 {
        return Err(GatewayError::bad_request("Invalid OAuth state"));
    }
    Ok(())
}

pub(super) fn protect_response(response: &mut Response) {
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        "no-store".parse().expect("static header"),
    );
    headers.insert(
        "referrer-policy",
        "no-referrer".parse().expect("static header"),
    );
    headers.insert(
        "x-content-type-options",
        "nosniff".parse().expect("static header"),
    );
}

fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
