//! One order, with its line items and customer, where staff move it along.

use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    icon::{icon, iconify::iconify_icon},
    router::{
        content::Form,
        error::{RouterErrorExt, SeeOther, bad_request, forbidden, see_other},
        href, module_param, page, path_param, route,
    },
    runtime::link,
    view::{View, attributes, component, view},
};

use super::status_badge;
use crate::{
    app::{_admin::display_date, login::require_user},
    components::{
        button::{ButtonSize, ButtonVariant, button, button_variants},
        card::{card, card_content, card_description, card_footer, card_header, card_title},
        table::{table, table_body, table_cell, table_head, table_header, table_row},
    },
    db::db,
    features::{
        order::{Order, OrderStatus},
        user::User,
    },
};

module_param!(pub order_id: u64, error = not_found);

/// Loads the order in the URL with its customer, line items, and their products.
async fn load(cx: &Cx) -> Result<Order> {
    let id = *path_param::<OrderId>(cx)?;
    let order = Order::filter_by_id(id)
        .include(Order::fields().customer())
        .include(Order::fields().items().product())
        .first()
        .exec(&mut db(cx))
        .await?
        .ok_or_not_found()?;
    Ok(order)
}

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let user = require_user(cx).await?;
    let order = load(cx).await?;

    Ok(view! {
        let customer = order.customer.get();
        let items = order.items.get();

        <div>
            link(
                href: href!(super::page),
                attrs: attributes! {
                    class=(button_variants(ButtonVariant::Ghost, ButtonSize::Sm))
                },
                icon(data: iconify_icon!("lucide:arrow-left"))
                "Orders"
            )
        </div>

        <header class="flex flex-wrap items-center gap-x-3 gap-y-1">
            <h1 class="text-2xl font-semibold tracking-tight">
                "Order #"
                (order.id)
            </h1>
            status_badge(status: order.status)
            <p class="w-full text-muted-foreground">
                "Placed on "
                (display_date(order.placed_at))
            </p>
        </header>

        <div class="grid items-start gap-6 lg:grid-cols-3">
            card(
                attrs: attributes! { class="lg:col-span-2" },
                card_header(card_title("Items"))
                card_content(
                    table(
                        table_header(
                            table_row(
                                table_head("Product")
                                table_head(
                                    attrs: attributes! { class="text-right" },
                                    "Price"
                                )
                                table_head(
                                    attrs: attributes! { class="text-right" },
                                    "Quantity"
                                )
                                table_head(
                                    attrs: attributes! { class="text-right" },
                                    "Subtotal"
                                )
                            )
                        )
                        table_body(
                            for item in items {
                                let product = item.product.get();
                                table_row(
                                    table_cell(
                                        <div class="flex flex-col">
                                            <span class="font-medium">(&product.name)</span>
                                            <span class="text-xs text-muted-foreground">
                                                (&product.sku)
                                            </span>
                                        </div>
                                    )
                                    table_cell(
                                        attrs: attributes! { class="text-right tabular-nums" },
                                        (item.unit_price)
                                    )
                                    table_cell(
                                        attrs: attributes! { class="text-right tabular-nums" },
                                        (item.quantity)
                                    )
                                    table_cell(
                                        attrs: attributes! { class="text-right tabular-nums" },
                                        (item.subtotal())
                                    )
                                )
                            }
                        )
                    )
                )
                card_footer(
                    attrs: attributes! { class="justify-between font-medium" },
                    <span>"Total"</span>
                    <span class="tabular-nums">(order.total)</span>
                )
            )

            <div class="flex flex-col gap-6">
                card(
                    card_header(card_title("Customer"))
                    card_content(
                        attrs: attributes! { class="flex flex-col gap-1 text-sm" },
                        <span class="font-medium">(&customer.name)</span>
                        <a
                            href=(format!("mailto:{}", customer.email))
                            class="text-muted-foreground hover:underline"
                        >
                            (&customer.email)
                        </a>
                        <span class="text-muted-foreground">(&customer.city)</span>
                        <span class="text-muted-foreground">
                            "Customer since "
                            (display_date(customer.joined_at))
                        </span>
                    )
                )
                status_card(order: &order, user: user)
            </div>
        </div>
    })
}

/// The order's next steps, as buttons for the user to take one.
#[component]
async fn status_card(order: &Order, user: &User) -> Result<impl View> {
    let next = order.status.next();

    Ok(view! {
        card(
            card_header(
                card_title("Status")
                card_description(
                    if next.is_empty() {
                        "This order is closed."
                    } else if !user.role.can_edit() {
                        "Your role can view orders but not change them."
                    } else {
                        "Move the order to its next step."
                    }
                )
            )
            if !next.is_empty() && user.role.can_edit() {
                card_content(
                    attrs: attributes! { class="flex flex-wrap gap-2" },
                    for status in next {
                        <form method="post" action=(href!(advance, OrderId(order.id)))>
                            <input type="hidden" name="status" value=(status.slug())>
                            button(
                                variant: step_variant(*status),
                                size: ButtonSize::Sm,
                                attrs: attributes! { type="submit" },
                                "Mark as "
                                (status.label().to_lowercase())
                            )
                        </form>
                    }
                )
            }
        )
    })
}

/// The button style for moving an order to `status`: stopping an order looks
/// destructive, moving it forward looks like the main action.
fn step_variant(status: OrderStatus) -> ButtonVariant {
    match status {
        OrderStatus::Cancelled | OrderStatus::Refunded => ButtonVariant::Destructive,
        _ => ButtonVariant::Primary,
    }
}

#[derive(Deserialize)]
pub struct Advance {
    status: OrderStatus,
}

/// Moves the order in the URL to the posted status.
#[route(POST)]
async fn advance(cx: &Cx, Form(form): Form<Advance>) -> Result<SeeOther> {
    let user = require_user(cx).await?;
    if !user.role.can_edit() {
        return Err(forbidden().into());
    }

    let id = *path_param::<OrderId>(cx)?;
    let mut order = Order::filter_by_id(id)
        .first()
        .exec(&mut db(cx))
        .await?
        .ok_or_not_found()?;
    if !order.advance(cx, form.status).await? {
        return Err(bad_request("the order cannot move to that status").into());
    }

    Ok(see_other(href!(page, OrderId(id)).resolve(cx)))
}
