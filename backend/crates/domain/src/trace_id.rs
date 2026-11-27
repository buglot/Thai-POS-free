//! The trace ID of one request.
//!
//! Every request has one ID. Support asks for it on the screen and finds the request in the log,
//! the audit log and the order history. The ID is the trace-id part of a W3C `traceparent` header.
//! In the header `00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01` the trace ID is
//! `4bf92f3577b34da6a3ce929d0e0e4736`.
//!
//! Types in this file:
//!
//! | Type      | Holds                                                 | Why                                                    |
//! | --------- | ----------------------------------------------------- | ------------------------------------------------------ |
//! | `TraceId` | 32 characters, each `0`-`9` or `a`-`f`, not all `0`   | Only `TraceId::parse` makes one, so every `TraceId` is valid. |
//!
//! This crate does not make a fresh ID. A fresh ID needs a random number, and a random number
//! needs a dependency. The kernel makes fresh IDs and reads the `traceparent` header.

use std::fmt;

/// The trace ID of one request.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TraceId(String);

impl TraceId {
    /// Reads a trace ID from text. Returns `None` when the text is not
    /// exactly 32 lowercase hex digits, or when every digit is `0`.
    pub fn parse(text: &str) -> Option<TraceId> {
        if text.len() != 32 {
            return None;
        }

        let mut all_zero = true;
        for byte in text.bytes() {
            let digit = byte.is_ascii_digit();
            let letter = (b'a'..=b'f').contains(&byte);
            if !digit && !letter {
                return None;
            }
            if byte != b'0' {
                all_zero = false;
            }
        }
        if all_zero {
            return None;
        }

        Some(TraceId(text.to_owned()))
    }
}

/// Prints the 32 digits, the same text that `parse` read.
impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// OK cases check that a valid ID is kept. NG cases check that bad text is refused.
#[cfg(test)]
mod tests {
    use super::TraceId;

    /// OK: a valid ID is kept and prints back as the same text.
    #[test]
    fn ok_parse_keeps_a_valid_id() {
        let text = "4bf92f3577b34da6a3ce929d0e0e4736";
        let id = TraceId::parse(text).expect("a valid ID");
        assert_eq!(id.to_string(), text);
    }

    /// OK: one digit that is not `0` is enough.
    #[test]
    fn ok_parse_keeps_an_id_with_one_digit_not_zero() {
        let text = "00000000000000000000000000000001";
        assert!(TraceId::parse(text).is_some());
    }

    /// NG: each text breaks one rule and is refused.
    #[test]
    fn ng_parse_refuses_bad_text() {
        let cases = [
            ("", "empty"),
            ("4bf92f3577b34da6a3ce929d0e0e473", "31 characters"),
            ("4bf92f3577b34da6a3ce929d0e0e47360", "33 characters"),
            ("4BF92F3577B34DA6A3CE929D0E0E4736", "uppercase"),
            ("4bf92f3577b34da6a3ce929d0e0e473g", "g is not hex"),
            ("4bf92f3577b34da6 3ce929d0e0e4736", "a space"),
            ("00000000000000000000000000000000", "all zero"),
            ("4bf92f3577b34da6a3ce929d0e0e4ก", "29 digits and ก, which is 3 bytes, so 32 bytes"),
            ("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01", "a whole traceparent header"),
        ];
        for (text, reason) in cases {
            assert_eq!(TraceId::parse(text), None, "{reason}: {text:?}");
        }
    }
}
