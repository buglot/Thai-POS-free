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

fn app() -> Router {
    Router::new().route("/hello", get(hello)).route("/t/{token}", get(hello)).layer(from_fn(trace::middleware))
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

/// True for 32 lowercase hex digits that are not all zero. Written from the W3C rule, not from the code.
fn is_trace_id(text: &str) -> bool {
    let hex = text.len() == 32 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    hex && text != "00000000000000000000000000000000"
}

/// OK: a valid `traceparent` keeps its trace-id, so the browser and the server log show the same ID.
#[tokio::test]
async fn ok_valid_traceparent_is_reused() {
    let (response, log) = send("/hello", Some(HeaderValue::from_static(VALID))).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header_id(&response), ID);
    assert!(log.contains(ID), "{log}");
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
