//! The products the store sells, and the prices they sell for.

use std::fmt;

use topcoat::{
    context::Cx,
    view::{NodeViewParts, PartsWriter},
};

/// A product in the store's catalog.
#[derive(Debug, toasty::Model)]
pub struct Product {
    #[key]
    #[auto]
    pub id: u64,

    pub name: String,

    /// The stock keeping unit, a short code that identifies the product.
    #[unique]
    pub sku: String,

    pub price: Money,

    /// How many units are in the warehouse.
    pub stock: u32,
}

/// An amount in US dollars, stored as a whole number of cents.
///
/// Render it in a view with `(amount)` to show it like `$1,234.50`. As a model field,
/// it is stored as a single integer column of cents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, toasty::Embed)]
pub struct Money(pub i64);

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
    }

    #[test]
    fn puts_the_sign_before_the_currency() {
        assert_eq!(Money(-150_000).to_string(), "-$1,500.00");
    }
}
