//! Money for the shop.
//!
//! Amounts are whole satang (สตางค์). 100 satang are 1 baht (บาท).
//! A float is never used, because a float cannot hold 0.10 exactly.
//!
//! Number types in this file:
//!
//! | Type         | Range                                   | Used for                                                    |
//! | ------------ | --------------------------------------- | ----------------------------------------------------------- |
//! | `i64`        | about -9.2 x 10^18 to 9.2 x 10^18       | The amount in satang. Negative means money that is missing. |
//! | `i128`       | about -1.7 x 10^38 to 1.7 x 10^38       | Only inside `ratio`, so that `amount * numerator` cannot overflow before the division. |
//! | `u32`        | 0 to about 4.2 x 10^9                   | A quantity or a numerator. It can never be negative.        |
//! | `NonZeroU32` | 1 to about 4.2 x 10^9                   | A denominator. The compiler refuses zero, so there is no division by zero. |
//! | `u64`        | 0 to about 1.8 x 10^19                  | The size of an amount without its sign, when it is printed. |
//!
//! When an amount does not fit in `i64` the program stops with a panic.
//! A wrong amount on a bill is worse than a stopped request.
//! `Cargo.toml` keeps this check on in release builds too.

use std::fmt;
use std::iter::Sum;
use std::num::NonZeroU32;
use std::ops::{Add, Sub};

/// An amount of money in satang (สตางค์). `Satang(150)` is 1.50 baht (บาท).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Satang(pub i64);

impl Satang {
    pub const ZERO: Satang = Satang(0);

    /// The price of `qty` pieces. Example: `Satang(6050).times(3)` is `Satang(18150)`.
    pub fn times(self, qty: u32) -> Satang {
        let total = self.0 * i64::from(qty);
        Satang(total)
    }

    /// `amount * numerator / denominator`, rounded to whole satang.
    /// Used for VAT (ภาษีมูลค่าเพิ่ม) and service charge (ค่าบริการ).
    ///
    /// Rounding: half a satang or more goes away from zero, less than half goes toward zero.
    /// Examples: 0.5 becomes 1, 0.49 becomes 0, -0.5 becomes -1.
    pub fn ratio(self, numerator: u32, denominator: NonZeroU32) -> Satang {
        let product: i128 = i128::from(self.0) * i128::from(numerator);
        let divisor: i128 = i128::from(denominator.get());

        // Divide the size of the number, then put the sign back.
        // Adding half the divisor before dividing rounds half a satang up.
        let size: i128 = product.abs();
        let rounded_size: i128 = (size * 2 + divisor) / (divisor * 2);

        let mut result: i128 = rounded_size;
        if product < 0 {
            result = -rounded_size;
        }

        match i64::try_from(result) {
            Ok(amount) => Satang(amount),
            Err(_) => panic!("money does not fit in 64 bits: {result}"),
        }
    }
}

/// Prints baht with two decimals, such as `4.22` or `-0.50`. No thousands separator.
/// PromptPay (พร้อมเพย์) needs exactly this form.
impl fmt::Display for Satang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut sign: &str = "";
        if self.0 < 0 {
            sign = "-";
        }

        // unsigned_abs gives u64, because the size of i64::MIN does not fit in i64.
        let size: u64 = self.0.unsigned_abs();
        let baht: u64 = size / 100;
        let satang: u64 = size % 100;
        write!(f, "{sign}{baht}.{satang:02}")
    }
}

impl Add for Satang {
    type Output = Satang;
    fn add(self, other: Satang) -> Satang {
        Satang(self.0 + other.0)
    }
}

impl Sub for Satang {
    type Output = Satang;
    fn sub(self, other: Satang) -> Satang {
        Satang(self.0 - other.0)
    }
}

/// Adds a list of amounts. An empty list is zero.
impl Sum for Satang {
    fn sum<I: Iterator<Item = Satang>>(amounts: I) -> Satang {
        let mut total = Satang::ZERO;
        for amount in amounts {
            total = total + amount;
        }
        total
    }
}

/// OK cases check the right answer. NG cases check that a wrong input is refused.
#[cfg(test)]
mod tests {
    use super::Satang;
    use std::hint::black_box;
    use std::num::NonZeroU32;

    fn over(n: u32) -> NonZeroU32 {
        NonZeroU32::new(n).expect("tests never divide by zero")
    }

    // ---------- sum ----------

