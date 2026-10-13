//! The orders placed in the store.

use topcoat::{
    Result,
    context::Cx,
    router::page,
    view::{View, view},
};

use crate::features::auth::require_user;

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    require_user(cx).await?;

    Ok(view! {
        <header class="flex flex-col gap-1">
            <h1 class="text-2xl font-semibold tracking-tight">"Orders"</h1>
        </header>
    })
}
