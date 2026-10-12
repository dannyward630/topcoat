//! The orders, with a status filter, a search that updates as you type, and a CSV
//! export.

pub mod order_id;

use std::fmt::Write;

use serde::Serialize;
use topcoat::{
    Result,
    context::Cx,
    icon::{icon, iconify::iconify_icon},
    router::{header, href, page, query_params, route},
    runtime::{Event, Signal, link, shard, signal},
    view::{View, attributes, class, component, element_id, view},
};

use self::order_id::OrderId;
use super::display_date;
use crate::{
    app::login::require_user,
    components::{
        badge::{BadgeVariant, badge},
        button::{ButtonSize, ButtonVariant, button_variants},
        card::{card, card_content, card_footer},
        input::input,
        pagination::{
            pagination, pagination_content, pagination_item, pagination_next, pagination_previous,
        },
        table::{table, table_body, table_cell, table_head, table_header, table_row},
        tabs::{tabs_list, tabs_trigger},
    },
    features::order::{OrderSearch, OrderStatus},
};

/// How many orders a page of the list shows.
const PER_PAGE: usize = 20;

/// The list's filters, so a link can open it already filtered.
#[query_params(error = redirect("?"))]
#[derive(Serialize)]
pub struct OrdersQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<OrderStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    q: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<usize>,
}

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    require_user(cx).await?;
    let query = query_params::<OrdersQuery>(cx)?;

    // The filters live in the browser. Changing one re-renders only the table below.
    let status = signal(cx, || query.status.map_or("", OrderStatus::slug).to_owned());
    let search = signal(cx, || query.q.clone().unwrap_or_default());
    let page_number = signal(cx, || query.page.unwrap_or(1));
    let search_id = element_id(cx);

    Ok(view! {
        <header class="flex flex-col gap-1">
            <h1 class="text-2xl font-semibold tracking-tight">"Orders"</h1>
            <p class="text-muted-foreground">
                "Every order placed in the store, newest first."
            </p>
        </header>

        <div class="flex flex-wrap items-center justify-between gap-3">
            tabs_list(
                attrs: attributes! { class="max-w-full overflow-x-auto" },
                // Each tab links to the filtered list, so it still works when opened in
                // a new tab. A plain click switches the filter in place instead.
                tabs_trigger(
                    active: $(status.get().is_empty()),
                    attrs: attributes! {
                        href=(href!(page))
                        @click=$(|e: Event| {
                            e.prevent_default();
                            status.set("".to_owned());
                            page_number.set(1);
                        })
                    },
                    "All"
                )
                for option in OrderStatus::ALL {
                    let slug = option.slug();
                    tabs_trigger(
                        active: $(status.get() == slug),
                        attrs: attributes! {
                            href=(href!(page).query(
                                &OrdersQuery { status: Some(option), q: None, page: None },
                            ))
                            @click=$(|e: Event| {
                                e.prevent_default();
                                status.set(slug.to_owned());
                                page_number.set(1);
                            })
                        },
                        (option.label())
                    )
                }
            )
            <label for=(&search_id) class="sr-only">"Search orders"</label>
            input(
                attrs: attributes! {
                    id=(&search_id)
                    type="search"
                    placeholder="Search by customer or order number"
                    class="sm:w-72"
                    :value=$(search.get())
                    @input=$(|e: Event| {
                        search.set(e.target.value);
                        page_number.set(1);
                    })
                }
            )
        </div>

        order_table(
            status: $(status.get()),
            search: $(search.get()),
            page_number: page_number.clone()
        )
    })
}

