use std::fmt;
use std::iter::Sum;
use std::ops::{Add, Sub};

/// Thai money in whole satang. 100 satang are 1 baht. Never a float, which cannot hold 0.10 exactly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Satang(pub i64);

impl Satang {
    pub const ZERO: Satang = Satang(0);

    /// The price of `qty` pieces.
    pub fn times(self, qty: u32) -> Satang {
        Satang(self.0 * i64::from(qty))
    }

    /// `self * numerator / denominator`. Half a satang rounds away from zero.
    ///
    /// # Panics
    /// If the result does not fit, because a wrong amount is worse than a stopped request.
    pub fn ratio(self, numerator: u32, denominator: std::num::NonZeroU32) -> Satang {
        let product = i128::from(self.0) * i128::from(numerator);
        let divisor = i128::from(denominator.get());
        let rounded = (product.abs() * 2 + divisor) / (divisor * 2) * product.signum();
        Satang(i64::try_from(rounded).expect("money does not fit in 64 bits"))
    }
}

/// Baht with two decimals, such as `4.22`. No thousands separator.
impl fmt::Display for Satang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let n = self.0.unsigned_abs();
        write!(f, "{sign}{}.{:02}", n / 100, n % 100)
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

impl Sum for Satang {
    fn sum<I: Iterator<Item = Satang>>(amounts: I) -> Satang {
        amounts.fold(Satang::ZERO, Add::add)
    }
}

#[cfg(test)]
mod tests {
    use super::Satang;
    use std::num::NonZeroU32;

    fn over(n: u32) -> NonZeroU32 {
        NonZeroU32::new(n).expect("tests never divide by zero")
    }

    #[test]
    fn adding_ten_satang_ten_times_is_exactly_one_baht() {
        let total: Satang = std::iter::repeat_n(Satang(10), 10).sum();
        assert_eq!(total, Satang(100));
    }

    #[test]
    fn times_and_subtract() {
        assert_eq!(Satang(6_050).times(3), Satang(18_150));
        assert_eq!(Satang(10_000) - Satang(18_150), Satang(-8_150));
    }

    #[test]
    fn ratio_rounds_half_a_satang_away_from_zero() {
        assert_eq!(Satang(49).ratio(1, over(100)), Satang(0));
        assert_eq!(Satang(50).ratio(1, over(100)), Satang(1));
        assert_eq!(Satang(-50).ratio(1, over(100)), Satang(-1));
        assert_eq!(Satang(-49).ratio(1, over(100)), Satang(0));
    }

    #[test]
    fn ratio_finds_seven_percent_vat_inside_a_price() {
        // 100.00 * 7 / 107 = 6.542...
        assert_eq!(Satang(10_000).ratio(7, over(107)), Satang(654));
    }

    #[test]
    fn ratio_does_not_overflow_in_the_middle() {
        assert_eq!(Satang(i64::MAX).ratio(1_000_000, over(1_000_000)), Satang(i64::MAX));
    }

    #[test]
    #[should_panic(expected = "does not fit")]
    fn ratio_refuses_a_result_that_does_not_fit() {
        let _ = Satang(i64::MAX).ratio(2, over(1));
    }

    #[test]
    #[should_panic(expected = "overflow")]
    fn addition_refuses_to_wrap_around() {
        let _ = std::hint::black_box(Satang(i64::MAX)) + Satang(1);
    }

    #[test]
    fn displays_two_decimals() {
        for (amount, text) in [
            (0, "0.00"),
            (5, "0.05"),
            (422, "4.22"),
            (100_005, "1000.05"),
            (-50, "-0.50"),
            (-12_345, "-123.45"),
            (i64::MIN, "-92233720368547758.08"),
        ] {
            assert_eq!(Satang(amount).to_string(), text);
        }
    }
}
