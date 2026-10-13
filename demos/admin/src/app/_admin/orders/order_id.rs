//! One order, with its items and customer, and the buttons that move it to its next
//! status.

use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{
        Body, Next,
        content::Form,
        error::{RouterErrorExt, SeeOther, see_other},
        href, layer, module_param, page, path_param,
        response::Response,
        route,
    },
    runtime::{Event, signal},
    view::{View, attributes, component, element_id, view},
};

use super::status_badge;
use crate::{
    app::_admin::Crumb,
    components::{
        avatar::{avatar, avatar_fallback},
        button::{ButtonVariant, button},
        card::{card, card_content, card_description, card_header, card_title},
        dialog::{
            dialog, dialog_content, dialog_description, dialog_footer, dialog_header, dialog_title,
        },
        table::{table, table_body, table_cell, table_footer, table_head, table_header, table_row},
    },
    features::{
        auth::require_user,
        order::{Order, OrderStatus},
    },
};

module_param!(pub order_id: u64, error = not_found);

/// Names the order in the layout's breadcrumb.
#[layer]
async fn crumb(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let id = path_param::<OrderId>(cx)?;
    let cx = cx.with(Crumb(format!("#{id}")));
    next.run(&cx, body).await
}

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
                status_actions(id: order.id, next: next)
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

/// The buttons that move the order with `id` to one of the `next` statuses.
///
/// A status that stops the order, such as cancelled, asks for confirmation first.
#[component]
async fn status_actions(cx: &Cx, id: u64, next: &'static [OrderStatus]) -> Result<impl View> {
    let confirming = signal(cx, || false);
    let form_id = element_id(cx);
    let dialog_id = element_id(cx);
    let title_id = element_id(cx);
    let reversal = next.iter().copied().find(|status| status.is_reversal());

    Ok(view! {
        <form
            id=(&form_id)
            method="post"
            action=(href!(advance, OrderId(id)))
            class="flex gap-2 sm:ml-auto"
        >
            for &status in next {
                if status.is_reversal() {
                    button(
                        variant: ButtonVariant::Outline,
                        attrs: attributes! {
                            type="button"
                            aria-haspopup="dialog"
                            aria-controls=(&dialog_id)
                            @click=$(|_e: Event| confirming.set(true))
                        },
                        (status.action())
                    )
                } else {
                    button(
                        attrs: attributes! { type="submit" name="status" value=(status.slug()) },
                        (status.action())
                    )
                }
            }
        </form>

        if let Some(status) = reversal {
            dialog(
                open: $(confirming.get()),
                attrs: attributes! {
                    id=(&dialog_id)
                    aria-labelledby=(&title_id)
                    @keydown=$(|e: Event| {
                        if e.key == "Escape" {
                            confirming.set(false);
                        }
                    })
                    @click=$(|e: Event| {
                        // A click on the backdrop targets the dialog itself.
                        if e.target.id == dialog_id {
                            confirming.set(false);
                        }
                    })
                },
                dialog_content(
                    dialog_header(
                        dialog_title(
                            attrs: attributes! { id=(&title_id) },
                            (status.action())
                            "?"
                        )
                        dialog_description(
                            "Order #"
                            (id)
                            " cannot move to another status after this."
                        )
                    )
                    dialog_footer(
                        button(
                            variant: ButtonVariant::Outline,
                            attrs: attributes! {
                                type="button"
                                @click=$(|_e: Event| confirming.set(false))
                            },
                            "Keep order"
                        )
                        button(
                            variant: ButtonVariant::Destructive,
                            attrs: attributes! {
                                type="submit"
                                form=(&form_id)
                                name="status"
                                value=(status.slug())
                            },
                            (status.action())
                        )
                    )
                )
            )
        }
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
