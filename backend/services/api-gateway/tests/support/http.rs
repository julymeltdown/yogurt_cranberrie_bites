use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

pub async fn json_request(
    app: &crate::support::TestApp,
    method: Method,
    uri: &str,
    token: Option<&str>,
    headers: &[(&str, &str)],
    body: Option<Value>,
) -> (StatusCode, HeaderMap, Value) {
    let has_body = body.is_some();
    let body = body
        .map(|value| Body::from(value.to_string()))
        .unwrap_or_else(Body::empty);
    let mut content_headers = headers.to_vec();
    if has_body {
        content_headers.push(("content-type", "application/json"));
    }
    request(app, method, uri, token, &content_headers, body).await
}

pub async fn json_request_ok(
    app: &crate::support::TestApp,
    method: Method,
    uri: &str,
    token: Option<&str>,
    headers: &[(&str, &str)],
    body: Option<Value>,
) -> (HeaderMap, Value) {
    let (status, headers, body) = json_request(app, method, uri, token, headers, body).await;
    assert_eq!(status, StatusCode::OK);
    (headers, body)
}

pub async fn multipart_avatar_upload(
    app: &crate::support::TestApp,
    token: &str,
) -> (StatusCode, HeaderMap, Value) {
    multipart_avatar_upload_with_content(app, token, b"avatar-bytes").await
}

pub async fn multipart_avatar_upload_with_content(
    app: &crate::support::TestApp,
    token: &str,
    content: &[u8],
) -> (StatusCode, HeaderMap, Value) {
    let boundary = "gateway-boundary";
    let mut body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"avatar.png\"\r\n\
         Content-Type: image/png\r\n\r\n\
         "
    )
    .into_bytes();
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    request(
        app,
        Method::POST,
        "/api/profile/avatar",
        Some(token),
        &[(
            "content-type",
            "multipart/form-data; boundary=gateway-boundary",
        )],
        Body::from(body),
    )
    .await
}

pub async fn multipart_avatar_upload_ok(
    app: &crate::support::TestApp,
    token: &str,
) -> (HeaderMap, Value) {
    let (status, headers, body) = multipart_avatar_upload(app, token).await;
    assert_eq!(status, StatusCode::OK);
    (headers, body)
}

pub fn header_contains(headers: &HeaderMap, name: &str, needle: &str) -> bool {
    headers
        .get_all(name)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| value.contains(needle))
}

async fn request(
    app: &crate::support::TestApp,
    method: Method,
    uri: &str,
    token: Option<&str>,
    headers: &[(&str, &str)],
    body: Body,
) -> (StatusCode, HeaderMap, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let response = app
        .router
        .clone()
        .oneshot(builder.body(body).expect("request"))
        .await
        .expect("response");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("json body")
    };
    (status, headers, body)
}
