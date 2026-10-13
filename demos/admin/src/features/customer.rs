//! The people who buy from the store.

use jiff::Timestamp;
use topcoat::{Result, context::Cx};

use super::order::Order;
use crate::db::{db, latency};

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

    /// The initials of the customer's name, for avatars.
    pub fn initials(&self) -> String {
        self.name
            .split_whitespace()
            .filter_map(|word| word.chars().next())
            .take(2)
            .collect()
    }
}
