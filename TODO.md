# TODO

## Runtime navigation for registry link components

`sidebar_menu_button`, `breadcrumb_link`, and `pagination_link` render plain
`<a>` elements, so they do a full page load. The admin demo works around this
in `demos/admin/src/app/_admin.rs` by rebuilding the sidebar button's markup
around `link`.

What already works: spreading `link_attrs(cx, href, prefetch_mode(cx))` into a
component's `attrs` renders one `href` plus `data-topcoat-link`, because
spread attributes replace literal ones. `sidebar_menu_button` still needs
`href: Some(..)` as well, or it renders a `<button>`, so the URL is passed
twice.

Using `link` inside each component means also passing a prefetch mode through
each one, which doesn't scale.

Options:

- **Inherit from an ancestor.** Resolve `data-topcoat-link` with
  `closest()` in `navigation.ts` instead of reading it from the anchor only.
  `<body>` or a container then turns on runtime navigation for every link
  inside it, an anchor's own attribute overrides that, and a new value like
  `off` opts a link out (`never` already means "runtime navigation, no
  prefetch"). The router's `prefetch(..)` could set it on `<body>`.
  Registry components stay unchanged, and handwritten and Markdown links are
  covered too. It's safe to apply broadly: same-page fragments, other sites,
  `target`, and `download` links fall through to the browser, and a response
  that isn't a Topcoat page falls back to a full load (`navigation.ts:231`).
  Costs: it's implicit like `hx-boost`, an inherited `intent` prefetches a
  whole region, and it's a runtime change (tests, `docs/link.md`, `llms.txt`).
- **Carry the mode in the href.** Components take `href: Option<LinkTarget>`;
  `&str` and `href!` convert with the context default, and something like
  `href!(..).prefetch(PrefetchMode::Viewport)` overrides it. One prop per
  component instead of two, but each link component still changes.
- **Do nothing for now.** Pass the URL twice with `link_attrs`, as above.

Leaning towards inheriting from an ancestor.
