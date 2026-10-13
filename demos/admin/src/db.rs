//! The database connection and its migrations.

use std::time::Duration;

use toasty::{Db, migration::MigrationSet};
use toasty_cli::{Config, MigrationConfig, ToastyCli};
use topcoat::context::{Cx, app_context};

/// The SQLite database file, created next to the package manifest when it is first
/// opened, so the demo finds it from any working directory.
const DATABASE_URL: &str = concat!("sqlite:", env!("CARGO_MANIFEST_DIR"), "/data.db");

/// The directory holding the migration history, snapshots, and SQL files.
const MIGRATIONS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/toasty");

/// How long [`latency`] waits.
const LATENCY: Duration = Duration::from_millis(600);

/// The migrations in `toasty/`, compiled into the binary.
const MIGRATIONS: MigrationSet = toasty::embed_migrations!();

/// Opens the database with every model in the application.
pub async fn connect() -> toasty::Result<Db> {
    Db::builder()
        .models(toasty::models!(crate::*))
        .connect(DATABASE_URL)
        .await
}

/// Applies the migrations the database has not seen yet.
///
/// After changing a model, generate a new migration with
/// `cargo run -p admin -- toasty migration generate`. The next start applies it.
pub async fn migrate(db: &Db) -> toasty::Result<()> {
    MIGRATIONS.apply(db).await?;
    Ok(())
}

/// The Toasty command line, which manages migrations in `toasty/`.
pub fn cli(db: Db) -> ToastyCli {
    let config = Config::new().migration(MigrationConfig::new().path(MIGRATIONS_DIR));
    ToastyCli::with_config(db, config)
}

/// Returns the database handle from the router's app context. Cloning it is cheap
/// because it refers to a shared connection pool.
pub fn db(cx: &Cx) -> Db {
    app_context::<Db>(cx).clone()
}

/// Waits a moment, like a busy database would before it answers.
///
/// The demo database answers instantly, so the pages' loading states would flash by
/// too fast to see. Call it before a query whose page shows a skeleton while it loads.
pub async fn latency() {
    tokio::time::sleep(LATENCY).await;
}
