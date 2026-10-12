//! Amounts of money.

use std::fmt;

use topcoat::{
    context::Cx,
    view::{NodeViewParts, PartsWriter},
};

/// An amount in US dollars, stored as a whole number of cents.
///
/// Render it in a view with `(amount)` to show it like `$1,234.50`. As a model field,
/// it is stored as a single integer column of cents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, toasty::Embed)]
pub struct Money(pub i64);

impl Money {
    /// The amount as a plain decimal number of dollars, like `1234.50`, for files that
    /// other programs read.
    pub fn decimal(self) -> impl fmt::Display {
        struct Decimal(i64);

        impl fmt::Display for Decimal {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let sign = if self.0 < 0 { "-" } else { "" };
                let cents = self.0.unsigned_abs();
                write!(f, "{sign}{}.{:02}", cents / 100, cents % 100)
            }
        }

        Decimal(self.0)
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let cents = self.0.unsigned_abs();
        let dollars = (cents / 100).to_string();

        f.write_str(sign)?;
        f.write_str("$")?;
        // Group the dollars in threes, counting from the right.
        for (index, digit) in dollars.chars().enumerate() {
            if index > 0 && (dollars.len() - index).is_multiple_of(3) {
                f.write_str(",")?;
            }
            write!(f, "{digit}")?;
        }
        write!(f, ".{:02}", cents % 100)
    }
}

impl NodeViewParts for Money {
    fn into_view_parts(self, _cx: &Cx, parts: &mut PartsWriter<'_>) {
        parts.push_string(self.to_string());
    }
}

impl std::iter::Sum for Money {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        Self(iter.map(|money| money.0).sum())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_dollars_in_thousands() {
        assert_eq!(Money(123_456_789).to_string(), "$1,234,567.89");
        assert_eq!(Money(100_000).to_string(), "$1,000.00");
        assert_eq!(Money(99_999).to_string(), "$999.99");
    }

    #[test]
    fn keeps_leading_zeros_in_cents() {
        assert_eq!(Money(5).to_string(), "$0.05");
        assert_eq!(Money(5).decimal().to_string(), "0.05");
    }

    #[test]
    fn puts_the_sign_before_the_currency() {
        assert_eq!(Money(-150_000).to_string(), "-$1,500.00");
        assert_eq!(Money(-150_000).decimal().to_string(), "-1500.00");
    }
}
