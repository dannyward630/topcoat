use topcoat::{
    Result,
    context::Cx,
    view::{Attributes, View, class, component, element_id, view},
};

/// The horizontal distance between two points of a chart, in the chart's own units.
const STEP: usize = 10;

/// A line chart of `values`, oldest first, with the area under the line shaded.
///
/// The chart stretches to fill its container and draws in the current text color, so
/// size and color it through its parent. The line spans the values from the lowest to
/// the highest. `attrs` are forwarded to the `<svg>`, with extra classes added to its
/// classes.
///
/// ```ignore
/// view! {
///     <div class="h-16 text-sky-500">
///         line_chart(values: &[3, 5, 4, 8])
///     </div>
/// }
/// ```
#[component]
pub async fn line_chart(
    cx: &Cx,
    values: &[i64],
    #[default] mut attrs: Attributes,
) -> Result<impl View> {
    let fill_id = element_id(cx);
    let min = values.iter().copied().min().unwrap_or(0);
    let max = values.iter().copied().max().unwrap_or(0);
    let width = values.len().saturating_sub(1) * STEP;
    let points = values
        .iter()
        .enumerate()
        .map(|(index, &value)| {
            // The line keeps clear of the top edge, so its stroke is not cut off.
            let height = if max == min {
                40
            } else {
                10 + (value - min) * 80 / (max - min)
            };
            format!("{},{}", index * STEP, 100 - height)
        })
        .collect::<Vec<_>>()
        .join(" L");

    Ok(view! {
        <svg
            viewBox=(format!("0 0 {width} 100"))
            preserveAspectRatio="none"
            class=(class!("size-full", attrs.remove("class")))
            aria-hidden="true"
            (attrs)
        >
            <defs>
                <linearGradient id=(&fill_id) x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0" stop-color="currentColor" stop-opacity="0.3"></stop>
                    <stop offset="1" stop-color="currentColor" stop-opacity="0"></stop>
                </linearGradient>
            </defs>
            <path
                d=(format!("M0,100 L{points} L{width},100 Z"))
                fill=(format!("url(#{fill_id})"))
            ></path>
            <path
                d=(format!("M{points}"))
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linejoin="round"
                vector-effect="non-scaling-stroke"
            ></path>
        </svg>
    })
}

/// A bar chart of `values`, oldest first, with bars rising from zero. The newest bar
/// stands out from the others.
///
/// The chart stretches to fill its container and draws in the current text color, so
/// size and color it through its parent. `attrs` are forwarded to the `<svg>`, with
/// extra classes added to its classes.
///
/// ```ignore
/// view! {
///     <div class="h-16 text-emerald-500">
///         bar_chart(values: &[3, 5, 4, 8])
///     </div>
/// }
/// ```
#[component]
pub async fn bar_chart(values: &[i64], #[default] mut attrs: Attributes) -> Result<impl View> {
    let max = values.iter().copied().max().unwrap_or(0).max(1);
    let width = values.len() * STEP;

    Ok(view! {
        <svg
            viewBox=(format!("0 0 {width} 100"))
            preserveAspectRatio="none"
            class=(class!("size-full", attrs.remove("class")))
            aria-hidden="true"
            (attrs)
        >
            for (index, &value) in values.iter().enumerate() {
                let height = value * 90 / max;
                <rect
                    x=((index * STEP + 1).to_string())
                    y=((100 - height).to_string())
                    width=((STEP - 2).to_string())
                    height=(height.to_string())
                    fill="currentColor"
                    class=(if index + 1 == values.len() { "" } else { "opacity-40" })
                ></rect>
            }
        </svg>
    })
}
