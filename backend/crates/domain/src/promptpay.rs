//! PromptPay (พร้อมเพย์) QR text.
//!
//! The customer scans the QR with a bank app. The app reads the receiver and the amount from the text.
//! This file builds the text offline. It needs no bank connection and holds no bank secret.
//!
//! Types in this file:
//!
//! | Type                   | Holds                                         | Why                                                              |
//! | ---------------------- | --------------------------------------------- | ---------------------------------------------------------------- |
//! | `PromptPayId`          | The receiver's digits, already checked        | Only `parse` makes one, so a QR never gets an unchecked receiver. |
//! | `PromptPayKind`        | Mobile, tax ID or e-wallet                    | Each kind goes in its own sub-field of field 29.                 |
//! | `PromptPayIdError`     | Why `parse` refused a receiver                | A screen can show a different text for each reason.              |
//! | `PromptPayAmountError` | Why `qr_text` refused an amount               | The same.                                                        |
//! | `u8`                   | One character of the text, 0 to 255          | The CRC and the check digit read ASCII text byte by byte.        |
//! | `u16`                  | The CRC, 0 to 65535                           | CRC-16 has exactly 16 bits. It is printed as 4 hex digits.       |
//! | `u32`                  | The check-digit sum, at most 9 x 90 = 810     | Far below the `u32` limit, so it cannot overflow.                |
//! | `usize`                | The length of a field value, at most 39 here  | It always fits the 2-digit length of a field.                    |
//!
//! The text format is EMVCo merchant-presented mode. EMVCo is the standards body of the card networks.
//! The text is a row of fields. Each field has three parts:
//!
//! | Part   | Size              | Example | Meaning                                          |
//! | ------ | ----------------- | ------- | ------------------------------------------------ |
//! | Tag    | 2 digits          | `54`    | Which field this is.                             |
//! | Length | 2 digits          | `04`    | How many characters the value has, `00` to `99`. |
//! | Value  | Length characters | `4.22`  | The content.                                     |
//!
//! So `54044.22` is tag `54`, length `04`, value `4.22`.
//! The value of field 29 is a row of sub-fields in the same form.
//!
//! The fields this file writes, in this order:
//!
//! | Tag     | Value                                  | Meaning                                                              |
//! | ------- | -------------------------------------- | -------------------------------------------------------------------- |
//! | `00`    | `01`                                   | Format version. Always the first field.                              |
//! | `01`    | `12`                                   | A dynamic QR, made for one payment. A static QR, printed once and used many times, has `11`. |
//! | `29`    | Sub-field `00`, then `01`, `02` or `03` | The PromptPay receiver.                                             |
//! | `29.00` | `A000000677010111`                     | The PromptPay application ID. It tells the bank app that this is PromptPay. |
//! | `29.01` | `0066` and the mobile number without its first `0` | 66 is the phone code of Thailand. `00` pads it to 13 digits. `0801234567` becomes `0066801234567`. |
//! | `29.02` | 13 digits                              | National ID or tax ID.                                               |
//! | `29.03` | 15 digits                              | E-wallet ID.                                                         |
//! | `58`    | `TH`                                   | Country: Thailand.                                                   |
//! | `53`    | `764`                                  | Currency: baht (บาท). 764 is the ISO 4217 number of the baht.        |
//! | `54`    | Baht with two decimals, such as `4.22` | The amount to pay.                                                   |
//! | `63`    | 4 hex digits                           | CRC-16/CCITT-FALSE of all text before these 4 digits, `6304` included. The scanning app uses it to find a misread QR. |
//!
//! `qr_text` refuses two kinds of amount:
//!
//! - Zero or less. EMVCo does not allow a zero amount or a minus sign in field 54. A bill with nothing left to pay needs no QR.
//! - More than 13 characters, so more than 9999999999.99 baht. EMVCo allows field 54 at most 13 characters.
//!
//! A 13-digit national ID or tax ID ends with a check digit.
//! Multiply the first 12 digits by 13, 12, 11 and so on down to 2, and add the results.
//! The check digit is (11 - sum mod 11) mod 10.
//! Example `1101700203450`:
//! 1 x 13 + 1 x 12 + 0 x 11 + 1 x 10 + 7 x 9 + 0 x 8 + 0 x 7 + 2 x 6 + 0 x 5 + 3 x 4 + 4 x 3 + 5 x 2 = 144.
//! 144 mod 11 = 1, and (11 - 1) mod 10 = 0, which is the last digit.

use std::fmt;

use crate::Satang;

