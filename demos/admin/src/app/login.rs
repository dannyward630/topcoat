//! Signing in.

use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, try_request_context},
    icon::{icon, iconify::iconify_icon},
    router::{
        Body, Method, StatusCode,
        content::Form,
        error::{SeeOther, redirect, rewrite, see_other},
        href, page, route,
    },
    view::{View, attributes, element_id, view},
};

use crate::{
    components::{
        alert::{AlertVariant, alert, alert_description, alert_title},
        badge::{BadgeVariant, badge},
        button::button,
        card::{card, card_content, card_description, card_footer, card_header, card_title},
        field::{field, field_error, field_group, field_label},
        input::input,
    },
    features::{
        auth::{current_user, sign_in},
        user::User,
    },
    seed::{DEMO_PASSWORD, DEMO_USERS},
};

/// Tells the sign-in page that signing in failed.
///
/// A field left empty has an error. If both were filled in, they did not match an
/// account.
struct SignInFailed {
    /// The submitted email, to fill in again.
    email: String,
    email_error: Option<&'static str>,
    password_error: Option<&'static str>,
}

impl SignInFailed {
    /// Whether the email and password were filled in but matched no account.
    fn no_match(&self) -> bool {
        self.email_error.is_none() && self.password_error.is_none()
    }
}

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    if current_user(cx).await?.is_some() {
        return Err(redirect(href!(super::_admin::_dashboard::page).resolve(cx)).into());
    }

    let failed = try_request_context::<SignInFailed>(cx);
    let email = failed.map(|failed| failed.email.as_str());
    let email_error = failed.and_then(|failed| failed.email_error);
    let password_error = failed.and_then(|failed| failed.password_error);

    // IDs that connect the labels to their inputs and the button to its form.
    let form_id = element_id(cx);
    let email_id = element_id(cx);
    let email_error_id = element_id(cx);
    let password_id = element_id(cx);
    let password_error_id = element_id(cx);

    Ok(view! {
        if failed.is_some() {
            (StatusCode::UNPROCESSABLE_ENTITY)
        }

        <main class="flex min-h-svh flex-col items-center justify-center gap-6 p-6">
            <p class="flex items-center gap-2 text-lg font-semibold">
                icon(data: iconify_icon!("lucide:store"))
                "Backroom"
            </p>

            card(
                attrs: attributes! { class="w-full max-w-sm" },
                card_header(
                    card_title("Sign in")
                    card_description("Use your staff account to manage the store.")
                )
                card_content(
                    // The server checks the fields and shows its own errors, so the
                    // browser does not check the email's format.
                    <form
                        id=(&form_id)
                        method="post"
                        action=(href!(submit))
                        novalidate=""
                    >
                        field_group(
                            if failed.is_some_and(SignInFailed::no_match) {
                                alert(
                                    variant: AlertVariant::Destructive,
                                    icon(data: iconify_icon!("lucide:circle-alert"))
                                    alert_title("Wrong email or password")
                                    alert_description("Check your details and try again.")
                                )
                            }
                            field(
                                field_label(
                                    attrs: attributes! { for=(&email_id) },
                                    "Email"
                                )
                                input(
                                    attrs: attributes! {
                                        id=(&email_id)
                                        name="email"
                                        type="email"
                                        autocomplete="username"
                                        value=(email)
                                        aria-invalid=(email_error.is_some().then_some("true"))
                                        aria-describedby=(email_error
                                            .is_some()
                                            .then_some(&email_error_id))
                                    }
                                )
                                if let Some(error) = email_error {
                                    field_error(
                                        attrs: attributes! { id=(&email_error_id) },
                                        (error)
                                    )
                                }
                            )
                            field(
                                field_label(
                                    attrs: attributes! { for=(&password_id) },
                                    "Password"
                                )
                                input(
                                    attrs: attributes! {
                                        id=(&password_id)
                                        name="password"
                                        type="password"
                                        autocomplete="current-password"
                                        aria-invalid=(password_error.is_some().then_some("true"))
                                        aria-describedby=(password_error
                                            .is_some()
                                            .then_some(&password_error_id))
                                    }
                                )
                                if let Some(error) = password_error {
                                    field_error(
                                        attrs: attributes! { id=(&password_error_id) },
                                        (error)
                                    )
                                }
                            )
                        )
                    </form>
                )
                card_footer(
                    button(
                        attrs: attributes! { type="submit" form=(&form_id) class="w-full" },
                        "Sign in"
                    )
                )
            )

            demo_accounts()
        </main>
    })
}

/// Lists the seeded accounts, so visitors can try each role.
#[topcoat::view::component]
async fn demo_accounts() -> Result<impl View> {
    Ok(view! {
        card(
            attrs: attributes! { class="w-full max-w-sm" },
            card_header(
                card_title("Demo accounts")
                card_description(
                    "Every account uses the password "
                    <code class="font-mono text-foreground">(DEMO_PASSWORD)</code>
                    "."
                )
            )
            card_content(
                <ul class="flex flex-col gap-3 text-sm">
                    for (name, email, role) in DEMO_USERS {
                        <li class="flex items-center justify-between gap-4">
                            <div class="flex flex-col">
                                <span class="font-medium">(name)</span>
                                <span class="text-muted-foreground">(email)</span>
                            </div>
                            badge(variant: BadgeVariant::Outline, (role.label()))
                        </li>
                    }
                </ul>
            )
        )
    })
}

#[derive(Deserialize)]
struct Credentials {
    email: String,
    password: String,
}

/// Signs in with an email and password, or shows the form again with an error.
#[route(POST)]
async fn submit(cx: &Cx, Form(credentials): Form<Credentials>) -> Result<SeeOther> {
    let Credentials { email, password } = credentials;
    let email_error = email.trim().is_empty().then_some("Enter your email.");
    let password_error = password.is_empty().then_some("Enter your password.");

    let user = if email_error.is_none() && password_error.is_none() {
        User::authenticate(cx, &email, password).await?
    } else {
        None
    };
    let Some(user) = user else {
        // Render the sign-in page in response to this POST, without a redirect, so
        // it can show the errors next to the submitted email.
        return Err(rewrite(href!(page).resolve(cx), Body::empty())
            .method(Method::GET)
            .with(SignInFailed {
                email,
                email_error,
                password_error,
            })
            .into());
    };

    sign_in(cx, &user).await?;
    Ok(see_other(
        href!(super::_admin::_dashboard::page).resolve(cx),
    ))
}
