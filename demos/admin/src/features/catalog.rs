//! The products the store sells, and the prices they sell for.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use toasty::{
    schema::Model,
    stmt::{Expr, List},
};
use topcoat::{
    Result,
    context::Cx,
    view::{NodeViewParts, PartsWriter},
};

use crate::db::{contains_pattern, db, latency};

/// How many units a product has left when its stock counts as low.
const LOW_STOCK: u32 = 10;

/// The longest name a product can have, in characters.
const MAX_NAME: usize = 80;

/// The longest SKU a product can have, in characters.
const MAX_SKU: usize = 20;

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

impl Product {
    /// Returns the product with `id`, or `None` if there is none.
    pub async fn find(cx: &Cx, id: u64) -> Result<Option<Product>> {
        let product = Product::filter_by_id(id).first().exec(&mut db(cx)).await?;
        Ok(product)
    }

    /// How much of the product is left.
    pub fn stock_level(&self) -> StockLevel {
        match self.stock {
            0 => StockLevel::Out,
            stock if stock <= LOW_STOCK => StockLevel::Low,
            _ => StockLevel::InStock,
        }
    }

    /// Changes the product to the details in `form`.
    ///
    /// If a field is invalid, the product stays unchanged and the returned errors say
    /// what is wrong with each field.
    pub async fn edit(&mut self, cx: &Cx, form: &ProductForm) -> Result<Result<(), ProductErrors>> {
        let name = form.name();
        let mut sku = form.sku();
        let price = form.price();
        let stock = form.stock();

        // A SKU can be valid on its own and still belong to another product.
        if let Ok(code) = &sku
            && let Some(other) = Product::filter_by_sku(code)
                .first()
                .exec(&mut db(cx))
                .await?
            && other.id != self.id
        {
            sku = Err("Another product already uses this SKU.");
        }

        match (name, sku, price, stock) {
            (Ok(name), Ok(sku), Ok(price), Ok(stock)) => {
                self.update()
                    .name(name)
                    .sku(sku)
                    .price(price)
                    .stock(stock)
                    .exec(&mut db(cx))
                    .await?;
                Ok(Ok(()))
            }
            (name, sku, price, stock) => Ok(Err(ProductErrors {
                name: name.err(),
                sku: sku.err(),
                price: price.err(),
                stock: stock.err(),
            })),
        }
    }
}

/// How much of a product is left in the warehouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StockLevel {
    /// Enough units for now.
    InStock,
    /// A few units left, time to order more.
    Low,
    /// No units left.
    Out,
}

impl StockLevel {
    /// Every stock level, from the most units to none.
    pub const ALL: [Self; 3] = [Self::InStock, Self::Low, Self::Out];

    /// The level's name, for display.
    pub fn label(self) -> &'static str {
        match self {
            Self::InStock => "In stock",
            Self::Low => "Low stock",
            Self::Out => "Out of stock",
        }
    }

    /// The level's name in URLs and forms. [`FromStr`] reads it back.
    pub fn slug(self) -> &'static str {
        match self {
            Self::InStock => "in-stock",
            Self::Low => "low",
            Self::Out => "out",
        }
    }

    /// Matches the products at this level.
    fn filter(self) -> Expr<bool> {
        let stock = Product::fields().stock();
        match self {
            Self::InStock => stock.gt(LOW_STOCK),
            Self::Low => stock.clone().gt(0).and(stock.le(LOW_STOCK)),
            Self::Out => stock.eq(0),
        }
    }
}

impl FromStr for StockLevel {
    type Err = UnknownStockLevel;

    fn from_str(slug: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|level| level.slug() == slug)
            .ok_or(UnknownStockLevel)
    }
}

/// The error for a string that names no [`StockLevel`].
#[derive(Debug)]
pub struct UnknownStockLevel;

/// The order to list products in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductSort {
    /// Alphabetically by name.
    #[default]
    Name,
    /// The most expensive products first.
    HighestPrice,
    /// The least expensive products first.
    LowestPrice,
    /// The products with the fewest units left first.
    LowestStock,
}

impl ProductSort {
    /// Every sort order, in the order to offer them.
    pub const ALL: [Self; 4] = [
        Self::Name,
        Self::HighestPrice,
        Self::LowestPrice,
        Self::LowestStock,
    ];

    /// The sort order's name, for display.
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::HighestPrice => "Highest price",
            Self::LowestPrice => "Lowest price",
            Self::LowestStock => "Lowest stock",
        }
    }

    /// The sort order's name in URLs and forms. [`FromStr`] reads it back.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::HighestPrice => "highest-price",
            Self::LowestPrice => "lowest-price",
            Self::LowestStock => "lowest-stock",
        }
    }
}

impl FromStr for ProductSort {
    type Err = UnknownSort;

