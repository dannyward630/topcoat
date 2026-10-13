//! The orders placed in the store, with a search, a status filter, and a choice of
//! sort order.

use jiff::Timestamp;
use serde::Serialize;
use topcoat::{
    Result,
    context::Cx,
    icon::{icon, iconify::iconify_icon},
    router::{href, page, query_params},
    runtime::{Event, Signal, shard, signal},
    view::{StaticClass, View, attributes, class, component, element_id, suspense, view},
};

use crate::{
    components::{
        avatar::{AvatarSize, avatar, avatar_fallback},
        badge::{BadgeVariant, badge},
        button::{ButtonSize, ButtonVariant, button},
        input::input,
        pagination::{
            pagination, pagination_content, pagination_ellipsis, pagination_item, pagination_link,
            pagination_next, pagination_previous,
        },
        select::select,
        skeleton::skeleton,
        table::{table, table_body, table_cell, table_head, table_header, table_row},
    },
    features::{
        auth::require_user,
        order::{OrderSearch, OrderSort, OrderStatus},
    },
};

/// How many orders a page of the list shows.
const PER_PAGE: usize = 20;

/// The filters the list starts with.
///
/// The page links point here, so they also work when opened in a new tab. A plain
/// click changes the page in place without changing the URL.
#[query_params(error = redirect("?"))]
#[derive(Serialize)]
pub struct OrdersQuery {
    q: Option<String>,
    status: Option<OrderStatus>,
    sort: Option<OrderSort>,
    page: Option<usize>,
}

impl OrdersQuery {
    /// The query for page `number` of the orders with `status` that match `text`,
    /// sorted by `sort`.
    fn new(text: &str, status: Option<OrderStatus>, sort: OrderSort, number: usize) -> Self {
        Self {
            q: (!text.is_empty()).then(|| text.to_owned()),
            status,
            sort: (sort != OrderSort::default()).then_some(sort),
            page: (number > 1).then_some(number),
        }
    }
}

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    require_user(cx).await?;
    let query = query_params::<OrdersQuery>(cx)?;
    let initial_status = query.status.map_or("", OrderStatus::slug);
    let initial_sort = query.sort.unwrap_or_default().slug();

    // The filters live in the browser. Changing one re-renders only the list.
    let search = signal(cx, || query.q.clone().unwrap_or_default());
    let status = signal(cx, || initial_status.to_owned());
    let sort = signal(cx, || initial_sort.to_owned());
    let page_number = signal(cx, || query.page.unwrap_or(1));
    let search_id = element_id(cx);
    let status_id = element_id(cx);
    let sort_id = element_id(cx);

    Ok(view! {
        <section
            class="overflow-hidden rounded-xl border border-border bg-card \
                text-card-foreground shadow-xs"
        >
            <div class="flex flex-col gap-2 p-3 sm:flex-row sm:items-center">
                <div class="relative sm:max-w-xs sm:flex-1">
                    <label for=(&search_id) class="sr-only">"Search orders"</label>
                    icon(
                        data: iconify_icon!("lucide:search"),
                        attrs: attributes! {
                            class="pointer-events-none absolute top-1/2 left-3 size-4 \
                                -translate-y-1/2 text-muted-foreground"
                        }
                    )
                    input(
                        attrs: attributes! {
                            id=(&search_id)
                            type="search"
                            placeholder="Search customers or order numbers"
                            class="pl-9"
                            :value=$(search.get())
                            @input=$(|e: Event| {
                                search.set(e.target.value);
                                page_number.set(1);
                            })
                        }
                    )
                </div>
                <div class="grid grid-cols-2 gap-2 sm:ml-auto sm:flex">
                    <label for=(&status_id) class="sr-only">"Status"</label>
                    select(
                        attrs: attributes! {
                            id=(&status_id)
                            class="sm:w-40"
                            :value=$(status.get())
                            @change=$(|e: Event| {
                                status.set(e.target.value);
                                page_number.set(1);
                            })
                        },
                        <option value="" selected=(initial_status.is_empty())>
                            "All statuses"
                        </option>
                        for option in OrderStatus::ALL {
                            <option
                                value=(option.slug())
                                selected=(initial_status == option.slug())
                            >
                                (option.label())
                            </option>
                        }
                    )
                    <label for=(&sort_id) class="sr-only">"Sort by"</label>
                    select(
                        attrs: attributes! {
                            id=(&sort_id)
                            class="sm:w-40"
                            :value=$(sort.get())
                            @change=$(|e: Event| {
                                sort.set(e.target.value);
                                page_number.set(1);
                            })
                        },
                        for option in OrderSort::ALL {
                            <option
                                value=(option.slug())
                                selected=(initial_sort == option.slug())
                            >
                                (option.label())
                            </option>
                        }
                    )
                </div>
            </div>

            order_list(
                search: search.clone(),
                status: status.clone(),
                sort: sort.clone(),
                page_number: page_number.clone()
            )
        </section>
    })
}

