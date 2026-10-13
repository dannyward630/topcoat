//! The dashboard, the first page after signing in, with the store's figures for
//! today.
//!
//! The leading underscore keeps the page at the URL of the group it is in.

use std::time::Duration;

use tokio::time::{MissedTickBehavior, interval};
use topcoat::{
    Result,
    context::Cx,
    router::page,
    runtime::{connected, shard},
    view::{EmitToken, StaticClass, View, attributes, class, component, emit, live, view},
};

use crate::{
    components::{
        card::card,
        chart::{bar_chart, line_chart},
    },
    features::{
        auth::require_user,
        stats::{SalesStats, VisitorStats},
    },
};

/// How often the dashboard checks whether its figures changed.
const TICK: Duration = Duration::from_secs(1);

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let user = require_user(cx).await?;
    let first_name = user.name.split_whitespace().next().unwrap_or(&user.name);

    Ok(view! {
        <header class="flex flex-col gap-1">
            <h1 class="text-2xl font-semibold tracking-tight">
                "Welcome back, "
                (first_name)
            </h1>
            <p class="text-muted-foreground">
                "Here is what is happening in the store."
            </p>
        </header>

        todays_stats()
    })
}

/// The store's figures for today, which keep updating while the page is open.
#[shard]
async fn todays_stats(cx: &Cx) -> Result<impl View> {
    require_user(cx).await?;

    Ok(view! {
        <section class="flex flex-col gap-3">
            <div class="flex items-center gap-2">
                <h2 class="font-semibold">"Today"</h2>
                <span class="relative flex size-2" aria-hidden="true">
                    <span
                        class="absolute inline-flex size-full animate-ping rounded-full bg-emerald-500 opacity-75"
                    ></span>
                    <span
                        class="relative inline-flex size-2 rounded-full bg-emerald-500"
                    ></span>
                </span>
                <span class="text-sm text-muted-foreground">"Live"</span>
            </div>
            <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
                stat_card(
                    label: "Revenue",
                    accent: class!("text-emerald-500"),
                    figure: revenue
                )
                stat_card(
                    label: "Orders",
                    accent: class!("text-violet-500"),
                    figure: orders
                )
                stat_card(
                    label: "Average order",
                    accent: class!("text-amber-500"),
                    figure: average_order
                )
                stat_card(
                    label: "Visitors right now",
                    accent: class!("text-sky-500"),
                    figure: visitors
                )
            </div>
        </section>
    })
}

/// A figure on the dashboard, as its card shows it.
#[derive(PartialEq, Eq)]
struct Figure {
    /// The figure itself, ready to show.
    value: String,
    /// How much the figure changed since the same time yesterday, in tenths of a
    /// percent.
    change: Option<i64>,
    /// The figure's recent history.
    chart: Chart,
}

/// A chart of a figure's recent history, oldest first.
#[derive(PartialEq, Eq)]
enum Chart {
    Bars(Vec<i64>),
    Line(Vec<i64>),
}

/// Today's revenue, with the revenue of each recent minute.
fn revenue() -> Figure {
    let sales = SalesStats::now();
    Figure {
        value: sales.today.revenue.to_string(),
        change: change(sales.today.revenue.0, sales.yesterday.revenue.0),
        chart: Chart::Bars(
            sales
                .by_minute
                .iter()
                .map(|sales| sales.revenue.0)
                .collect(),
        ),
    }
}

/// Today's orders, with the orders of each recent minute.
fn orders() -> Figure {
    let sales = SalesStats::now();
    Figure {
        value: grouped(sales.today.orders),
        change: change(sales.today.orders, sales.yesterday.orders),
        chart: Chart::Bars(sales.by_minute.iter().map(|sales| sales.orders).collect()),
    }
}

/// The average of today's orders, with the average of each recent minute.
fn average_order() -> Figure {
    let sales = SalesStats::now();
    let today = sales.today.average_order();
    Figure {
        value: today.to_string(),
        change: change(today.0, sales.yesterday.average_order().0),
        // A minute without orders has no average, so it leaves no point on the line.
        chart: Chart::Line(
            sales
                .by_minute
                .iter()
                .filter(|sales| sales.orders > 0)
                .map(|sales| sales.average_order().0)
                .collect(),
        ),
    }
}

/// The shoppers on the storefront, with the shoppers of each recent second.
fn visitors() -> Figure {
    let visitors = VisitorStats::now();
    Figure {
        value: grouped(visitors.current),
        change: change(visitors.current, visitors.yesterday),
        chart: Chart::Line(visitors.by_second),
    }
}

/// A card showing the figure `figure` returns, under its `label`.
///
/// The card checks the figure every [`TICK`] and updates in the browser when it
/// changed. Its chart is drawn in the `accent` color.
#[component]
async fn stat_card(
    cx: &Cx,
    label: &str,
    accent: StaticClass,
    figure: fn() -> Figure,
) -> Result<impl View> {
    Ok(view! {
        card(
            attrs: attributes! { class="gap-0 overflow-hidden pb-0" },
            <p class="px-6 text-sm text-muted-foreground">(label)</p>
            (live! {
                let mut ticks = interval(TICK);
                ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
                let mut shown = None;
                loop {
                    ticks.tick().await;
                    let current = figure();
                    if shown.as_ref() != Some(&current) {
                        emit! { figure_details(figure: &current, accent: accent.clone()) }?;
                        shown = Some(current);
                    }
                    if !connected(cx) {
                        break Ok(EmitToken);
                    }
                }
            })
        )
    })
}

/// The value, change, and chart of `figure`, with the chart in the `accent` color.
#[component]
async fn figure_details(figure: &Figure, accent: StaticClass) -> Result<impl View> {
    Ok(view! {
        <p class="mt-1 px-6 text-3xl font-semibold tracking-tight tabular-nums">
            (&figure.value)
        </p>
        if let Some(change) = figure.change {
            <p
                class="mt-2 flex items-center gap-1.5 px-6 text-xs text-muted-foreground"
            >
                if change >= 0 {
                    <span class="font-medium text-emerald-600">
                        "+"
                        (percent(change))
                    </span>
                } else {
                    <span class="font-medium text-rose-600">
                        "-"
                        (percent(change))
                    </span>
                }
                "vs. yesterday"
            </p>
        }
        <div class=(class!("mt-4 h-16", accent))>
            match &figure.chart {
                Chart::Bars(values) => bar_chart(values: values),
                Chart::Line(values) => line_chart(values: values),
            }
        </div>
    })
}

/// How much `now` differs from `before`, in tenths of a percent of `before`.
fn change(now: i64, before: i64) -> Option<i64> {
    (before != 0).then(|| (now - before) * 1_000 / before)
}

/// Shows a change in tenths of a percent as a percentage, like `4.2%`.
fn percent(change: i64) -> String {
    let change = change.unsigned_abs();
    format!("{}.{}%", change / 10, change % 10)
}

/// Shows `number` with its digits grouped in threes, like `12,345`.
fn grouped(number: i64) -> String {
    let digits = number.unsigned_abs().to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if number < 0 {
        grouped.push('-');
    }
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}
