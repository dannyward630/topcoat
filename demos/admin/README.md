# Admin Panel

Backroom is the back office of an online store, where staff sign in to manage the store. It shows how a larger Topcoat application fits together: Toasty models and migrations, password sign-in with sessions, layouts that guard pages, and a Topcoat UI shell driven by the browser runtime.

Clone the Topcoat repository:

```sh
git clone https://github.com/tokio-rs/topcoat.git
cd topcoat
```

Then run it with:

```sh
cargo topcoat dev -p admin
```

The first start creates `data.db` next to this file and fills it with demo data. The sign-in page lists the demo accounts. Delete `data.db` to start over.

## Where to look

| Path | Contents |
| --- | --- |
| [`src/app.rs`](src/app.rs) | The router and the HTML document around every page. |
| [`src/app/login.rs`](src/app/login.rs) | Signing in, and the `require_user` guard. |
| [`src/app/_admin.rs`](src/app/_admin.rs) | The sidebar layout for signed-in staff, and the dashboard. |
| [`src/app/_admin/orders.rs`](src/app/_admin/orders.rs) | The order list: a shard that re-renders as you filter and search, and a CSV export. |
| [`src/app/_admin/orders/order_id.rs`](src/app/_admin/orders/order_id.rs) | One order, and the form that moves it to its next status. |
| [`src/features/`](src/features) | The Toasty models and the rules around them. |
| [`src/seed.rs`](src/seed.rs) | The demo data written to a fresh database. |
| [`src/components/`](src/components) | Components installed with `topcoat ui add`, unchanged from the registry. |
| [`toasty/`](toasty) | The migrations, applied when the app starts. |

## Changing a model

After editing a model, generate a migration from the repository root:

```sh
cargo run -p admin -- toasty migration generate --name <name>
```

The migration is compiled into the binary and applied on the next start.

## Updating components

The components in `src/components` must match the Topcoat UI registry, which `cargo test -p admin` checks. To pull registry changes, run `topcoat ui add <name> --overwrite` in this directory.