    fn from_str(slug: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|sort| sort.slug() == slug)
            .ok_or(UnknownSort)
    }
}

/// The error for a string that names no [`ProductSort`].
#[derive(Debug)]
pub struct UnknownSort;

/// Which products to list.
#[derive(Debug, Default)]
pub struct ProductSearch<'a> {
    /// Only products at this stock level, or all products if `None`.
    pub stock: Option<StockLevel>,
    /// Text to find in the product's name or SKU. Empty text matches every product.
    pub text: &'a str,
}

impl ProductSearch<'_> {
    /// Returns the matching products on page `page` of pages with `per_page` products
    /// each, sorted by `sort`. The first page is 1.
    pub async fn page(
        &self,
        cx: &Cx,
        sort: ProductSort,
        page: usize,
        per_page: usize,
    ) -> Result<Vec<Product>> {
        latency().await;

        let fields = Product::fields();
        let products = self
            .query()
            .order_by(match sort {
                ProductSort::Name => fields.name().asc(),
                ProductSort::HighestPrice => fields.price().desc(),
                ProductSort::LowestPrice => fields.price().asc(),
                ProductSort::LowestStock => fields.stock().asc(),
            })
            .limit(per_page)
            .offset(page.saturating_sub(1) * per_page)
            .exec(&mut db(cx))
            .await?;
        Ok(products)
    }

    /// Returns how many products match.
    pub async fn count(&self, cx: &Cx) -> Result<usize> {
        let count = self.query().count().exec(&mut db(cx)).await?;
        Ok(usize::try_from(count).unwrap_or(usize::MAX))
    }

    fn query(&self) -> <Product as Model>::Query<List<Product>> {
        let stock = self.stock.map(StockLevel::filter);

        let text = self.text.trim();
        let text = (!text.is_empty()).then(|| {
            let pattern = contains_pattern(text);
            Product::fields()
                .name()
                .like_with_escape(pattern.clone(), '\\')
                .or(Product::fields().sku().like_with_escape(pattern, '\\'))
        });

        match [stock, text].into_iter().flatten().reduce(Expr::and) {
            Some(filter) => Product::filter(filter),
            None => Product::all(),
        }
    }
}

/// A product's details as typed into a form, before they are checked.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProductForm {
    pub name: String,
    pub sku: String,
    pub price: String,
    pub stock: String,
}

impl ProductForm {
    /// Returns the form filled in with the current details of `product`.
    pub fn of(product: &Product) -> Self {
        Self {
            name: product.name.clone(),
            sku: product.sku.clone(),
            price: product.price.decimal(),
            stock: product.stock.to_string(),
        }
    }

    /// The name without surrounding spaces.
    fn name(&self) -> Result<String, &'static str> {
        let name = self.name.trim();
        if name.is_empty() {
            Err("Enter a name.")
        } else if name.chars().count() > MAX_NAME {
            Err("Keep the name to 80 characters or fewer.")
        } else {
            Ok(name.to_owned())
        }
    }

    /// The SKU in capital letters, without surrounding spaces.
    fn sku(&self) -> Result<String, &'static str> {
        let sku = self.sku.trim();
        if sku.is_empty() {
            Err("Enter a SKU.")
        } else if !sku
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '-')
        {
            Err("Use only letters, numbers, and hyphens.")
        } else if sku.len() > MAX_SKU {
            Err("Keep the SKU to 20 characters or fewer.")
        } else {
            Ok(sku.to_ascii_uppercase())
        }
    }

    fn price(&self) -> Result<Money, &'static str> {
        let price = self.price.trim();
        if price.is_empty() {
            return Err("Enter a price.");
        }
        match price.parse::<Money>() {
            Ok(price) if price.0 > 0 => Ok(price),
            Ok(_) => Err("Enter a price above zero."),
            Err(InvalidMoney) => Err("Enter a price in dollars, like 24.99."),
        }
    }

    fn stock(&self) -> Result<u32, &'static str> {
        let stock = self.stock.trim();
        if stock.is_empty() {
            return Err("Enter how many units are in stock.");
        }
        stock
            .parse()
            .map_err(|_| "Enter a whole number of units, 0 or more.")
    }
}

/// What is wrong with each field of a [`ProductForm`]. A field without a problem is
/// `None`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProductErrors {
    pub name: Option<&'static str>,
    pub sku: Option<&'static str>,
    pub price: Option<&'static str>,
    pub stock: Option<&'static str>,
}

/// An amount in US dollars, stored as a whole number of cents.
///
/// Render it in a view with `(amount)` to show it like `$1,234.50`. As a model field,
/// it is stored as a single integer column of cents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, toasty::Embed)]
pub struct Money(pub i64);

