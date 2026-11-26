//! The body of every API answer that refuses or fails.
//!
//! A handler returns `Err(AppError)`. axum turns it into a status and this JSON body:
//!
//! ```json
//! { "code": "invalid", "message": "name is empty", "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736" }
//! ```
//!
//! The screen shows the Thai or English text for `code`, and `trace_id` in small text.
//! `message` is for developers. An `internal` error writes its detail only to the log,
//! because the detail may hold a database message or a secret. Its body says `internal error`.
//!
//! Types in this file:
//!
//! | Type        | Holds                                 | Why                                                         |
//! | ----------- | ------------------------------------- | ----------------------------------------------------------- |
//! | `ErrorCode` | One code from the table below         | The frontend has a text for each code. A story adds the codes it needs. |
//! | `AppError`  | A code and a message for developers   | What a handler returns when it refuses or fails             |
//! | `ErrorBody` | `code`, `message` and `trace_id`      | The JSON that the browser reads                             |
//!
//! | Code       | Status | When                                              |
//! | ---------- | ------ | ------------------------------------------------- |
//! | `invalid`  | 422    | The request breaks a rule, such as an empty name  |
//! | `internal` | 500    | Our fault, such as a lost database connection     |

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::trace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Invalid,
    Internal,
}

impl ErrorCode {
    fn status(self) -> StatusCode {
        match self {
            ErrorCode::Invalid => StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Debug)]
pub struct AppError {
    code: ErrorCode,
    message: String,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> AppError {
        AppError { code, message: message.into() }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    code: ErrorCode,
    message: String,
    trace_id: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let mut message = self.message;
        if self.code == ErrorCode::Internal {
            tracing::error!(detail = %message, "internal error");
            message = String::from("internal error");
        } else {
            tracing::info!(code = ?self.code, %message, "request refused");
        }

        let body = ErrorBody { code: self.code, message, trace_id: trace::current().to_string() };
        (self.code.status(), Json(body)).into_response()
    }
}
