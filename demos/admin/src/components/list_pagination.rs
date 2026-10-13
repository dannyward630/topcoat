use topcoat::{
    Result,
    runtime::{Event, Signal},
    view::{View, attributes, class, component, view},
};

use super::pagination::{
    pagination, pagination_content, pagination_ellipsis, pagination_item, pagination_link,
    pagination_next, pagination_previous,
};

/// One page of a list that is split into pages of the same size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListPage {
    /// How many items the whole list has.
    pub count: usize,
    /// The page's number, from 1 to [`pages`](Self::pages).
    pub number: usize,
    per_page: usize,
}

impl ListPage {
    /// Page `number` of a list of `count` items, with `per_page` items on each page.
    ///
    /// A number before the first page or after the last one gives the nearest page
    /// instead. An empty list still has one page.
    ///
    /// # Panics
    ///
    /// Panics if `per_page` is 0.
    pub fn new(count: usize, per_page: usize, number: usize) -> Self {
        assert!(per_page > 0, "a page holds at least one item");
        let pages = count.div_ceil(per_page).max(1);
        Self {
            count,
            number: number.clamp(1, pages),
            per_page,
        }
    }

    /// How many pages the list has.
    pub fn pages(self) -> usize {
        self.count.div_ceil(self.per_page).max(1)
    }

    /// Where the page's first item is in the list, counting from 1.
    pub fn first(self) -> usize {
        (self.number - 1) * self.per_page + 1
    }

    /// Where the page's last item is in the list, counting from 1.
    pub fn last(self) -> usize {
        (self.number * self.per_page).min(self.count)
    }

    /// How many items the page shows.
    pub fn items(self) -> usize {
        self.last() + 1 - self.first()
    }

    /// The page numbers to link to, with `None` where an ellipsis stands for the
    /// pages left out.
    ///
    /// It keeps the first and last pages and the pages around this one. An ellipsis
    /// never hides a single page.
    fn window(self) -> Vec<Option<usize>> {
        let pages = self.pages();
        let shown =
            |number: usize| number == 1 || number == pages || number.abs_diff(self.number) <= 1;
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
}

/// The footer of a list split into pages: which items `page` shows, and links to the
/// other pages.
///
/// `noun` names the items, like `orders`. `href` gives the URL of a page from its
/// number, so a link also works when opened in a new tab. A plain click sets
/// `number` instead, so the list can change in place.
#[component]
pub async fn list_pagination(
    page: ListPage,
    noun: &str,
    href: impl Fn(usize) -> String + Send + Sync,
    number: &Signal<usize>,
) -> Result<impl View> {
    let current = page.number;
    let pages = page.pages();
    let previous = current.saturating_sub(1).max(1);
    let next = (current + 1).min(pages);

    Ok(view! {
        <div class="flex flex-col items-center gap-3 px-6 py-3 sm:flex-row">
            <p class="shrink-0 text-sm text-muted-foreground tabular-nums">
                "Showing "
                <span class="font-medium text-foreground">
                    (page.first())
                    "-"
                    (page.last())
                </span>
                " of "
                <span class="font-medium text-foreground">(page.count)</span>
                " "
                (noun)
            </p>
            // Pulled right by the "Next" link's padding, so its chevron lines up with
            // the last column of a table above.
            pagination(
                attrs: attributes! { class="sm:-mr-3 sm:justify-end" },
                pagination_content(
                    pagination_item(
                        pagination_previous(
                            attrs: attributes! {
                                href=(href(previous))
                                aria-disabled=((current == 1).then_some("true"))
                                class=(class!("pointer-events-none opacity-50" if current == 1))
                                @click=$(|e: Event| {
                                    e.prevent_default();
                                    number.set(previous);
                                })
                            }
                        )
                    )
                    // Narrow screens only have room for the previous and next links.
                    for shown in page.window() {
                        pagination_item(
                            attrs: attributes! { class="hidden sm:block" },
                            if let Some(shown) = shown {
                                pagination_link(
                                    active: shown == current,
                                    attrs: attributes! {
                                        href=(href(shown))
                                        @click=$(|e: Event| {
                                            e.prevent_default();
                                            number.set(shown);
                                        })
                                    },
                                    (shown)
                                )
                            } else {
                                pagination_ellipsis()
                            }
                        )
                    }
                    pagination_item(
                        pagination_next(
                            attrs: attributes! {
                                href=(href(next))
                                aria-disabled=((current == pages).then_some("true"))
                                class=(class!("pointer-events-none opacity-50" if current == pages))
                                @click=$(|e: Event| {
                                    e.prevent_default();
                                    number.set(next);
                                })
                            }
                        )
                    )
                )
            )
        </div>
    })
}

#[cfg(test)]
mod tests {
    use super::ListPage;

    #[test]
    fn a_page_shows_its_share_of_the_items() {
        let page = ListPage::new(45, 20, 2);
        assert_eq!((page.first(), page.last(), page.items()), (21, 40, 20));

        let last = ListPage::new(45, 20, 3);
        assert_eq!((last.first(), last.last(), last.items()), (41, 45, 5));
    }

    #[test]
    fn a_page_out_of_range_gives_the_nearest_page() {
        assert_eq!(ListPage::new(45, 20, 0).number, 1);
        assert_eq!(ListPage::new(45, 20, 9).number, 3);
    }

    #[test]
    fn an_empty_list_has_one_empty_page() {
        let page = ListPage::new(0, 20, 4);
        assert_eq!((page.number, page.pages(), page.items()), (1, 1, 0));
    }

    #[test]
    fn the_window_keeps_both_ends_and_the_neighbors() {
        let page = ListPage::new(200, 20, 5);
        let window = page.window();
        assert_eq!(
            window,
            [Some(1), None, Some(4), Some(5), Some(6), None, Some(10)]
        );
    }

    #[test]
    fn the_window_never_hides_a_single_page() {
        // Page 2 sits between page 1 and page 3, so it shows instead of an ellipsis.
        let window = ListPage::new(200, 20, 4).window();
        assert_eq!(window[..4], [Some(1), Some(2), Some(3), Some(4)]);
    }
}