impl Money {
    /// The amount as a plain number of dollars, like `1234.50`, the way number inputs
    /// show it.
    pub fn decimal(self) -> String {
        let sign = if self.0 < 0 { "-" } else { "" };
        let cents = self.0.unsigned_abs();
        format!("{sign}{}.{:02}", cents / 100, cents % 100)
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

impl FromStr for Money {
    type Err = InvalidMoney;

    /// Reads an amount the way [`Display`](fmt::Display) shows it, like `$1,234.50`.
    /// The dollar sign, the commas, and the cents are optional, so `1234.5` reads too.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (negative, text) = match text.strip_prefix('-') {
            Some(text) => (true, text),
            None => (false, text),
        };
        let text = text.strip_prefix('$').unwrap_or(text);
        let (dollars, cents) = match text.split_once('.') {
            Some((dollars, cents)) if (1..=2).contains(&cents.len()) => (dollars, cents),
            Some(_) => return Err(InvalidMoney),
            None => (text, ""),
        };
        if !dollars.chars().any(|char| char.is_ascii_digit()) {
            return Err(InvalidMoney);
        }

        let mut amount = 0_i64;
        let mut push = |char: char| {
            let digit = char.to_digit(10).ok_or(InvalidMoney)?;
            amount = amount
                .checked_mul(10)
                .and_then(|amount| amount.checked_add(i64::from(digit)))
                .ok_or(InvalidMoney)?;
            Ok(())
        };
        for char in dollars.chars().filter(|&char| char != ',') {
            push(char)?;
        }
        // Missing cents count as zero, so `24.5` is 24 dollars and 50 cents.
        let mut cents = cents.chars();
        for _ in 0..2 {
            push(cents.next().unwrap_or('0'))?;
        }

        Ok(Self(if negative { -amount } else { amount }))
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

/// The error for text that is not an amount of [`Money`].
#[derive(Debug, PartialEq, Eq)]
pub struct InvalidMoney;

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

    #[test]
    fn reads_back_what_it_shows() {
        for cents in [0, 5, 2_499, 99_999, 123_456_789, -150_000] {
            assert_eq!(Money(cents).to_string().parse(), Ok(Money(cents)));
            assert_eq!(Money(cents).decimal().parse(), Ok(Money(cents)));
        }
    }

    #[test]
    fn reads_amounts_without_all_their_cents() {
        assert_eq!("24".parse(), Ok(Money(2_400)));
        assert_eq!("24.5".parse(), Ok(Money(2_450)));
    }

    #[test]
    fn rejects_text_that_is_not_an_amount() {
        for text in [
            "",
            "$",
            ".50",
            "1.",
            "1.999",
            "1.2.3",
            "12a",
            "99999999999999999999",
        ] {
            assert_eq!(text.parse::<Money>(), Err(InvalidMoney), "{text:?}");
        }
    }

    fn product() -> Product {
        Product {
            id: 1,
            name: "Canvas Tote Bag".to_owned(),
            sku: "BAG-001".to_owned(),
            price: Money(2_450),
            stock: 12,
        }
    }

    #[test]
    fn a_products_own_form_is_valid() {
        let product = product();
        let form = ProductForm::of(&product);
        assert_eq!(form.name(), Ok(product.name));
        assert_eq!(form.sku(), Ok(product.sku));
        assert_eq!(form.price(), Ok(product.price));
        assert_eq!(form.stock(), Ok(product.stock));
    }

    #[test]
    fn tidies_names_and_skus() {
        let form = ProductForm {
            name: "  Canvas Tote Bag ".to_owned(),
            sku: " bag-001 ".to_owned(),
            ..ProductForm::default()
        };
        assert_eq!(form.name().as_deref(), Ok("Canvas Tote Bag"));
        assert_eq!(form.sku().as_deref(), Ok("BAG-001"));
    }

    #[test]
    fn rejects_invalid_fields() {
        let form = |name: &str, sku: &str, price: &str, stock: &str| ProductForm {
            name: name.to_owned(),
            sku: sku.to_owned(),
            price: price.to_owned(),
            stock: stock.to_owned(),
        };
        let blank = form(" ", " ", " ", " ");
        assert!(blank.name().is_err());
        assert!(blank.sku().is_err());
        assert!(blank.price().is_err());
        assert!(blank.stock().is_err());

        let long = form(
            &"a".repeat(MAX_NAME + 1),
            &"A".repeat(MAX_SKU + 1),
            "0",
            "-1",
        );
        assert!(long.name().is_err());
        assert!(long.sku().is_err());
        assert!(long.price().is_err(), "a price of zero is rejected");
        assert!(long.stock().is_err(), "negative stock is rejected");

        let malformed = form("Tote", "BAG 001", "-5.00", "1.5");
        assert!(malformed.sku().is_err());
        assert!(malformed.price().is_err());
        assert!(malformed.stock().is_err());
    }
}
