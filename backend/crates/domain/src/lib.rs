//! Business rules with no I/O. Nothing here knows about HTTP or SQL.

mod bill;
mod money;

pub use bill::{Bill, BillLine, Rate, VatMode};
pub use money::Satang;
