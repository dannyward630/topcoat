# TODO

## Runtime navigation for registry link components

`sidebar_menu_button`, `breadcrumb_link`, and `pagination_link` use ordinary links, which reload the page. The admin sidebar uses `link` with the sidebar's CSS classes to navigate without a full reload.

Passing `link_attrs(cx, href, prefetch_mode(cx))` through a component's `attrs` also works. The spread replaces the component's `href` and adds `data-topcoat-link`. For `sidebar_menu_button`, you must still pass `href: Some(..)` to make it render a link instead of a button. This means passing the URL twice.

Using `link` inside the components would avoid that duplication, but each component would need a way to choose the prefetch mode.

Possible approaches:

- Let a parent element enable runtime navigation for its links. In `navigation.ts`, use `closest()` to find `data-topcoat-link`. A link could override its parent's setting, with a new value such as `off` for ordinary navigation. `never` already means runtime navigation without prefetching. This would also work for handwritten links and links rendered from Markdown. It would make link behavior less obvious from the link itself, and setting `intent` on a container would enable prefetching for all its links. This needs runtime tests and updates to `docs/runtime/link.md` and `llms.txt`.
- Include the prefetch mode in a new `LinkTarget` type. Components would accept `href: Option<LinkTarget>`. Conversions from strings and `href!` would use the context's default mode, with an override such as `href!(..).prefetch(PrefetchMode::Viewport)`. Each link component would still need to adopt the new type.
- Keep passing the URL twice with `link_attrs` for now.

The current preference is to let links inherit the setting from a parent. The runtime should still leave external URLs, same-page fragments, downloads, and links with a target to the browser. Responses that are not Topcoat pages should still fall back to a full page load.