/// One page of the orders that match the filters, with links to the other pages.
///
/// The orders load after the rest of the list, with a skeleton in their place until
/// they arrive.
#[shard]
async fn order_list(
    cx: &Cx,
    search: Signal<String>,
    status: Signal<String>,
    sort: Signal<String>,
    page_number: Signal<usize>,
) -> Result<impl View> {
    // The browser can request a shard directly, so it checks the user itself.
    require_user(cx).await?;

    let text = search.get();
    // An unknown status or sort order comes from a tampered request. Fall back to
    // every status and the default order then.
    let selected = status.get().parse::<OrderStatus>().ok();
    let sort = sort.get().parse::<OrderSort>().unwrap_or_default();
    let count = OrderSearch {
        status: selected,
        text: &text,
    }
    .count(cx)
    .await?;

    let pages = count.div_ceil(PER_PAGE).max(1);
    let current = page_number.get().clamp(1, pages);
    let first = (current - 1) * PER_PAGE + 1;
    let last = (current * PER_PAGE).min(count);
    let previous = current.saturating_sub(1).max(1);
    let next = (current + 1).min(pages);

    Ok(view! {
        if count == 0 {
            <div class="flex flex-col items-center gap-3 px-6 py-16 text-center">
                <span
                    class="flex size-10 items-center justify-center rounded-full \
                        bg-foreground/5 text-muted-foreground"
                >
                    icon(data: iconify_icon!("lucide:package-search"))
                </span>
                <div class="flex flex-col gap-1">
                    <p class="font-medium">"No orders found"</p>
                    <p class="text-sm text-muted-foreground">
                        "No orders match your search and status."
                    </p>
                </div>
                button(
                    variant: ButtonVariant::Outline,
                    size: ButtonSize::Sm,
                    attrs: attributes! {
                        type="button"
                        @click=$(|_e: Event| {
                            search.set("".to_owned());
                            status.set("".to_owned());
                            page_number.set(1);
                        })
                    },
                    "Clear filters"
                )
            </div>
        } else {
            suspense(
                fallback: view! { order_rows_skeleton(rows: last + 1 - first) },
                order_rows(
                    filter: OrderSearch { status: selected, text: &text },
                    sort: sort,
                    number: current
                )
            )

            <div class="flex flex-col items-center gap-3 px-6 py-3 sm:flex-row">
                <p class="shrink-0 text-sm text-muted-foreground tabular-nums">
                    "Showing "
                    <span class="font-medium text-foreground">
                        (first)
                        "-"
                        (last)
                    </span>
                    " of "
                    <span class="font-medium text-foreground">(count)</span>
                    " orders"
                </p>
                pagination(
                    attrs: attributes! { class="sm:justify-end" },
                    pagination_content(
                        pagination_item(
                            pagination_previous(
                                attrs: attributes! {
                                    href=(href!(page).query(
                                        &OrdersQuery::new(&text, selected, sort, previous),
                                    ))
                                    aria-disabled=((current == 1).then_some("true"))
                                    class=(class!("pointer-events-none opacity-50" if current == 1))
                                    @click=$(|e: Event| {
                                        e.prevent_default();
                                        page_number.set(previous);
                                    })
                                }
                            )
                        )
                        // Narrow screens only have room for the previous and next
                        // links.
                        for number in page_window(current, pages) {
                            pagination_item(
                                attrs: attributes! { class="hidden sm:block" },
                                if let Some(number) = number {
                                    pagination_link(
                                        active: number == current,
                                        attrs: attributes! {
                                            href=(href!(page).query(
                                                &OrdersQuery::new(&text, selected, sort, number),
                                            ))
                                            @click=$(|e: Event| {
                                                e.prevent_default();
                                                page_number.set(number);
                                            })
                                        },
                                        (number)
                                    )
                                } else {
                                    pagination_ellipsis()
                                }
                            )
                        }
                        pagination_item(
                            pagination_next(
                                attrs: attributes! {
                                    href=(href!(page).query(
                                        &OrdersQuery::new(&text, selected, sort, next),
                                    ))
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
        }
    })
}

/// The page numbers to link to, with `None` where an ellipsis stands for the pages
/// left out.
///
/// It keeps the first and last pages and the pages around `current`. An ellipsis
/// never hides a single page.
fn page_window(current: usize, pages: usize) -> Vec<Option<usize>> {
    let shown = |number: usize| number == 1 || number == pages || number.abs_diff(current) <= 1;
    let mut window = Vec::new();
    for number in 1..=pages {
        if shown(number) || (shown(number - 1) && shown(number + 1)) {
            window.push(Some(number));
        } else if window.last() != Some(&None) {
            window.push(None);
        }
    }
    window
}

/// Classes for the columns that narrow screens leave out.
const ITEMS_COLUMN: StaticClass = class!("hidden lg:table-cell");
const PLACED_COLUMN: StaticClass = class!("hidden md:table-cell");
const STATUS_COLUMN: StaticClass = class!("hidden sm:table-cell");

/// The table of orders on page `number` of the list, sorted by `sort`.
#[component]
async fn order_rows(
    cx: &Cx,
    filter: OrderSearch<'_>,
    sort: OrderSort,
    number: usize,
) -> Result<impl View> {
    let orders = filter.page(cx, sort, number, PER_PAGE).await?;

    Ok(view! {
        table(
            order_columns()
            table_body(
                for order in &orders {
                    let customer = order.customer.get();
                    let units = order
                        .items
                        .get()
                        .iter()
                        .map(|item| item.quantity)
                        .sum::<u32>();
                    table_row(
                        table_cell(
                            attrs: attributes! { class="font-medium tabular-nums" },
                            "#"
                            (order.id)
                        )
                        table_cell(
                            // `max-w-0` lets long names and addresses shorten with an
                            // ellipsis instead of widening the table.
                            attrs: attributes! { class="max-w-0" },
                            <div class="flex items-center gap-3">
                                avatar(
                                    size: AvatarSize::Sm,
                                    avatar_fallback((customer.initials()))
                                )
                                <div class="flex min-w-0 flex-col">
                                    <span class="truncate font-medium">(&customer.name)</span>
                                    <span class="truncate text-xs text-muted-foreground">
                                        (&customer.email)
                                    </span>
                                </div>
                            </div>
                            // Narrow screens leave out the status column, so the status
                            // moves here.
                            <div class="mt-2 sm:hidden">
                                status_badge(status: order.status)
                            </div>
                        )
                        table_cell(
                            attrs: attributes! { class=(STATUS_COLUMN) },
                            status_badge(status: order.status)
                        )
                        table_cell(
                            attrs: attributes! {
                                class=(class!("text-muted-foreground tabular-nums", ITEMS_COLUMN))
                            },
                            (units)
                            if units == 1 {
                                " item"
                            } else {
                                " items"
                            }
                        )
                        table_cell(
                            attrs: attributes! { class=(class!("text-muted-foreground", PLACED_COLUMN)) },
                            <time
                                datetime=(order.placed_at.to_string())
                                title=(order
                                    .placed_at
                                    .strftime("%b %-d, %Y at %H:%M UTC")
                                    .to_string())
                            >
                                (time_ago(order.placed_at))
                            </time>
                        )
                        table_cell(
                            attrs: attributes! { class="text-right font-medium tabular-nums" },
                            (order.total)
                        )
                    )
                }
            )
        )
    })
}

/// A placeholder for [`order_rows`] with `rows` rows of the same height.
#[component]
async fn order_rows_skeleton(rows: usize) -> Result<impl View> {
    Ok(view! {
        table(
            attrs: attributes! { aria-busy="true" },
            order_columns()
            table_body(
                for _ in 0..rows {
                    table_row(
                        table_cell(skeleton(attrs: attributes! { class="h-4 w-10" }))
                        table_cell(
                            attrs: attributes! { class="max-w-0" },
                            <div class="flex items-center gap-3">
                                avatar(
                                    size: AvatarSize::Sm,
                                    skeleton(attrs: attributes! { class="size-full" })
                                )
                                <div class="flex min-w-0 flex-col">
                                    <div class="flex h-5 items-center">
                                        skeleton(
                                            attrs: attributes! { class="h-3.5 w-28 max-w-full" }
                                        )
                                    </div>
                                    <div class="flex h-4 items-center">
                                        skeleton(
                                            attrs: attributes! { class="h-3 w-44 max-w-full" }
                                        )
                                    </div>
                                </div>
                            </div>
                            <div class="mt-2 sm:hidden">
                                skeleton(attrs: attributes! { class="h-5.5 w-20" })
                            </div>
                        )
                        table_cell(
                            attrs: attributes! { class=(STATUS_COLUMN) },
                            skeleton(attrs: attributes! { class="h-5.5 w-20" })
                        )
                        table_cell(
                            attrs: attributes! { class=(ITEMS_COLUMN) },
                            skeleton(attrs: attributes! { class="h-4 w-14" })
                        )
                        table_cell(
                            attrs: attributes! { class=(PLACED_COLUMN) },
                            skeleton(attrs: attributes! { class="h-4 w-20" })
                        )
                        table_cell(
                            skeleton(attrs: attributes! { class="ml-auto h-4 w-16" })
                        )
                    )
                }
            )
        )
    })
}

/// The header row naming the columns of the order table.
#[component]
async fn order_columns() -> Result<impl View> {
    Ok(view! {
        table_header(
            table_row(
                table_head("Order")
                // The customer column takes the space the other columns leave.
                table_head(attrs: attributes! { class="w-full" }, "Customer")
                table_head(attrs: attributes! { class=(STATUS_COLUMN) }, "Status")
                table_head(attrs: attributes! { class=(ITEMS_COLUMN) }, "Items")
                table_head(attrs: attributes! { class=(PLACED_COLUMN) }, "Placed")
                table_head(attrs: attributes! { class="text-right" }, "Total")
            )
        )
    })
}

/// A badge naming an order's status, with a dot colored by how far along it is.
#[component]
async fn status_badge(status: OrderStatus) -> Result<impl View> {
    let dot: StaticClass = match status {
        OrderStatus::Pending => class!("bg-amber-500"),
        OrderStatus::Paid => class!("bg-sky-500"),
        OrderStatus::Shipped => class!("bg-violet-500"),
        OrderStatus::Delivered => class!("bg-emerald-500"),
        OrderStatus::Cancelled => class!("bg-muted-foreground/60"),
        OrderStatus::Refunded => class!("bg-rose-500"),
    };

    Ok(view! {
        badge(
            variant: BadgeVariant::Outline,
            <span class=(class!("size-1.5 rounded-full", dot))></span>
            (status.label())
        )
    })
}

/// Describes how long ago `timestamp` was, like `3 hours ago`. Anything older than a
/// week shows its date instead, like `Oct 12, 2026`.
fn time_ago(timestamp: Timestamp) -> String {
    let minutes = Timestamp::now().duration_since(timestamp).as_mins();
    let (amount, unit) = if minutes < 60 {
        (minutes, "minute")
    } else if minutes < 24 * 60 {
        (minutes / 60, "hour")
    } else if minutes < 7 * 24 * 60 {
        (minutes / (24 * 60), "day")
    } else {
        return timestamp.strftime("%b %-d, %Y").to_string();
    };

    match amount {
        ..1 => "just now".to_owned(),
        1 => format!("1 {unit} ago"),
        _ => format!("{amount} {unit}s ago"),
    }
}