const PROMPTPAY_APP_ID: &str = "A000000677010111";
const MAX_AMOUNT_LENGTH: usize = 13;

/// A PromptPay receiver. Build one with [`PromptPayId::parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptPayId {
    kind: PromptPayKind,
    digits: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptPayKind {
    /// 10 digits that start with 0.
    Mobile,
    /// 13 digits with a right check digit.
    TaxId,
    /// 15 digits.
    EWallet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptPayIdError {
    /// Not 10, 13 or 15 digits, a character other than a digit, a dash or a space,
    /// or 10 digits that do not start with 0.
    BadFormat,
    /// 13 digits with a wrong check digit.
    BadCheckDigit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptPayAmountError {
    ZeroOrNegative,
    TooLarge,
}

impl PromptPayId {
    /// Reads a receiver as an admin types it. Dashes and spaces are dropped, so `080-123 4567` is fine.
    pub fn parse(raw: &str) -> Result<PromptPayId, PromptPayIdError> {
        let mut digits = String::new();
        for c in raw.chars() {
            if c == '-' || c == ' ' {
                continue;
            }
            if !c.is_ascii_digit() {
                return Err(PromptPayIdError::BadFormat);
            }
            digits.push(c);
        }

        let kind = match digits.len() {
            10 => PromptPayKind::Mobile,
            13 => PromptPayKind::TaxId,
            15 => PromptPayKind::EWallet,
            _ => return Err(PromptPayIdError::BadFormat),
        };
        if kind == PromptPayKind::Mobile && !digits.starts_with('0') {
            return Err(PromptPayIdError::BadFormat);
        }
        if kind == PromptPayKind::TaxId && !check_digit_is_right(digits.as_bytes()) {
            return Err(PromptPayIdError::BadCheckDigit);
        }

        Ok(PromptPayId { kind, digits })
    }

    pub fn kind(&self) -> PromptPayKind {
        self.kind
    }

    /// The digits without dashes or spaces, such as `0801234567`.
    pub fn digits(&self) -> &str {
        &self.digits
    }

    /// The text to show as a QR code. It asks the payer for exactly `amount`.
    pub fn qr_text(&self, amount: Satang) -> Result<String, PromptPayAmountError> {
        if amount <= Satang::ZERO {
            return Err(PromptPayAmountError::ZeroOrNegative);
        }
        let amount_text = amount.to_string();
        if amount_text.len() > MAX_AMOUNT_LENGTH {
            return Err(PromptPayAmountError::TooLarge);
        }

        let receiver = match self.kind {
            PromptPayKind::Mobile => field("01", &format!("0066{}", &self.digits[1..])),
            PromptPayKind::TaxId => field("02", &self.digits),
            PromptPayKind::EWallet => field("03", &self.digits),
        };
        let mut account = field("00", PROMPTPAY_APP_ID);
        account.push_str(&receiver);

        let mut text = String::new();
        text.push_str(&field("00", "01"));
        text.push_str(&field("01", "12"));
        text.push_str(&field("29", &account));
        text.push_str(&field("58", "TH"));
        text.push_str(&field("53", "764"));
        text.push_str(&field("54", &amount_text));
        text.push_str("6304");
        let crc = crc16(text.as_bytes());
        text.push_str(&format!("{crc:04X}"));
        Ok(text)
    }
}

fn field(tag: &str, value: &str) -> String {
    format!("{tag}{:02}{value}", value.len())
}

/// CRC-16/CCITT-FALSE: polynomial 0x1021, start value 0xFFFF, bits in normal order, no final XOR.
fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for byte in data {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

/// `id` is 13 ASCII digits.
fn check_digit_is_right(id: &[u8]) -> bool {
    let mut sum: u32 = 0;
    let mut weight: u32 = 13;
    for byte in &id[..12] {
        sum += u32::from(byte - b'0') * weight;
        weight -= 1;
    }
    let expected = (11 - sum % 11) % 10;
    u32::from(id[12] - b'0') == expected
}

impl fmt::Display for PromptPayIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PromptPayIdError::BadFormat => f.write_str(
                "a PromptPay ID is a 10-digit mobile number that starts with 0, a 13-digit tax ID or a 15-digit e-wallet ID",
            ),
            PromptPayIdError::BadCheckDigit => f.write_str("the last digit of this 13-digit ID is wrong"),
        }
    }
}

impl std::error::Error for PromptPayIdError {}

