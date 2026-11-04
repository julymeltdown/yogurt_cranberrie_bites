//! Real HTTP transport must poll retained completion after a fixed-size serializer.
use axum::{body::Body, response::Response, routing::get, Router};
use platform::actor_delegation::response_scope::{
    ResponseScope, ResponseScopeToken, RetainedScopeLayer,
};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct Completion(AtomicUsize);
impl ResponseScope for Completion {
    fn finish_response(
        self: Arc<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), tonic::Status>> + Send>> {
        Box::pin(async move {
            tokio::task::yield_now().await;
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
}

async fn actual_http(protected: bool, data: &'static str) -> (Vec<u8>, usize) {
    let count = Arc::new(Completion(AtomicUsize::new(0)));
    let scope = count.clone();
    let router = Router::new()
        .route(
            "/",
            get(move || {
                let scope = scope.clone();
                async move {
                    let mut response = Response::new(Body::from(data));
                    response
                        .headers_mut()
                        .insert("content-length", data.len().to_string().parse().unwrap());
                    if protected {
                        response
                            .extensions_mut()
                            .insert(ResponseScopeToken::new(scope));
                    }
                    response
                }
            }),
        )
        .layer(RetainedScopeLayer);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
    });
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    socket
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), socket.read_to_end(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    let completed = count.0.load(Ordering::SeqCst);
    stop.send(()).unwrap();
    server.await.unwrap().unwrap();
    (bytes, completed)
}

#[tokio::test]
async fn normalized_http_cannot_skip_completion_after_content_length_bytes() {
    let (bytes, completed) = actual_http(true, "data").await;
    let response = String::from_utf8(bytes).unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(headers.starts_with("HTTP/1.1 200"));
    assert!(!headers.to_ascii_lowercase().contains("content-length:"));
    assert!(headers
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked"));
    assert_eq!(body, "4\r\ndata\r\n0\r\n\r\n");
    assert_eq!(
        completed, 1,
        "actual terminal framing must await settlement"
    );
}

#[tokio::test]
async fn unprotected_http_retains_its_fixed_length_without_settlement() {
    let (bytes, completed) = actual_http(false, "data").await;
    let response = String::from_utf8(bytes).unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(headers.to_ascii_lowercase().contains("content-length: 4"));
    assert!(!headers.to_ascii_lowercase().contains("transfer-encoding:"));
    assert_eq!(body, "data");
    assert_eq!(completed, 0);
}

#[tokio::test]
async fn zero_length_scoped_http_still_polls_the_completion_boundary() {
    let (bytes, completed) = actual_http(true, "").await;
    let response = String::from_utf8(bytes).unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(!headers.to_ascii_lowercase().contains("content-length:"));
    assert_eq!(body, "0\r\n\r\n");
    assert_eq!(completed, 1);
}
