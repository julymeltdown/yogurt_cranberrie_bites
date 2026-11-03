use crate::application::dto::auth_dto::ErrorResponse;
use axum::body::{to_bytes, Body, HttpBody};
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use chrono::Utc;
use uuid::Uuid;

const MAX_EXTRACTOR_REJECTION_BYTES: usize = 16 * 1024;

enum ExtractorFailure {
    MalformedJson,
    MissingJsonContentType,
    WrongJsonFieldType,
    InvalidQuery,
}

impl ExtractorFailure {
    const fn message(&self) -> &'static str {
        match self {
            Self::MalformedJson => "Malformed JSON request body",
            Self::MissingJsonContentType => "Content-Type must be application/json",
            Self::WrongJsonFieldType => "JSON request body does not match the expected schema",
            Self::InvalidQuery => "Invalid query parameters",
        }
    }
}

pub async fn normalize_extractor_rejections(request: Request<Body>, next: Next) -> Response {
    let response = next.run(request).await;
    if is_json_response(&response) || !is_extractor_status(response.status()) {
        return response;
    }

    let exact_size = response.body().size_hint().exact();
    if exact_size.is_none_or(|size| size > MAX_EXTRACTOR_REJECTION_BYTES as u64) {
        return response;
    }

    let (mut parts, body) = response.into_parts();
    let bytes = match to_bytes(body, MAX_EXTRACTOR_REJECTION_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => return Response::from_parts(parts, Body::empty()),
    };
    let Some(failure) = classify(parts.status, &bytes) else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    let trace_id = parts
        .headers
        .get("x-correlation-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let envelope = ErrorResponse {
        code: format!("GATEWAY_{}", parts.status.as_u16()),
        message: failure.message().to_owned(),
        trace_id: Some(trace_id),
        timestamp: Some(Utc::now()),
        details: None,
    };
    let normalized = match serde_json::to_vec(&envelope) {
        Ok(normalized) => normalized,
        Err(_) => return Response::from_parts(parts, Body::from(bytes)),
    };
    parts.headers.remove(CONTENT_LENGTH);
    parts.headers.insert(
        CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Response::from_parts(parts, Body::from(normalized))
}

fn is_json_response(response: &Response) -> bool {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
}

const fn is_extractor_status(status: StatusCode) -> bool {
    matches!(status.as_u16(), 400 | 415 | 422)
}

fn classify(status: StatusCode, body: &[u8]) -> Option<ExtractorFailure> {
    let diagnostic = std::str::from_utf8(body).ok()?;
    match status {
        StatusCode::BAD_REQUEST
            if diagnostic.starts_with("Failed to parse the request body as JSON:") =>
        {
            Some(ExtractorFailure::MalformedJson)
        }
        StatusCode::BAD_REQUEST
            if diagnostic.starts_with("Failed to deserialize query string:") =>
        {
            Some(ExtractorFailure::InvalidQuery)
        }
        StatusCode::UNSUPPORTED_MEDIA_TYPE
            if diagnostic.starts_with("Expected request with `Content-Type: application/json`") =>
        {
            Some(ExtractorFailure::MissingJsonContentType)
        }
        StatusCode::UNPROCESSABLE_ENTITY
            if diagnostic
                .starts_with("Failed to deserialize the JSON body into the target type:") =>
        {
            Some(ExtractorFailure::WrongJsonFieldType)
        }
        _ => None,
    }
}
