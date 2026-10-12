//! The people who buy from the store.

use jiff::Timestamp;

use super::order::Order;

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
