//! One order, with its items and customer, and the buttons that move it to its next
//! status.

use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{
        content::Form,
        error::{RouterErrorExt, SeeOther, see_other},
        href, module_param, page, path_param, route,
    },
    view::{View, attributes, view},
};

use super::status_badge;
use crate::{
    components::{
        avatar::{avatar, avatar_fallback},
        button::{ButtonVariant, button},
        card::{card, card_content, card_description, card_header, card_title},
        table::{table, table_body, table_cell, table_footer, table_head, table_header, table_row},
    },
    features::{
        auth::require_user,
        order::{Order, OrderStatus},
    },
};

module_param!(pub order_id: u64, error = not_found);

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let user = require_user(cx).await?;
    let id = *path_param::<OrderId>(cx)?;
    let order = Order::find_with_items(cx, id).await?.ok_or_not_found()?;
    // Viewers can see the order but not change it.
    let next = if user.role.can_edit() {
        order.status.next()
    } else {
        &[]
    };

    Ok(view! {
        let customer = order.customer.get();
        let items = order.items.get();
        let units = items.iter().map(|item| item.quantity).sum::<u32>();

        <header class="flex flex-col gap-4 sm:flex-row sm:items-start">
            <div class="flex flex-col gap-1">
                <div class="flex items-center gap-3">
                    <h1 class="text-2xl font-semibold tracking-tight">
                        "Order #"
                        (order.id)
                    </h1>
                    status_badge(status: order.status)
                </div>
                <p class="text-sm text-muted-foreground">
                    "Placed "
                    <time datetime=(order.placed_at.to_string())>
                        (order.placed_at.strftime("%b %-d, %Y at %H:%M UTC").to_string())
                    </time>
                </p>
            </div>
            if !next.is_empty() {
                <form
                    method="post"
                    action=(href!(advance, OrderId(order.id)))
                    class="flex gap-2 sm:ml-auto"
                >
                    for &status in next {
                        button(
                            variant: if matches!(
                                status,
                                OrderStatus::Cancelled | OrderStatus::Refunded,
                            ) {
                                ButtonVariant::Outline
                            } else {
                                ButtonVariant::Primary
                            },
                            attrs: attributes! { type="submit" name="status" value=(status.slug()) },
                            (status.action())
                        )
                    }
                </form>
            }
        </header>

        <div class="grid grid-cols-1 gap-6 lg:grid-cols-3 lg:items-start">
            card(
                attrs: attributes! { class="lg:col-span-2" },
                card_header(
                    card_title("Items")
                    card_description(
                        (units)
                        if units == 1 {
                            " unit"
                        } else {
                            " units"
                        }
                    )
                )
                table(
                    table_header(
                        table_row(
                            table_head(attrs: attributes! { class="w-full" }, "Product")
                            table_head(
                                attrs: attributes! { class="text-right" },
                                "Quantity"
                            )
                            table_head(
                                attrs: attributes! { class="text-right" },
                                "Price"
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
                                    (item.quantity)
                                )
                                table_cell(
                                    attrs: attributes! { class="text-right text-muted-foreground tabular-nums" },
                                    (item.unit_price)
                                )
                                table_cell(
                                    attrs: attributes! { class="text-right font-medium tabular-nums" },
                                    (item.subtotal())
                                )
                            )
                        }
                    )
                    table_footer(
                        table_row(
                            table_cell(attrs: attributes! { colspan="3" }, "Total")
                            table_cell(
                                attrs: attributes! { class="text-right tabular-nums" },
                                (order.total)
                            )
                        )
                    )
                )
            )

            card(
                card_header(card_title("Customer"))
                card_content(
                    <div class="flex items-center gap-3">
                        avatar(avatar_fallback((customer.initials())))
                        <div class="flex min-w-0 flex-col">
                            <span class="truncate font-medium">(&customer.name)</span>
                            <a
                                href=(format!("mailto:{}", customer.email))
                                class="truncate text-sm text-muted-foreground hover:underline"
                            >
                                (&customer.email)
                            </a>
                        </div>
                    </div>
                    <dl class="mt-6 grid grid-cols-2 gap-4 text-sm">
                        <div class="flex flex-col gap-1">
                            <dt class="text-muted-foreground">"City"</dt>
                            <dd>(&customer.city)</dd>
                        </div>
                        <div class="flex flex-col gap-1">
                            <dt class="text-muted-foreground">"Customer since"</dt>
                            <dd>
                                <time datetime=(customer.joined_at.to_string())>
                                    (customer.joined_at.strftime("%b %Y").to_string())
                                </time>
                            </dd>
                        </div>
                    </dl>
                )
            )
        </div>
    })
}

#[derive(Deserialize)]
pub struct Advance {
    /// The status to move the order to.
    status: OrderStatus,
}

/// Moves the order to the submitted status, then shows the order again.
#[route(POST)]
pub async fn advance(cx: &Cx, Form(form): Form<Advance>) -> Result<SeeOther> {
    let user = require_user(cx).await?;
    user.role.can_edit().then_some(()).ok_or_forbidden()?;

    let id = *path_param::<OrderId>(cx)?;
    let mut order = Order::find(cx, id).await?.ok_or_not_found()?;
    // A page opened before someone else changed the order can offer a status the
    // order can no longer move to. The order then stays as it is, and the page
    // shows its current status.
    order.advance(cx, form.status).await?;

    Ok(see_other(href!(page, OrderId(id)).resolve(cx)))
}
