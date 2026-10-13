use topcoat::{
    Result,
    view::{Attributes, Child, View, class, component, view},
};

/// A table inside a horizontally scrollable container.
///
/// The first and last columns have extra padding on their outer side, so their content
/// lines up with the edges of a surrounding panel, such as a card. A line separates the
/// table from the elements before and after it. When the table is the first or last
/// child of its parent, it has no line on that side, so it does not double the
/// parent's own border.
///
/// Pass table sections as children. `attrs` are forwarded to the `<table>`, with extra
/// classes added to its classes.
///
/// ```ignore
/// view! {
///     table(
///         table_header(
///             table_row(
///                 table_head("Environment")
///                 table_head("Status")
///             )
///         )
///         table_body(
///             table_row(
///                 table_cell("production")
///                 table_cell("Live")
///             )
///         )
///     )
/// }
/// ```
#[component]
pub async fn table(
    #[default] mut attrs: Attributes,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <div data-slot="table" class="w-full overflow-x-auto">
            // The lines go on the `<table>`, which draws them around its rows and
            // leaves the caption outside. They depend on the container's place among
            // its siblings.
            <table
                class=(class!(
                    "w-full caption-bottom border-collapse border-border text-sm",
                    "[:not(:first-child)>&]:border-t [:not(:last-child)>&]:border-b",
                    attrs.remove("class"),
                ))
                (attrs)
            >
                (child)
            </table>
        </div>
    })
}

/// The heading section of a [`table`], holding the row of column headers.
#[component]
pub async fn table_header(
    #[default] mut attrs: Attributes,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <thead
            class=(class!("bg-foreground/5 [&_tr]:border-b", attrs.remove("class")))
            (attrs)
        >
            (child)
        </thead>
    })
}

/// The body of a table, containing data rows.
#[component]
pub async fn table_body(
    #[default] mut attrs: Attributes,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <tbody
            class=(class!("[&_tr:last-child]:border-0", attrs.remove("class")))
            (attrs)
        >
            (child)
        </tbody>
    })
}

/// The closing section of a [`table`], for totals and other summaries.
#[component]
pub async fn table_footer(
    #[default] mut attrs: Attributes,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <tfoot
            class=(class!(
                "border-t border-border bg-foreground/5 font-medium [&>tr]:last:border-b-0",
                attrs.remove("class"),
            ))
            (attrs)
        >
            (child)
        </tfoot>
    })
}

/// A table row with a separator and hover styling.
#[component]
pub async fn table_row(
    #[default] mut attrs: Attributes,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <tr
            class=(class!(
                "border-b border-border transition-colors hover:bg-foreground/5",
                attrs.remove("class"),
            ))
            (attrs)
        >
            (child)
        </tr>
    })
}

/// A column header in a [`table_header`]'s row.
#[component]
pub async fn table_head(
    #[default] mut attrs: Attributes,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <th
            class=(class!(
                "h-10 px-3 text-left align-middle font-medium whitespace-nowrap",
                "text-muted-foreground first:pl-6 last:pr-6",
                attrs.remove("class"),
            ))
            (attrs)
        >
            (child)
        </th>
    })
}

/// One cell of a [`table_row`].
#[component]
pub async fn table_cell(
    #[default] mut attrs: Attributes,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <td
            class=(class!(
                "p-3 align-middle whitespace-nowrap first:pl-6 last:pr-6",
                attrs.remove("class"),
            ))
            (attrs)
        >
            (child)
        </td>
    })
}

/// A caption describing the table.
#[component]
pub async fn table_caption(
    #[default] mut attrs: Attributes,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <caption
            class=(class!("mt-4 text-sm text-muted-foreground", attrs.remove("class")))
            (attrs)
        >
            (child)
        </caption>
    })
}