    /// OK: ten times 0.10 baht is exactly 1.00 baht. A float would give 0.9999999.
    #[test]
    fn ok_sum_of_ten_satang_ten_times_is_one_baht() {
        let total: Satang = std::iter::repeat_n(Satang(10), 10).sum();
        assert_eq!(total, Satang(100));
    }

    /// OK: an empty bill is zero.
    #[test]
    fn ok_sum_of_nothing_is_zero() {
        let total: Satang = std::iter::empty().sum();
        assert_eq!(total, Satang::ZERO);
    }

    // ---------- times, add, sub ----------

    /// OK: 3 pieces at 60.50 baht are 181.50 baht.
    #[test]
    fn ok_times_multiplies_by_quantity() {
        assert_eq!(Satang(6_050).times(3), Satang(18_150));
    }

    /// OK: zero pieces cost nothing.
    #[test]
    fn ok_times_zero_is_zero() {
        assert_eq!(Satang(6_050).times(0), Satang::ZERO);
    }

    /// OK: paying 100.00 for a 181.50 bill leaves 81.50 missing, shown as a negative amount.
    #[test]
    fn ok_subtract_can_go_below_zero() {
        assert_eq!(Satang(10_000) - Satang(18_150), Satang(-8_150));
    }

    /// NG: an amount too large for i64 stops instead of becoming a wrong number.
    #[test]
    #[should_panic(expected = "overflow")]
    fn ng_add_past_the_limit_panics() {
        let _ = black_box(Satang(i64::MAX)) + Satang(1);
    }

    /// NG: the same for subtraction below the lowest amount.
    #[test]
    #[should_panic(expected = "overflow")]
    fn ng_subtract_past_the_limit_panics() {
        let _ = black_box(Satang(i64::MIN)) - Satang(1);
    }

    /// NG: the same for a huge quantity.
    #[test]
    #[should_panic(expected = "overflow")]
    fn ng_times_past_the_limit_panics() {
        let _ = black_box(Satang(i64::MAX)).times(2);
    }

    // ---------- ratio ----------

    /// OK: the VAT inside 100.00 baht at 7% is 100 * 7 / 107 = 6.542..., so 6.54.
    #[test]
    fn ok_ratio_vat_inside_a_price() {
        assert_eq!(Satang(10_000).ratio(7, over(107)), Satang(654));
    }

    /// OK: half a satang rounds away from zero, less than half rounds toward zero.
    #[test]
    fn ok_ratio_rounding_positive() {
        assert_eq!(Satang(50).ratio(1, over(100)), Satang(1)); // 0.50 -> 1
        assert_eq!(Satang(49).ratio(1, over(100)), Satang(0)); // 0.49 -> 0
    }

    /// OK: negative amounts round the same way, so a refund mirrors a sale.
    #[test]
    fn ok_ratio_rounding_negative() {
        assert_eq!(Satang(-50).ratio(1, over(100)), Satang(-1)); // -0.50 -> -1
        assert_eq!(Satang(-49).ratio(1, over(100)), Satang(0)); // -0.49 -> 0
    }

    /// OK: the multiplication may pass the i64 limit as long as the result fits.
    #[test]
    fn ok_ratio_large_middle_step_is_fine() {
        assert_eq!(Satang(i64::MAX).ratio(1_000_000, over(1_000_000)), Satang(i64::MAX));
    }

    /// NG: a result that does not fit in i64 stops.
    #[test]
    #[should_panic(expected = "does not fit")]
    fn ng_ratio_result_past_the_limit_panics() {
        let _ = Satang(i64::MAX).ratio(2, over(1));
    }

    /// NG: a zero denominator cannot be written. `NonZeroU32::new(0)` gives nothing back.
    #[test]
    fn ng_ratio_zero_denominator_is_impossible() {
        assert_eq!(NonZeroU32::new(0), None);
    }

    // ---------- text ----------

    /// OK: two decimals, a minus sign for negatives, and the lowest possible amount.
    #[test]
    fn ok_text_has_two_decimals() {
        let cases = [
            (0, "0.00"),
            (5, "0.05"),
            (422, "4.22"),
            (100_005, "1000.05"),
            (-50, "-0.50"),
            (-12_345, "-123.45"),
            (i64::MIN, "-92233720368547758.08"),
        ];
        for (amount, text) in cases {
            assert_eq!(Satang(amount).to_string(), text, "amount {amount}");
        }
    }
}
