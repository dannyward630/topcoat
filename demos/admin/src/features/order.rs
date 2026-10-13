//! Orders, their line items, and how an order moves from placed to delivered.

use std::str::FromStr;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use topcoat::{Result, context::Cx};

use super::{
    catalog::{Money, Product},
    customer::Customer,
};
use crate::db::db;

/// A purchase by a customer.
#[derive(Debug, toasty::Model)]
#[index(status)]
pub struct Order {
    #[key]
    #[auto]
    pub id: u64,

    #[index]
    pub customer_id: u64,

    #[belongs_to]
    pub customer: toasty::Deferred<Customer>,

    #[has_many]
    pub items: toasty::Deferred<Vec<OrderItem>>,

    pub status: OrderStatus,

    /// The sum of the line items, kept on the order so lists do not load the items.
    pub total: Money,

    pub placed_at: Timestamp,
}

impl Order {
    /// Moves the order to `status`, if the current status allows it.
    ///
    /// Returns `false` and leaves the order unchanged if it does not.
    pub async fn advance(&mut self, cx: &Cx, status: OrderStatus) -> Result<bool> {
        if !self.status.next().contains(&status) {
            return Ok(false);
        }
        self.update().status(status).exec(&mut db(cx)).await?;
        Ok(true)
    }
}

/// Where an order is on its way to the customer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, toasty::Embed)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    /// Placed, waiting for payment.
    Pending,
    /// Paid, waiting to be packed and shipped.
    Paid,
    /// Handed to the carrier.
    Shipped,
    /// Received by the customer.
    Delivered,
    /// Stopped before it shipped.
    Cancelled,
    /// Paid for and then refunded.
    Refunded,
}

impl OrderStatus {
    /// Every status, in the order an order moves through them.
    pub const ALL: [Self; 6] = [
        Self::Pending,
        Self::Paid,
        Self::Shipped,
        Self::Delivered,
        Self::Cancelled,
        Self::Refunded,
    ];

    /// The status's name, for display.
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Paid => "Paid",
            Self::Shipped => "Shipped",
            Self::Delivered => "Delivered",
            Self::Cancelled => "Cancelled",
            Self::Refunded => "Refunded",
        }
    }

    /// The status's name in URLs and forms. [`FromStr`] reads it back.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Paid => "paid",
            Self::Shipped => "shipped",
            Self::Delivered => "delivered",
            Self::Cancelled => "cancelled",
            Self::Refunded => "refunded",
        }
    }

    /// The statuses an order can move to from this one.
    pub fn next(self) -> &'static [Self] {
        match self {
            Self::Pending => &[Self::Paid, Self::Cancelled],
            Self::Paid => &[Self::Shipped, Self::Refunded],
            Self::Shipped => &[Self::Delivered],
            Self::Delivered | Self::Cancelled | Self::Refunded => &[],
        }
    }
}

impl FromStr for OrderStatus {
    type Err = UnknownStatus;

    fn from_str(slug: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|status| status.slug() == slug)
            .ok_or(UnknownStatus)
    }
}

/// The error for a string that names no [`OrderStatus`].
#[derive(Debug)]
pub struct UnknownStatus;

/// One product in an order, with the quantity and the price paid for it.
#[derive(Debug, toasty::Model)]
pub struct OrderItem {
    #[key]
    #[auto]
    pub id: u64,

    #[index]
    pub order_id: u64,

    #[belongs_to]
    pub order: toasty::Deferred<Order>,

    #[index]
    pub product_id: u64,

    #[belongs_to]
    pub product: toasty::Deferred<Product>,

    pub quantity: u32,

    /// The price of one unit when the order was placed. It does not follow later
    /// changes to the product's price.
    pub unit_price: Money,
}

impl OrderItem {
    /// The price of all units on this line.
    pub fn subtotal(&self) -> Money {
        Money(self.unit_price.0 * i64::from(self.quantity))
    }
}
