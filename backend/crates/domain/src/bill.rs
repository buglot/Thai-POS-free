//! The bill: subtotal, discount (ส่วนลด), service charge (ค่าบริการ), VAT (ภาษีมูลค่าเพิ่ม) and total.
//!
//! The steps, in this order:
//!
//! 1. Subtotal: each line's unit price times its quantity, added up.
//! 2. Discount: the amount asked, but never more than the subtotal. So the bill never goes below zero.
//! 3. Net: subtotal minus discount.
//! 4. Service charge: the service rate of the net. A discount lowers the service charge too.
//! 5. Base: net plus service charge.
//! 6. VAT, in one of three modes:
//!    - `None`: no VAT. The total is the base.
//!    - `Inside`: the prices already hold VAT. The total is the base.
//!      The VAT part is `base * rate / (10_000 + rate)`.
//!    - `OnTop`: VAT is `base * rate / 10_000`. The total is the base plus VAT.
//!      So VAT is charged on the service charge as well.
//!
//! Each division rounds to whole satang (สตางค์) with `Satang::ratio`. Half a satang or more rounds up.
//!
//! Types in this file:
//!
//! | Type         | Holds                                          | Why                                                         |
//! | ------------ | ---------------------------------------------- | ----------------------------------------------------------- |
//! | `Rate`       | `u32` basis points, 0 to 10,000. 700 is 7%.    | Whole numbers keep 7.25% exact. `u32` cannot be negative.   |
//! | `VatMode`    | `None`, `Inside(Rate)` or `OnTop(Rate)`        | A VAT rate cannot exist without its mode, nor the reverse.  |
//! | `BillLine`   | a unit price in `Satang`, a quantity in `u32`  | A quantity cannot be negative.                              |
//! | `Bill`       | five amounts in `Satang`                       | The answer, ready to show or to store.                      |
//! | `NonZeroU32` | 1 to about 4.2 x 10^9                          | The denominator `Satang::ratio` needs. It cannot be zero.   |
//!
//! Wrong input:
//!
//! - A rate above 100% is refused: `Rate::from_basis_points` gives `None`.
//!   VAT in Thailand is 7% and a service charge is usually 10%, so a higher number is a typing mistake.
//! - A negative unit price or a negative discount stops with a panic.
//!   The menu (US-005) and the discount screen (US-013) refuse them before they reach a bill.
//!   A wrong amount on a bill is worse than a stopped request.

use crate::money::Satang;
use std::num::NonZeroU32;

/// 100% in basis points.
const HUNDRED_PERCENT: u32 = 10_000;

/// A rate for VAT or service charge, in basis points. 1 basis point is 0.01%.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate(u32);

impl Rate {
    pub const ZERO: Rate = Rate(0);

    /// The rate for 0 to 10,000 basis points (0% to 100%). `None` above 100%.
    pub fn from_basis_points(basis_points: u32) -> Option<Rate> {
        if basis_points > HUNDRED_PERCENT {
            return None;
        }
        Some(Rate(basis_points))
    }

    /// This rate of `amount`. 7% of 110.00 is 7.70.
    fn of(self, amount: Satang) -> Satang {
        let denominator = NonZeroU32::new(HUNDRED_PERCENT).expect("100% is not zero");
        amount.ratio(self.0, denominator)
    }

    /// The tax at this rate that is already inside `amount`. The 7% inside 107.00 is 7.00.
    fn included_in(self, amount: Satang) -> Satang {
        let denominator = NonZeroU32::new(HUNDRED_PERCENT + self.0).expect("100% plus a rate is not zero");
        amount.ratio(self.0, denominator)
    }
}

/// How the shop charges VAT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VatMode {
    /// No VAT, for example a small shop that is not VAT registered.
    None,
    /// Menu prices already hold VAT. The customer pays the menu price.
    Inside(Rate),
    /// VAT is added on top of the menu price.
    OnTop(Rate),
}

/// One row of an order: a unit price and how many.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BillLine {
    pub unit_price: Satang,
    pub qty: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bill {
    pub subtotal: Satang,
    /// The discount given. It is less than the discount asked when the subtotal is smaller.
    pub discount: Satang,
    pub service: Satang,
    /// With `VatMode::Inside` this is the VAT part of `total`. It is not added again.
    pub vat: Satang,
    pub total: Satang,
}

