use auth_service::adapters::out::mail::reset_mailer::{HttpResetMailer, LoggingResetMailer};
use auth_service::application::ports::ResetMailerPort;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::Value;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tracing_subscriber::fmt::MakeWriter;

#[tokio::test]
async fn logging_reset_mailer_does_not_write_email_or_reset_token_to_logs() {
    let logs = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(SharedWriterFactory(logs.clone()))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    LoggingResetMailer
        .send_reset_link(
            "reset-user@example.com",
            "https://app.example.test/auth/reset?token=super-secret-reset-token",
        )
        .await
        .expect("send reset link");

    let output = String::from_utf8(logs.lock().expect("logs").clone()).expect("utf8 logs");
    assert!(output.contains("password reset link generated"));
    assert!(!output.contains("reset-user@example.com"));
    assert!(!output.contains("super-secret-reset-token"));
    assert!(!output.contains("token="));
}

#[tokio::test]
async fn http_reset_mailer_posts_reset_link_to_configured_provider() {
    let capture = ResetMailCapture::default();
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let app = Router::new()
        .route("/send", post(capture_reset_mail))
        .with_state(capture.clone());
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server");
    });
    let mailer = HttpResetMailer::new(
        format!("http://{addr}/send"),
        "provider-bearer-token".into(),
    )
    .expect("mailer");

    mailer
        .send_reset_link(
            "reset-user@example.com",
            "https://app.example.com/auth/reset#token=reset-token",
        )
        .await
        .expect("send reset link");

    assert_eq!(
        capture
            .authorization
            .lock()
            .expect("authorization")
            .as_deref(),
        Some("Bearer provider-bearer-token")
    );
    let payload = capture.payload.lock().expect("payload").clone();
    assert_eq!(
        payload.as_ref().and_then(|value| value["email"].as_str()),
        Some("reset-user@example.com")
    );
    assert_eq!(
        payload
            .as_ref()
            .and_then(|value| value["reset_link"].as_str()),
        Some("https://app.example.com/auth/reset#token=reset-token")
    );
}

#[derive(Clone, Default)]
struct ResetMailCapture {
    authorization: Arc<Mutex<Option<String>>>,
    payload: Arc<Mutex<Option<Value>>>,
}

async fn capture_reset_mail(
    State(capture): State<ResetMailCapture>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> StatusCode {
    *capture.authorization.lock().expect("authorization") = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    *capture.payload.lock().expect("payload") = Some(payload);
    StatusCode::NO_CONTENT
}

#[derive(Clone)]
struct SharedWriterFactory(Arc<Mutex<Vec<u8>>>);

impl<'a> MakeWriter<'a> for SharedWriterFactory {
    type Writer = SharedWriter;

    fn make_writer(&'a self) -> Self::Writer {
        SharedWriter(self.0.clone())
    }
}

struct SharedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("logs").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
