//! Sends requests through a small router with the trace middleware, then reads the response and the log.
//!
//! OK cases check the right answer. NG cases check that a wrong input is refused.

use std::io;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, Request, StatusCode};
use axum::middleware::from_fn;
use axum::response::Response;
use axum::routing::get;
use pos_kernel::error::{AppError, ErrorCode};
use pos_kernel::trace;
use tower::ServiceExt;

/// The trace-id inside `VALID`.
const ID: &str = "4bf92f3577b34da6a3ce929d0e0e4736";
/// The example header from the W3C Trace Context standard.
const VALID: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

/// A log that a test can read. tracing writes each line into the shared bytes.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<u8>>>);

impl Log {
    fn text(&self) -> String {
        let bytes = self.0.lock().expect("no test panics while it holds the log").clone();
        String::from_utf8(bytes).expect("the log is UTF-8")
    }
}

impl io::Write for Log {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("no test panics while it holds the log").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

async fn hello() -> &'static str {
    "hello"
}

async fn refuse() -> Result<&'static str, AppError> {
    Err(AppError::new(ErrorCode::Invalid, "name is empty"))
}

async fn fail() -> Result<&'static str, AppError> {
    Err(AppError::new(ErrorCode::Internal, "database refused password hunter2"))
}

fn app() -> Router {
    Router::new()
        .route("/hello", get(hello))
        .route("/t/{token}", get(hello))
        .route("/refuse", get(refuse))
        .route("/fail", get(fail))
        .layer(from_fn(trace::middleware))
}

/// Sends one GET request and returns the response and every log line it wrote.
async fn send(path: &str, traceparent: Option<HeaderValue>) -> (Response, String) {
    let log = Log::default();
    let writer = log.clone();
    let subscriber = tracing_subscriber::fmt().with_writer(move || writer.clone()).finish();
    let _only_this_test = tracing::subscriber::set_default(subscriber);

    let mut request = Request::builder().uri(path);
    if let Some(value) = traceparent {
        request = request.header("traceparent", value);
    }
    let request = request.body(Body::empty()).expect("the test request is valid");
    let response = app().oneshot(request).await.expect("a router always answers");
    (response, log.text())
}

fn header_id(response: &Response) -> String {
    let value = response.headers().get("x-trace-id").expect("every response has x-trace-id");
    value.to_str().expect("a trace ID is ASCII").to_owned()
}

async fn body_json(response: Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("the body can be read");
    serde_json::from_slice(&bytes).expect("an error body is JSON")
}

/// True for 32 lowercase hex digits that are not all zero. Written from the W3C rule, not from the code.
fn is_trace_id(text: &str) -> bool {
    let hex = text.len() == 32 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    hex && text != "00000000000000000000000000000000"
}

/// OK: the Done-when of T-07. One request puts one ID in the log, the `x-trace-id` header and the error body.
#[tokio::test]
async fn ok_one_id_in_log_header_and_body() {
    let (response, log) = send("/refuse", None).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let id = header_id(&response);
    assert!(is_trace_id(&id), "{id}");

    let body = body_json(response).await;
    assert_eq!(body["code"], "invalid");
    assert_eq!(body["message"], "name is empty");
    assert_eq!(body["trace_id"], id.as_str());

    assert!(!log.is_empty());
    for line in log.lines() {
        assert!(line.contains(&id), "{line}");
    }
}

/// OK: a valid `traceparent` keeps its trace-id, so the browser, the log and the error body show the same ID.
#[tokio::test]
async fn ok_valid_traceparent_is_reused() {
    let (response, log) = send("/refuse", Some(HeaderValue::from_static(VALID))).await;

    assert_eq!(header_id(&response), ID);
    assert!(log.contains(ID), "{log}");
    let body = body_json(response).await;
    assert_eq!(body["trace_id"], ID);
}

/// OK: without `traceparent` the server makes a valid ID, and a new one for each request.
#[tokio::test]
async fn ok_without_traceparent_a_fresh_id_is_made() {
    let (first, first_log) = send("/hello", None).await;
    let (second, _) = send("/hello", None).await;

    let id = header_id(&first);
    assert!(is_trace_id(&id), "{id}");
    assert!(first_log.contains(&id), "{first_log}");
    assert_ne!(id, header_id(&second));
}

/// NG: a `traceparent` that breaks any rule is ignored. The request gets a fresh ID,
/// and neither the header nor its trace-id reaches the log.
#[tokio::test]
async fn ng_bad_traceparent_gets_a_fresh_id() {
    let bad_headers = [
        "garbage",
        // Unknown version.
        "01-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        // Uppercase hex.
        "00-4BF92F3577B34DA6A3CE929D0E0E4736-00f067aa0ba902b7-01",
        // Trace-id with 31 digits.
        "00-4bf92f3577b34da6a3ce929d0e0e473-00f067aa0ba902b7-01",
        // Trace-id all zero.
        "00-00000000000000000000000000000000-00f067aa0ba902b7-01",
        // Parent-id with 15 digits.
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b-01",
        // Parent-id all zero.
        "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01",
        // Flags that are not hex.
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-zz",
        // A fifth part.
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-00",
        // Text added to try to write into the log.
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01 user=admin",
        // Bytes outside ASCII.
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-ไทย",
    ];
    for bad in bad_headers {
        let value = HeaderValue::from_bytes(bad.as_bytes()).expect("each test header is a legal HTTP value");
        let (response, log) = send("/hello", Some(value)).await;

        let id = header_id(&response);
        assert!(is_trace_id(&id), "{bad:?} gave {id}");
        assert_ne!(id, ID, "{bad:?}");
        assert!(log.contains(&id), "{bad:?}: {log}");
        assert!(!log.contains(ID), "{bad:?}: {log}");
        assert!(!log.contains(bad), "{bad:?}: {log}");
    }
}

/// OK: the log names the route and not the path, so a secret in a link stays out of the log.
#[tokio::test]
async fn ok_log_shows_the_route_not_the_path() {
    let (response, log) = send("/t/secret-table-link", None).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(log.contains("/t/{token}"), "{log}");
    assert!(!log.contains("secret-table-link"), "{log}");
}

/// NG: a handler fails inside. The body says only `internal`. The detail goes to the log with the trace ID.
#[tokio::test]
async fn ng_internal_error_keeps_the_detail_out_of_the_body() {
    let (response, log) = send("/fail", None).await;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let id = header_id(&response);
    let body = body_json(response).await;
    assert_eq!(body["code"], "internal");
    assert_eq!(body["trace_id"], id.as_str());
    assert!(!body.to_string().contains("hunter2"), "{body}");

    let detail_line = log.lines().find(|line| line.contains("hunter2")).expect("the detail is in the log");
    assert!(detail_line.contains(&id), "{detail_line}");
}

/// OK: outside a request `current` still gives a valid ID, and a new one each time.
#[test]
fn ok_current_outside_a_request_is_a_fresh_id() {
    let first = trace::current().to_string();
    let second = trace::current().to_string();

    assert!(is_trace_id(&first), "{first}");
    assert_ne!(first, second);
}