impl Bill {
    pub fn compute(lines: &[BillLine], discount_asked: Satang, service_rate: Rate, vat_mode: VatMode) -> Bill {
        assert!(discount_asked >= Satang::ZERO, "a discount cannot be negative: {discount_asked}");

        let mut subtotal = Satang::ZERO;
        for line in lines {
            assert!(line.unit_price >= Satang::ZERO, "a price cannot be negative: {}", line.unit_price);
            subtotal = subtotal + line.unit_price.times(line.qty);
        }

        let mut discount = discount_asked;
        if discount > subtotal {
            discount = subtotal;
        }

        let net = subtotal - discount;
        let service = service_rate.of(net);
        let base = net + service;

        let mut vat = Satang::ZERO;
        let mut total = base;
        match vat_mode {
            VatMode::None => {}
            VatMode::Inside(rate) => {
                vat = rate.included_in(base);
            }
            VatMode::OnTop(rate) => {
                vat = rate.of(base);
                total = base + vat;
            }
        }

        Bill { subtotal, discount, service, vat, total }
    }
}

/// Each OK case is a hand-worked example. The arithmetic is in the comments, in baht (บาท).
#[cfg(test)]
mod tests {
    use super::{Bill, BillLine, Rate, VatMode};
    use crate::money::Satang;

    /// A line with its unit price in satang.
    fn line(unit_price: i64, qty: u32) -> BillLine {
        BillLine { unit_price: Satang(unit_price), qty }
    }

    fn rate(basis_points: u32) -> Rate {
        Rate::from_basis_points(basis_points).expect("tests use rates up to 100%")
    }

    // ---------- the three VAT modes ----------

    /// OK: with no VAT and no service charge the total is the sum of the lines.
    #[test]
    fn ok_no_vat_total_is_the_sum_of_the_lines() {
        // 2 x 60.00 + 1 x 25.00 = 145.00
        let lines = [line(6_000, 2), line(2_500, 1)];
        let bill = Bill::compute(&lines, Satang::ZERO, Rate::ZERO, VatMode::None);
        let expected = Bill {
            subtotal: Satang(14_500),
            discount: Satang::ZERO,
            service: Satang::ZERO,
            vat: Satang::ZERO,
            total: Satang(14_500),
        };
        assert_eq!(bill, expected);
    }

    /// OK: VAT inside the price is reported but does not change the total.
    #[test]
    fn ok_vat_inside_does_not_change_the_total() {
        // 2 x 60.00 + 1 x 25.00 = 145.00
        // VAT part: 145.00 x 700 / 10,700 = 9.4859... -> 9.49
        let lines = [line(6_000, 2), line(2_500, 1)];
        let bill = Bill::compute(&lines, Satang::ZERO, Rate::ZERO, VatMode::Inside(rate(700)));
        let expected = Bill {
            subtotal: Satang(14_500),
            discount: Satang::ZERO,
            service: Satang::ZERO,
            vat: Satang(949),
            total: Satang(14_500),
        };
        assert_eq!(bill, expected);
    }

    /// OK: VAT on top is charged on the price plus the service charge.
    #[test]
    fn ok_vat_on_top_is_charged_on_price_and_service() {
        // 1 x 100.00 = 100.00
        // service: 100.00 x 10% = 10.00
        // VAT: (100.00 + 10.00) x 7% = 7.70
        // total: 110.00 + 7.70 = 117.70
        let bill = Bill::compute(&[line(10_000, 1)], Satang::ZERO, rate(1_000), VatMode::OnTop(rate(700)));
        let expected = Bill {
            subtotal: Satang(10_000),
            discount: Satang::ZERO,
            service: Satang(1_000),
            vat: Satang(770),
            total: Satang(11_770),
        };
        assert_eq!(bill, expected);
    }

    // ---------- discount and service charge ----------

    /// OK: the service charge is taken from the amount after the discount.
    #[test]
    fn ok_service_is_charged_after_the_discount() {
        // 1 x 200.00 = 200.00, discount 100.00, net 100.00
        // service: 100.00 x 10% = 10.00, not 200.00 x 10% = 20.00
        let bill = Bill::compute(&[line(20_000, 1)], Satang(10_000), rate(1_000), VatMode::None);
        let expected = Bill {
            subtotal: Satang(20_000),
            discount: Satang(10_000),
            service: Satang(1_000),
            vat: Satang::ZERO,
            total: Satang(11_000),
        };
        assert_eq!(bill, expected);
    }

