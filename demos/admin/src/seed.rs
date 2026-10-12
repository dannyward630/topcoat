//! Demo data for a fresh database.

use jiff::{SignedDuration, Timestamp};
use toasty::Db;

use crate::{
    features::{
        catalog::Product,
        customer::Customer,
        order::{Order, OrderItem, OrderStatus},
        user::{Role, User, hash_password},
    },
    money::Money,
};

/// The password of every demo account.
pub const DEMO_PASSWORD: &str = "backroom";

/// The demo accounts, one for each role.
pub const DEMO_USERS: [(&str, &str, Role); 3] = [
    ("Ada Lovelace", "ada@backroom.test", Role::Admin),
    ("Grace Hopper", "grace@backroom.test", Role::Staff),
    ("Alan Turing", "alan@backroom.test", Role::Viewer),
];

/// The catalog: name, SKU, and price in cents.
const PRODUCTS: [(&str, &str, i64); 40] = [
    ("Canvas Tote Bag", "BAG-001", 2400),
    ("Leather Weekender", "BAG-002", 18900),
    ("Roll-Top Backpack", "BAG-003", 12900),
    ("Waxed Cotton Cap", "CAP-001", 3200),
    ("Wool Beanie", "CAP-002", 2800),
    ("Ceramic Pour-Over", "COF-001", 3800),
    ("Burr Coffee Grinder", "COF-002", 14900),
    ("Gooseneck Kettle", "COF-003", 7900),
    ("Espresso Cups, Set of 4", "COF-004", 4400),
    ("Cold Brew Bottle", "COF-005", 2900),
    ("Linen Apron", "HOM-001", 4200),
    ("Stoneware Mug", "HOM-002", 1800),
    ("Cast Iron Skillet", "HOM-003", 5900),
    ("Olive Wood Board", "HOM-004", 4600),
    ("Beeswax Candle", "HOM-005", 2200),
    ("Wool Throw Blanket", "HOM-006", 11900),
    ("Merino Crew Socks", "KNT-001", 1900),
    ("Fisherman Sweater", "KNT-002", 16800),
    ("Cashmere Scarf", "KNT-003", 13500),
    ("Fingerless Gloves", "KNT-004", 3400),
    ("Brass Pen", "OFF-001", 5500),
    ("Dot Grid Notebook", "OFF-002", 1600),
    ("Walnut Desk Tray", "OFF-003", 6800),
    ("Leather Card Holder", "OFF-004", 3900),
    ("Desk Lamp", "OFF-005", 12400),
    ("Organic Cotton Tee", "TOP-001", 3500),
    ("Oxford Shirt", "TOP-002", 8900),
    ("Flannel Overshirt", "TOP-003", 11200),
    ("Heavyweight Hoodie", "TOP-004", 9800),
    ("Chore Coat", "TOP-005", 19800),
    ("Selvedge Jeans", "BTM-001", 15600),
    ("Canvas Work Pants", "BTM-002", 10800),
    ("Wool Trousers", "BTM-003", 14200),
    ("Leather Belt", "ACC-001", 6400),
    ("Steel Water Bottle", "ACC-002", 3600),
    ("Field Watch", "ACC-003", 24900),
    ("Sunglasses", "ACC-004", 13900),
    ("Silk Bandana", "ACC-005", 2600),
    ("Leather Sneakers", "SHO-001", 17500),
    ("Suede Chukka Boots", "SHO-002", 22900),
];

const FIRST_NAMES: [&str; 20] = [
    "Amara", "Bruno", "Chloe", "Dmitri", "Elena", "Farid", "Greta", "Hiro", "Ines", "Jonas",
    "Keiko", "Liam", "Maya", "Nikolai", "Olivia", "Pablo", "Quinn", "Rosa", "Samir", "Tess",
];

const LAST_NAMES: [&str; 16] = [
    "Andersen",
    "Bianchi",
    "Costa",
    "Dubois",
    "Eriksen",
    "Fischer",
    "Garcia",
    "Haddad",
    "Ivanova",
    "Jensen",
    "Kowalski",
    "Lindqvist",
    "Moreau",
    "Nakamura",
    "Okafor",
    "Petrov",
];

const CITIES: [&str; 10] = [
    "Amsterdam",
    "Berlin",
    "Chicago",
    "Lisbon",
    "Melbourne",
    "Montreal",
    "Oslo",
    "Seoul",
    "Toronto",
    "Vienna",
];

/// How many customers and orders to create.
const CUSTOMERS: usize = 80;
const ORDERS: usize = 400;

/// How far back the oldest orders go.
const HISTORY: SignedDuration = SignedDuration::from_hours(90 * 24);

