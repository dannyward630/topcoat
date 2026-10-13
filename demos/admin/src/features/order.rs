//! Orders, their line items, and how an order moves from placed to delivered.

use std::str::FromStr;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use toasty::{
    schema::Model,
    stmt::{Expr, List},
};
use topcoat::{Result, context::Cx};

use super::{
    catalog::{Money, Product},
    customer::Customer,
};
use crate::db::{db, latency};

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
    /// Returns the order with `id`, or `None` if there is none.
    pub async fn find(cx: &Cx, id: u64) -> Result<Option<Order>> {
        let order = Order::filter_by_id(id).first().exec(&mut db(cx)).await?;
        Ok(order)
    }

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

    /// What moving an order to this status is called, for buttons.
    pub fn action(self) -> &'static str {
        match self {
            Self::Pending => "Mark as pending",
            Self::Paid => "Mark as paid",
            Self::Shipped => "Mark as shipped",
            Self::Delivered => "Mark as delivered",
            Self::Cancelled => "Cancel order",
            Self::Refunded => "Refund order",
        }
    }

    /// Whether moving to this status stops the order, as cancelling or refunding it
    /// does.
    pub fn is_reversal(self) -> bool {
        matches!(self, Self::Cancelled | Self::Refunded)
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
    /// Returns the items of the order with `order_id`, with their products loaded.
    pub async fn of_order(cx: &Cx, order_id: u64) -> Result<Vec<OrderItem>> {
        latency().await;
        let items = OrderItem::filter_by_order_id(order_id)
            .include(OrderItem::fields().product())
            .order_by(OrderItem::fields().id().asc())
            .exec(&mut db(cx))
            .await?;
        Ok(items)
    }

    /// The price of all units on this line.
    pub fn subtotal(&self) -> Money {
        Money(self.unit_price.0 * i64::from(self.quantity))
    }
}

/// The order to list orders in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OrderSort {
    /// The most recently placed orders first.
    #[default]
    Newest,
    /// The earliest placed orders first.
    Oldest,
    /// The most expensive orders first.
    HighestTotal,
    /// The least expensive orders first.
    LowestTotal,
}

impl OrderSort {
    /// Every sort order, in the order to offer them.
    pub const ALL: [Self; 4] = [
        Self::Newest,
        Self::Oldest,
        Self::HighestTotal,
        Self::LowestTotal,
    ];

    /// The sort order's name, for display.
    pub fn label(self) -> &'static str {
        match self {
            Self::Newest => "Newest first",
            Self::Oldest => "Oldest first",
            Self::HighestTotal => "Highest total",
            Self::LowestTotal => "Lowest total",
        }
    }

    /// The sort order's name in URLs and forms. [`FromStr`] reads it back.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Newest => "newest",
            Self::Oldest => "oldest",
            Self::HighestTotal => "highest-total",
            Self::LowestTotal => "lowest-total",
        }
    }
}

impl FromStr for OrderSort {
    type Err = UnknownSort;

    fn from_str(slug: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|sort| sort.slug() == slug)
            .ok_or(UnknownSort)
    }
}

/// The error for a string that names no [`OrderSort`].
#[derive(Debug)]
pub struct UnknownSort;

/// Which orders to list.
#[derive(Debug, Default)]
pub struct OrderSearch<'a> {
    /// Only orders with this status, or all orders if `None`.
    pub status: Option<OrderStatus>,
    /// Text to find in the customer's name or email, or the order number. Empty text
    /// matches every order.
    pub text: &'a str,
}

impl OrderSearch<'_> {
    /// Returns the matching orders on page `page` of pages with `per_page` orders each,
    /// sorted by `sort`, with their customers and items loaded. The first page is 1.
    pub async fn page(
        &self,
        cx: &Cx,
        sort: OrderSort,
        page: usize,
        per_page: usize,
    ) -> Result<Vec<Order>> {
        latency().await;

        let orders = self
            .query()
            .include(Order::fields().customer())
            .include(Order::fields().items())
            .order_by(match sort {
                // IDs follow the order the orders were placed in.
                OrderSort::Newest => Order::fields().id().desc(),
                OrderSort::Oldest => Order::fields().id().asc(),
                OrderSort::HighestTotal => Order::fields().total().desc(),
                OrderSort::LowestTotal => Order::fields().total().asc(),
            })
            .limit(per_page)
            .offset(page.saturating_sub(1) * per_page)
            .exec(&mut db(cx))
            .await?;
        Ok(orders)
    }

    /// Returns how many orders match.
    pub async fn count(&self, cx: &Cx) -> Result<usize> {
        let count = self.query().count().exec(&mut db(cx)).await?;
        Ok(usize::try_from(count).unwrap_or(usize::MAX))
    }

    fn query(&self) -> <Order as Model>::Query<List<Order>> {
        let status = self
            .status
            .map(|status| Order::fields().status().eq(status));

        let text = self.text.trim();
        let text = (!text.is_empty()).then(|| {
            // `%` and `_` are wildcards in a LIKE pattern, so escape them to match
            // them literally.
            let mut pattern = String::from("%");
            for char in text.chars() {
                if matches!(char, '%' | '_' | '\\') {
                    pattern.push('\\');
                }
                pattern.push(char);
            }
            pattern.push('%');

            // SQLite's LIKE ignores the case of ASCII letters.
            let customer = Order::fields().customer();
            let mut matches = customer
                .name()
                .like_with_escape(pattern.clone(), '\\')
                .or(customer.email().like_with_escape(pattern, '\\'));
            if let Ok(id) = text.trim_start_matches('#').parse::<u64>() {
                matches = matches.or(Order::fields().id().eq(id));
            }
            matches
        });

        match [status, text].into_iter().flatten().reduce(Expr::and) {
            Some(filter) => Order::filter(filter),
            None => Order::all(),
        }
    }
}