impl fmt::Display for PromptPayAmountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PromptPayAmountError::ZeroOrNegative => f.write_str("a PromptPay amount must be more than 0.00 baht"),
            PromptPayAmountError::TooLarge => f.write_str("a PromptPay amount can be at most 9999999999.99 baht"),
        }
    }
}

impl std::error::Error for PromptPayAmountError {}

/// OK cases check the right answer. NG cases check that a wrong input is refused.
#[cfg(test)]
mod tests {
    use super::{PromptPayAmountError, PromptPayId, PromptPayIdError, PromptPayKind, crc16};
    use crate::Satang;

    fn mobile() -> PromptPayId {
        PromptPayId::parse("0801234567").expect("a valid mobile number")
    }

    // ---------- parse ----------

    /// OK: the number of digits picks the kind, and the digits are kept as typed.
    /// `1101700203450` also checks the last "mod 10": 11 - 1 = 10, and 10 mod 10 = 0.
    #[test]
    fn ok_parse_each_kind() {
        let cases = [
            ("0801234567", PromptPayKind::Mobile),
            ("1101700203450", PromptPayKind::TaxId),
            ("0105555000129", PromptPayKind::TaxId),
            ("004999000000001", PromptPayKind::EWallet),
        ];
        for (raw, kind) in cases {
            let id = PromptPayId::parse(raw).expect(raw);
            assert_eq!(id.kind(), kind, "{raw}");
            assert_eq!(id.digits(), raw);
        }
    }

    /// OK: dashes and spaces are dropped, as on a printed ID card.
    #[test]
    fn ok_parse_drops_dashes_and_spaces() {
        assert_eq!(PromptPayId::parse("080-123 4567").expect("mobile").digits(), "0801234567");
        assert_eq!(PromptPayId::parse("1 1017 00203 45 0").expect("tax ID").digits(), "1101700203450");
    }

    /// NG: a wrong number of digits is refused.
    #[test]
    fn ng_parse_wrong_length_is_refused() {
        let cases =
            ["", "- -", "12345", "080123456", "08012345678", "110170020345", "11017002034500", "0049990000000010"];
        for raw in cases {
            assert_eq!(PromptPayId::parse(raw), Err(PromptPayIdError::BadFormat), "{raw}");
        }
    }

    /// NG: a character other than a digit, a dash or a space is refused. Thai digits are refused too.
    #[test]
    fn ng_parse_other_characters_are_refused() {
        let cases = ["08012345ab", "+66801234567", "080.123.4567", "๐๘๐๑๒๓๔๕๖๗"];
        for raw in cases {
            assert_eq!(PromptPayId::parse(raw), Err(PromptPayIdError::BadFormat), "{raw}");
        }
    }

    /// NG: 10 digits that do not start with 0 are not a Thai mobile number.
    #[test]
    fn ng_parse_mobile_without_leading_zero_is_refused() {
        assert_eq!(PromptPayId::parse("8801234567"), Err(PromptPayIdError::BadFormat));
    }

    /// NG: a 13-digit ID with a wrong last digit is refused.
    /// `1111111111111`: the sum is 1 x (13 + 12 + ... + 2) = 90. 90 mod 11 = 2. (11 - 2) mod 10 = 9, not 1.
    #[test]
    fn ng_parse_tax_id_with_wrong_check_digit_is_refused() {
        let cases = ["1101700203451", "0105555000120", "1111111111111"];
        for raw in cases {
            assert_eq!(PromptPayId::parse(raw), Err(PromptPayIdError::BadCheckDigit), "{raw}");
        }
    }

    // ---------- qr_text ----------

    /// OK: the text equals every row of `testdata/promptpay_vectors.tsv`, for mobile, tax ID and e-wallet.
    /// The rows come from the npm package `promptpay-qr`, not from this code. Each row is the receiver,
    /// the amount in satang (สตางค์) and the expected text, separated by tabs.
    #[test]
    fn ok_qr_text_equals_the_independent_generator() {
        let rows = include_str!("../testdata/promptpay_vectors.tsv");
        let mut checked = 0;
        for row in rows.lines() {
            if row.is_empty() {
                continue;
            }
            let columns: Vec<&str> = row.split('\t').collect();
            assert_eq!(columns.len(), 3, "{row}");
            let id = PromptPayId::parse(columns[0]).expect(row);
            let satang: i64 = columns[1].parse().expect(row);
            assert_eq!(id.qr_text(Satang(satang)), Ok(columns[2].to_string()), "{row}");
            checked += 1;
        }
        assert_eq!(checked, 20, "the vectors file lost or gained rows");
    }