/// One page of the orders that match the filters, with links to the other pages.
#[shard]
async fn order_table(
    cx: &Cx,
    status: String,
    search: String,
    page_number: Signal<usize>,
) -> Result<impl View> {
    // The browser can request a shard directly, so it checks the user itself.
    require_user(cx).await?;

    // An unknown status comes from a tampered request. Show every order then.
    let status = status.parse::<OrderStatus>().ok();
    let filter = OrderSearch {
        status,
        text: &search,
    };

    let count = filter.count(cx).await?;
    let pages = count.div_ceil(PER_PAGE).max(1);
    let current = page_number.get().clamp(1, pages);
    let orders = filter.page(cx, current, PER_PAGE).await?;

    let first = (current - 1) * PER_PAGE + 1;
    let last = first + orders.len() - 1;
    let filters = |number: usize| OrdersQuery {
        status,
        q: (!search.is_empty()).then(|| search.clone()),
        page: Some(number),
    };
    let previous = current.saturating_sub(1).max(1);
    let next = (current + 1).min(pages);
    let previous_query = filters(previous);
    let next_query = filters(next);
    let export_query = OrdersQuery {
        page: None,
        ..filters(1)
    };

    Ok(view! {
        card(
            card_content(
                if orders.is_empty() {
                    <p class="py-10 text-center text-sm text-muted-foreground">
                        "No orders match."
                    </p>
                } else {
                    table(
                        table_header(
                            table_row(
                                table_head("Order")
                                table_head("Customer")
                                table_head("Status")
                                table_head("Placed")
                                table_head(
                                    attrs: attributes! { class="text-right" },
                                    "Total"
                                )
                            )
                        )
                        table_body(
                            for order in &orders {
                                let customer = order.customer.get();
                                table_row(
                                    table_cell(
                                        link(
                                            href: href!(order_id::page, OrderId(order.id)),
                                            attrs: attributes! { class="font-medium hover:underline" },
                                            "#"
                                            (order.id)
                                        )
                                    )
                                    table_cell(
                                        <div class="flex flex-col">
                                            <span>(&customer.name)</span>
                                            <span class="text-xs text-muted-foreground">
                                                (&customer.email)
                                            </span>
                                        </div>
                                    )
                                    table_cell(status_badge(status: order.status))
                                    table_cell((display_date(order.placed_at)))
                                    table_cell(
                                        attrs: attributes! { class="text-right tabular-nums" },
                                        (order.total)
                                    )
                                )
                            }
                        )
                    )
                }
            )
            card_footer(
                attrs: attributes! { class="flex-wrap justify-between" },
                <p class="text-sm text-muted-foreground">
                    if count > 0 {
                        "Showing "
                        (first)
                        " to "
                        (last)
                        " of "
                        (count)
                    }
                </p>
                <div class="flex flex-wrap items-center gap-2">
                    <a
                        href=(href!(export).query(&export_query))
                        download=""
                        class=(button_variants(ButtonVariant::Outline, ButtonSize::Sm))
                    >
                        icon(data: iconify_icon!("lucide:download"))
                        "Export CSV"
                    </a>
                    // The pagination fills its container, so this wrapper sizes it to fit.
                    <div>
                        pagination(
                            pagination_content(
                                pagination_item(
                                    pagination_previous(
                                        attrs: attributes! {
                                            href=(href!(page).query(&previous_query))
                                            aria-disabled=((current == 1).then_some("true"))
                                            class=(class!("pointer-events-none opacity-50" if current == 1))
                                            @click=$(|e: Event| {
                                                e.prevent_default();
                                                page_number.set(previous);
                                            })
                                        }
                                    )
                                )
                                pagination_item(
                                    <span
                                        class="px-2 text-sm text-muted-foreground tabular-nums"
                                    >
                                        "Page "
                                        (current)
                                        " of "
                                        (pages)
                                    </span>
                                )
                                pagination_item(
                                    pagination_next(
                                        attrs: attributes! {
                                            href=(href!(page).query(&next_query))
                                            aria-disabled=((current == pages).then_some("true"))
                                            class=(class!("pointer-events-none opacity-50" if current == pages))
                                            @click=$(|e: Event| {
                                                e.prevent_default();
                                                page_number.set(next);
                                            })
                                        }
                                    )
                                )
                            )
                        )
                    </div>
                </div>
            )
        )
    })
}

/// Downloads the orders that match the filters as a CSV file.
#[route(GET "./export")]
async fn export(cx: &Cx) -> Result<([(header::HeaderName, &'static str); 2], String)> {
    require_user(cx).await?;
    let query = query_params::<OrdersQuery>(cx)?;
    let orders = OrderSearch {
        status: query.status,
        text: query.q.as_deref().unwrap_or_default(),
    }
    .all(cx)
    .await?;

    let mut csv = String::from("order,placed_at,customer,email,status,total\n");
    for order in &orders {
        let customer = order.customer.get();
        writeln!(
            csv,
            "{},{},{},{},{},{}",
            order.id,
            order.placed_at,
            csv_field(&customer.name),
            csv_field(&customer.email),
            order.status.slug(),
            order.total.decimal(),
        )
        .expect("writing to a string does not fail");
    }

    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"orders.csv\"",
            ),
        ],
        csv,
    ))
}

/// Quotes a CSV field if it contains a character that would end it early.
fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// A badge naming an order's status, colored by how far along the order is.
#[component]
pub async fn status_badge(status: OrderStatus) -> Result<impl View> {
    let variant = match status {
        OrderStatus::Pending => BadgeVariant::Outline,
        OrderStatus::Paid | OrderStatus::Shipped => BadgeVariant::Secondary,
        OrderStatus::Delivered => BadgeVariant::Primary,
        OrderStatus::Cancelled | OrderStatus::Refunded => BadgeVariant::Destructive,
    };

    Ok(view! { badge(variant: variant, (status.label())) })
}
