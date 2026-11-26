//! Business rules with no I/O. Nothing here knows about HTTP or SQL.

mod money;
mod promptpay;

pub use money::Satang;
pub use promptpay::{PromptPayAmountError, PromptPayId, PromptPayIdError, PromptPayKind};