/// Fills an empty database with demo data.
///
/// Each kind of data is only added if its table is empty, so restarting the demo keeps
/// changes made in the back office.
pub async fn seed(db: &Db) -> toasty::Result<()> {
    let mut db = db.clone();

    if User::all().first().exec(&mut db).await?.is_none() {
        seed_users(&mut db).await?;
    }
    if Product::all().first().exec(&mut db).await?.is_none() {
        seed_store(&mut db).await?;
    }

    Ok(())
}

async fn seed_users(db: &mut Db) -> toasty::Result<()> {
    for (name, email, role) in DEMO_USERS {
        // Each account gets its own salt, so equal passwords still hash differently.
        let password_hash = tokio::task::spawn_blocking(|| hash_password(DEMO_PASSWORD))
            .await
            .expect("hashing a password does not panic");

        toasty::create!(User {
            name,
            email,
            password_hash: &password_hash,
            role,
        })
        .exec(db)
        .await?;
    }

    Ok(())
}

async fn seed_store(db: &mut Db) -> toasty::Result<()> {
    // A fixed seed gives every fresh database the same data.
    let mut rng = Rng(0x5eed);
    let now = Timestamp::now();

    // One transaction makes the many inserts fast, and leaves no half-filled store
    // behind if one fails.
    let mut tx = db.transaction().await?;

    let mut products = Vec::with_capacity(PRODUCTS.len());
    for (name, sku, price) in PRODUCTS {
        let product = toasty::create!(Product {
            name,
            sku,
            price: Money(price),
            stock: rng.below(120),
        })
        .exec(&mut tx)
        .await?;
        products.push(product);
    }

    let mut customers = Vec::with_capacity(CUSTOMERS);
    for index in 0..CUSTOMERS {
        let first = FIRST_NAMES[index % FIRST_NAMES.len()];
        let last = LAST_NAMES[(index * 7) % LAST_NAMES.len()];
        let customer = toasty::create!(Customer {
            name: format!("{first} {last}"),
            // The index keeps addresses unique when a name comes up twice.
            email: format!(
                "{}.{}{index}@example.com",
                first.to_lowercase(),
                last.to_lowercase()
            ),
            city: *rng.pick(&CITIES),
            joined_at: now - HISTORY - rng.duration(HISTORY),
        })
        .exec(&mut tx)
        .await?;
        customers.push(customer);
    }

    // Orders are created oldest first, so their IDs follow the order they were placed.
    let mut placed = (0..ORDERS)
        .map(|_| now - rng.duration(HISTORY))
        .collect::<Vec<_>>();
    placed.sort();

    for placed_at in placed {
        let customer = rng.pick(&customers);
        let age = now.duration_since(placed_at);

        let mut lines = Vec::new();
        for _ in 0..=rng.below(3) {
            let product = rng.pick(&products);
            let quantity = 1 + rng.below(3);
            lines.push((product, quantity));
        }
        let total: Money = lines
            .iter()
            .map(|(product, quantity)| Money(product.price.0 * i64::from(*quantity)))
            .sum();

        let order = toasty::create!(Order {
            customer_id: customer.id,
            status: status_for_age(&mut rng, age),
            total,
            placed_at,
        })
        .exec(&mut tx)
        .await?;

        for (product, quantity) in lines {
            toasty::create!(OrderItem {
                order_id: order.id,
                product_id: product.id,
                quantity,
                unit_price: product.price,
            })
            .exec(&mut tx)
            .await?;
        }
    }

    tx.commit().await
}

/// Picks a believable status for an order placed `age` ago: recent orders are still
/// in progress, older ones are mostly delivered.
fn status_for_age(rng: &mut Rng, age: SignedDuration) -> OrderStatus {
    let roll = rng.below(100);
    if roll < 4 {
        return OrderStatus::Cancelled;
    }
    if roll < 7 {
        return OrderStatus::Refunded;
    }

    match age.as_hours() / 24 {
        0..=1 => OrderStatus::Pending,
        2..=3 => OrderStatus::Paid,
        4..=8 => OrderStatus::Shipped,
        _ => OrderStatus::Delivered,
    }
}

/// A small pseudo-random number generator, so the demo needs no extra dependency.
///
/// This is xorshift64. It is not suitable for anything that needs to be
/// unpredictable.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Returns a number from 0 up to, but not including, `bound`.
    fn below(&mut self, bound: u32) -> u32 {
        u32::try_from(self.next() % u64::from(bound)).expect("the remainder is below `bound`")
    }

    /// Returns one of `items`.
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        let index = self.next() % items.len() as u64;
        &items[usize::try_from(index).expect("the remainder is below the length")]
    }

    /// Returns a duration from zero up to, but not including, `max`.
    fn duration(&mut self, max: SignedDuration) -> SignedDuration {
        let secs = self.next() % max.as_secs().unsigned_abs();
        SignedDuration::from_secs(i64::try_from(secs).expect("the remainder is below `max`"))
    }
}
