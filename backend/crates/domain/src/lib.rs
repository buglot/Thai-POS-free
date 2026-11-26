//! Business rules with no I/O. Nothing here knows about HTTP or SQL.

mod money;
mod role;

pub use money::Satang;
pub use role::{Permission, Role};