    /// OK: one mobile row of the vectors file, decoded by hand, field by field.
    /// Field 29 is 37 long: sub-field 00 is 2 + 2 + 16 = 20 characters, sub-field 01 is 2 + 2 + 13 = 17.
    #[test]
    fn ok_qr_text_mobile_field_by_field() {
        let expected = [
            "000201",               // 00, length 02, version 01
            "010212",               // 01, length 02, 12 = dynamic QR
            "2937",                 // 29, length 37, the receiver
            "0016A000000677010111", //     00, length 16, PromptPay application ID
            "01130066801234567",    //     01, length 13, 080 123 4567 without its 0, after 0066
            "5802TH",               // 58, length 02, Thailand
            "5303764",              // 53, length 03, baht
            "54044.22",             // 54, length 04, 4.22 baht
            "6304",                 // 63, length 04
            "44FE",                 //     the CRC from the vectors file
        ]
        .concat();
        assert_eq!(mobile().qr_text(Satang(422)), Ok(expected));
    }

    /// OK: one tax ID row of the vectors file, decoded by hand, field by field.
    #[test]
    fn ok_qr_text_tax_id_field_by_field() {
        let id = PromptPayId::parse("0105555000129").expect("valid tax ID");
        let expected = [
            "000201",               // 00, length 02, version 01
            "010212",               // 01, length 02, 12 = dynamic QR
            "2937",                 // 29, length 37 = 20 + 17
            "0016A000000677010111", //     00, length 16, PromptPay application ID
            "02130105555000129",    //     02, length 13, the tax ID as it is
            "5802TH",               // 58, length 02, Thailand
            "5303764",              // 53, length 03, baht
            "5406145.00",           // 54, length 06, 145.00 baht
            "6304",                 // 63, length 04
            "426D",                 //     the CRC from the vectors file
        ]
        .concat();
        assert_eq!(id.qr_text(Satang(14_500)), Ok(expected));
    }

    /// OK: one e-wallet row of the vectors file, decoded by hand, field by field.
    /// Field 29 is 39 long: 20 for sub-field 00, and 2 + 2 + 15 = 19 for sub-field 03.
    #[test]
    fn ok_qr_text_e_wallet_field_by_field() {
        let id = PromptPayId::parse("004999000000001").expect("valid e-wallet ID");
        let expected = [
            "000201",               // 00, length 02, version 01
            "010212",               // 01, length 02, 12 = dynamic QR
            "2939",                 // 29, length 39 = 20 + 19
            "0016A000000677010111", //     00, length 16, PromptPay application ID
            "0315004999000000001",  //     03, length 15, the e-wallet ID as it is
            "5802TH",               // 58, length 02, Thailand
            "5303764",              // 53, length 03, baht
            "5409100000.05",        // 54, length 09, 100000.05 baht
            "6304",                 // 63, length 04
            "0BA0",                 //     the CRC from the vectors file
        ]
        .concat();
        assert_eq!(id.qr_text(Satang(10_000_005)), Ok(expected));
    }

    /// OK: the CRC of the ASCII text `123456789` is 29B1, the published check value of CRC-16/CCITT-FALSE.
    #[test]
    fn ok_crc_matches_the_published_check_value() {
        assert_eq!(crc16(b"123456789"), 0x29B1);
    }

    /// OK: 9999999999.99 baht is 13 characters, the most that field 54 allows.
    #[test]
    fn ok_qr_text_largest_amount() {
        let text = mobile().qr_text(Satang(999_999_999_999)).expect("13 characters fit");
        assert!(text.contains("54139999999999.996304"), "{text}");
    }

    /// NG: zero baht is refused.
    #[test]
    fn ng_qr_text_zero_amount_is_refused() {
        assert_eq!(mobile().qr_text(Satang::ZERO), Err(PromptPayAmountError::ZeroOrNegative));
    }

    /// NG: a negative amount is refused, also the lowest possible amount.
    #[test]
    fn ng_qr_text_negative_amount_is_refused() {
        for satang in [-1, -42_200, i64::MIN] {
            assert_eq!(mobile().qr_text(Satang(satang)), Err(PromptPayAmountError::ZeroOrNegative), "{satang}");
        }
    }

    /// NG: 10000000000.00 baht is 14 characters, one more than field 54 allows.
    #[test]
    fn ng_qr_text_amount_over_13_characters_is_refused() {
        for satang in [1_000_000_000_000, i64::MAX] {
            assert_eq!(mobile().qr_text(Satang(satang)), Err(PromptPayAmountError::TooLarge), "{satang}");
        }
    }
}
