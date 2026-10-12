//! The products the store sells.

use crate::money::Money;

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