    /// OK: a discount larger than the subtotal is cut to the subtotal, so the bill is zero, not negative.
    #[test]
    fn ok_discount_larger_than_the_bill_stops_at_zero() {
        // 1 x 50.00 = 50.00, discount asked 9,999.99, discount given 50.00
        // net 0.00, so service, VAT and total are 0.00
        let bill = Bill::compute(&[line(5_000, 1)], Satang(999_999), rate(1_000), VatMode::OnTop(rate(700)));
        let expected = Bill {
            subtotal: Satang(5_000),
            discount: Satang(5_000),
            service: Satang::ZERO,
            vat: Satang::ZERO,
            total: Satang::ZERO,
        };
        assert_eq!(bill, expected);
    }

    /// OK: a full restaurant bill with a discount, a service charge and VAT inside.
    #[test]
    fn ok_discount_service_and_vat_inside_together() {
        // 3 x 89.00 = 267.00, discount 17.00, net 250.00
        // service: 250.00 x 10% = 25.00, base 275.00
        // VAT part: 275.00 x 700 / 10,700 = 17.9906... -> 17.99
        let bill = Bill::compute(&[line(8_900, 3)], Satang(1_700), rate(1_000), VatMode::Inside(rate(700)));
        let expected = Bill {
            subtotal: Satang(26_700),
            discount: Satang(1_700),
            service: Satang(2_500),
            vat: Satang(1_799),
            total: Satang(27_500),
        };
        assert_eq!(bill, expected);
    }

    // ---------- rounding and edges ----------

    /// OK: half a satang rounds up, and less than half rounds down.
    #[test]
    fn ok_half_a_satang_rounds_up() {
        // 1 x 12.35 = 12.35
        // service: 12.35 x 10% = 1.235 -> 1.24 (exactly half a satang, so up)
        // VAT: (12.35 + 1.24) x 7% = 13.59 x 7% = 0.9513 -> 0.95 (less than half, so down)
        // total: 13.59 + 0.95 = 14.54
        let bill = Bill::compute(&[line(1_235, 1)], Satang::ZERO, rate(1_000), VatMode::OnTop(rate(700)));
        let expected = Bill {
            subtotal: Satang(1_235),
            discount: Satang::ZERO,
            service: Satang(124),
            vat: Satang(95),
            total: Satang(1_454),
        };
        assert_eq!(bill, expected);
    }

    /// OK: a bill with no lines is zero in every field, whatever the rates.
    #[test]
    fn ok_empty_bill_is_zero() {
        let bill = Bill::compute(&[], Satang::ZERO, rate(1_000), VatMode::Inside(rate(700)));
        let expected = Bill {
            subtotal: Satang::ZERO,
            discount: Satang::ZERO,
            service: Satang::ZERO,
            vat: Satang::ZERO,
            total: Satang::ZERO,
        };
        assert_eq!(bill, expected);
    }

    // ---------- wrong input ----------

    /// OK: 0% and 100% are the lowest and highest rates allowed.
    #[test]
    fn ok_rate_from_0_to_100_percent_is_allowed() {
        assert_eq!(Rate::from_basis_points(0), Some(Rate::ZERO));
        assert!(Rate::from_basis_points(10_000).is_some());
    }

    /// NG: a rate above 100% is refused.
    #[test]
    fn ng_rate_above_100_percent_is_refused() {
        assert_eq!(Rate::from_basis_points(10_001), None);
        assert_eq!(Rate::from_basis_points(u32::MAX), None);
    }

    /// NG: a negative unit price stops the bill.
    #[test]
    #[should_panic(expected = "a price cannot be negative: -1.00")]
    fn ng_negative_price_panics() {
        let _ = Bill::compute(&[line(-100, 1)], Satang::ZERO, Rate::ZERO, VatMode::None);
    }

    /// NG: a negative discount stops the bill. It would raise the total.
    #[test]
    #[should_panic(expected = "a discount cannot be negative: -1.00")]
    fn ng_negative_discount_panics() {
        let _ = Bill::compute(&[line(10_000, 1)], Satang(-100), Rate::ZERO, VatMode::None);
    }
}
