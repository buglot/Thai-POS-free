//! One trace ID for each request.
//!
//! The browser may send the ID in a W3C `traceparent` header. Otherwise the server makes one.
//! The ID appears in every log line of the request, in the `x-trace-id` response header
//! and in the error body. Support asks for the ID on the screen and searches the log for it.
//!
//! `middleware` also writes one log line when a request ends, with the method, the route and the status.
//! Add it last, with `Router::layer(axum::middleware::from_fn(trace::middleware))`,
//! so that it wraps every route and knows which route matched.
//!
//! Types in this file:
//!
//! | Type      | Holds                                                | Why                                                        |
//! | --------- | ---------------------------------------------------- | ---------------------------------------------------------- |
//! | `TraceId` | 32 characters, each `0`-`9` or `a`-`f`, not all `0`  | The trace-id part of `traceparent`. Only this file makes one, so every `TraceId` is valid. |
//! | `CURRENT` | The `TraceId` of the request this task handles       | The error body reads it. tokio keeps one value for each task. |
//!
//! A `traceparent` header has four parts joined by `-`, such as
//! `00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01`.
//!
//! | Part      | Rule                                         |
//! | --------- | -------------------------------------------- |
//! | version   | Exactly `00`. It is the only version today.  |
//! | trace-id  | 32 lowercase hex digits, not all `0`         |
//! | parent-id | 16 lowercase hex digits, not all `0`         |
//! | flags     | 2 lowercase hex digits                       |
//!
//! A header that breaks a rule is ignored and a fresh ID is made. The header is never written to the log,
//! so a caller cannot put text into the log.
//!
//! The log shows the route, such as `/t/{token}`, and never the path. A path may hold a secret,
//! such as the table link in US-018.

use std::fmt;

use axum::extract::{MatchedPath, Request};
use axum::http::{HeaderMap, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use tracing::Instrument;

pub const RESPONSE_HEADER: &str = "x-trace-id";

#[derive(Debug, Clone)]
pub struct TraceId(String);

impl TraceId {
    /// A version 4 UUID always has a `4` in it, so it is never all zero.
    fn fresh() -> TraceId {
        TraceId(uuid::Uuid::new_v4().simple().to_string())
    }

    fn from_traceparent(header: &str) -> Option<TraceId> {
        let parts: Vec<&str> = header.split('-').collect();
        if parts.len() != 4 {
            return None;
        }
        let version = parts[0];
        let trace_id = parts[1];
        let parent_id = parts[2];
        let flags = parts[3];

        if version != "00" {
            return None;
        }
        if !is_lower_hex(trace_id, 32) || is_all_zero(trace_id) {
            return None;
        }
        if !is_lower_hex(parent_id, 16) || is_all_zero(parent_id) {
            return None;
        }
        if !is_lower_hex(flags, 2) {
            return None;
        }
        Some(TraceId(trace_id.to_owned()))
    }

    fn for_request(headers: &HeaderMap) -> TraceId {
        // `to_str` refuses bytes outside visible ASCII.
        if let Some(value) = headers.get("traceparent")
            && let Ok(text) = value.to_str()
            && let Some(id) = TraceId::from_traceparent(text)
        {
            return id;
        }
        TraceId::fresh()
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// True when `text` is exactly `len` characters, each `0`-`9` or `a`-`f`.
fn is_lower_hex(text: &str, len: usize) -> bool {
    if text.len() != len {
        return false;
    }
    for byte in text.bytes() {
        let digit = byte.is_ascii_digit();
        let letter = (b'a'..=b'f').contains(&byte);
        if !digit && !letter {
            return false;
        }
    }
    true
}

fn is_all_zero(text: &str) -> bool {
    text.bytes().all(|byte| byte == b'0')
}

tokio::task_local! {
    static CURRENT: TraceId;
}

/// Outside a request, such as in a unit test, this is a fresh ID, so an error body always has one.
pub fn current() -> TraceId {
    match CURRENT.try_with(|id| id.clone()) {
        Ok(id) => id,
        Err(_) => TraceId::fresh(),
    }
}

pub async fn middleware(request: Request, next: Next) -> Response {
    let trace_id = TraceId::for_request(request.headers());

    let mut route: &str = "no route";
    if let Some(matched) = request.extensions().get::<MatchedPath>() {
        route = matched.as_str();
    }
    let span = tracing::info_span!("request", trace_id = %trace_id, method = %request.method(), route);

    let header = HeaderValue::from_str(&trace_id.0).expect("a trace ID is only hex digits");
    let handle = async move {
        let mut response = next.run(request).await;
        tracing::info!(status = response.status().as_u16(), "request done");
        response.headers_mut().insert(RESPONSE_HEADER, header);
        response
    };
    CURRENT.scope(trace_id, handle.instrument(span)).await
}
