//! The people who buy from the store.

use std::str::FromStr;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use toasty::{schema::Model, stmt::List};
use topcoat::{Result, context::Cx};

use super::{catalog::Money, order::Order};
use crate::db::{contains_pattern, db, latency};

/// Someone who has an account with the store.
#[derive(Debug, toasty::Model)]
pub struct Customer {
    #[key]
    #[auto]
    pub id: u64,

    pub name: String,

    #[unique]
    pub email: String,

    pub city: String,

    /// When the customer created their account.
    pub joined_at: Timestamp,

    #[has_many]
    pub orders: toasty::Deferred<Vec<Order>>,
}

impl Customer {
    /// Returns the customer with `id`.
    pub async fn get(cx: &Cx, id: u64) -> Result<Customer> {
        latency().await;
        let customer = Customer::get_by_id(&mut db(cx), &id).await?;
        Ok(customer)
    }

    /// Returns the customer with `id`, or `None` if there is none.
    pub async fn find(cx: &Cx, id: u64) -> Result<Option<Customer>> {
        let customer = Customer::filter_by_id(id).first().exec(&mut db(cx)).await?;
        Ok(customer)
    }

    /// The initials of the customer's name, for avatars.
    pub fn initials(&self) -> String {
        self.name
            .split_whitespace()
            .filter_map(|word| word.chars().next())
            .take(2)
            .collect()
    }
}

/// What a customer bought, from their orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spending {
    /// How many orders the customer placed, including the cancelled and refunded ones.
    pub orders: usize,
    /// The total of the orders that were not cancelled or refunded.
    pub spent: Money,
    /// How many orders count towards `spent`.
    kept: i64,
}

impl Spending {
    /// What the customer who placed `orders` bought.
    pub fn of(orders: &[Order]) -> Self {
        let kept = orders.iter().filter(|order| !order.status.is_reversal());
        Self {
            orders: orders.len(),
            spent: kept.clone().map(|order| order.total).sum(),
            kept: i64::try_from(kept.count()).unwrap_or(i64::MAX),
        }
    }

    /// The average total of the orders that count towards `spent`, or `None` if
    /// there are none.
    pub fn average_order(self) -> Option<Money> {
        (self.kept > 0).then(|| Money(self.spent.0 / self.kept))
    }
}

/// The order to list customers in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CustomerSort {
    /// By name, from A to Z.
    #[default]
    Name,
    /// The customers who joined most recently first.
    Newest,
    /// The customers who joined earliest first.
    Oldest,
}

impl CustomerSort {
    /// Every sort order, in the order to offer them.
    pub const ALL: [Self; 3] = [Self::Name, Self::Newest, Self::Oldest];

    /// The sort order's name, for display.
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Newest => "Newest customers",
            Self::Oldest => "Oldest customers",
        }
    }

    /// The sort order's name in URLs and forms. [`FromStr`] reads it back.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Newest => "newest",
            Self::Oldest => "oldest",
        }
    }
}

impl FromStr for CustomerSort {
    type Err = UnknownSort;

    fn from_str(slug: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|sort| sort.slug() == slug)
            .ok_or(UnknownSort)
    }
}

/// The error for a string that names no [`CustomerSort`].
#[derive(Debug)]
pub struct UnknownSort;

/// Which customers to list.
#[derive(Debug, Default)]
pub struct CustomerSearch<'a> {
    /// Text to find in the customer's name or email. Empty text matches every
    /// customer.
    pub text: &'a str,
}

impl CustomerSearch<'_> {
    /// Returns the matching customers on page `page` of pages with `per_page`
    /// customers each, sorted by `sort`, with their orders loaded. The first page
    /// is 1.
    pub async fn page(
        &self,
        cx: &Cx,
        sort: CustomerSort,
        page: usize,
        per_page: usize,
    ) -> Result<Vec<Customer>> {
        latency().await;

        let customers = self
            .query()
            .include(Customer::fields().orders())
            .order_by(match sort {
                CustomerSort::Name => Customer::fields().name().asc(),
                CustomerSort::Newest => Customer::fields().joined_at().desc(),
                CustomerSort::Oldest => Customer::fields().joined_at().asc(),
            })
            .limit(per_page)
            .offset(page.saturating_sub(1) * per_page)
            .exec(&mut db(cx))
            .await?;
        Ok(customers)
    }

    /// Returns how many customers match.
    pub async fn count(&self, cx: &Cx) -> Result<usize> {
        let count = self.query().count().exec(&mut db(cx)).await?;
        Ok(usize::try_from(count).unwrap_or(usize::MAX))
    }

    fn query(&self) -> <Customer as Model>::Query<List<Customer>> {
        let text = self.text.trim();
        if text.is_empty() {
            return Customer::all();
        }
        let pattern = contains_pattern(text);
        let fields = Customer::fields();
        Customer::filter(
            fields
                .name()
                .like_with_escape(pattern.clone(), '\\')
                .or(fields.email().like_with_escape(pattern, '\\')),
        )
    }
}
