//! HTTP handlers for authentication and invitation flows.

use super::{Auth, current_user};
use crate::templates::{
    ErrorMessageTemplate, ForgotFormTemplate, LoginFormTemplate, MessagePageTemplate,
    ResetFormTemplate, SignupFormTemplate, SuccessMessageTemplate, VerifyFormTemplate,
};
use askama::Template;
use axum::{
    extract::{Form, Query, State},
    http::{HeaderName, HeaderValue},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_extra::extract::SignedCookieJar;
use rs_auth_axum::cookie::{remove_session_cookie, set_session_cookie};
use rs_auth_axum::extract::{ClientInfo, resolve_optional_user};
use rs_auth_core::{error::AuthError, types::NewUser};
use serde::Deserialize;

fn error_fragment(message: &str) -> Response {
    let template = ErrorMessageTemplate {
        message: message.to_string(),
    };
    Html(template.render().unwrap()).into_response()
}

fn success_fragment(message: &str) -> Response {
    let template = SuccessMessageTemplate {
        message: message.to_string(),
    };
    Html(template.render().unwrap()).into_response()
}

fn message_page(title: &str, message: &str, link_href: &str, link_text: &str) -> Response {
    let template = MessagePageTemplate {
        title: title.to_string(),
        message: message.to_string(),
        link_href: link_href.to_string(),
        link_text: link_text.to_string(),
    };
    Html(template.render().unwrap()).into_response()
}

/// Tells htmx to navigate to another page after a form post
fn htmx_redirect(jar: SignedCookieJar, to: &'static str) -> Response {
    (jar, [("HX-Redirect", to)]).into_response()
}

/// Renders the login form
pub async fn login_form(State(auth): State<Auth>, jar: SignedCookieJar) -> Response {
    if current_user(&auth, &jar).await.is_some() {
        return Redirect::to("/").into_response();
    }
    Html(LoginFormTemplate {}.render().unwrap()).into_response()
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
}

/// Handles login POST: checks the password, then sets the session cookie
pub async fn login_post(
    State(auth): State<Auth>,
    jar: SignedCookieJar,
    client: ClientInfo,
    Form(input): Form<LoginForm>,
) -> Response {
    match auth
        .service
        .login(&input.email, &input.password, client.ip, client.user_agent)
        .await
    {
        Ok(result) => {
            let jar = set_session_cookie(
                jar,
                &result.session_token,
                &auth.config.cookie,
                auth.config.session_ttl,
            );
            htmx_redirect(jar, "/")
        }
        Err(AuthError::InvalidCredentials) => error_fragment("Invalid email or password."),
        Err(AuthError::EmailNotVerified) => error_fragment(
            "Please verify your email address first. Check your inbox for the verification link.",
        ),
        Err(e) => {
            println!("Login failed: {e}");
            error_fragment("Something went wrong. Please try again.")
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct InviteQuery {
    pub invite: Option<String>,
}

/// Renders signup only for a current, unused invitation link.
pub async fn signup_form(
    State(auth): State<Auth>,
    jar: SignedCookieJar,
    Query(query): Query<InviteQuery>,
) -> Response {
    if current_user(&auth, &jar).await.is_some() {
        return no_referrer(Redirect::to("/").into_response());
    }
    let response = if let Some(token) = query.invite.filter(|token| !token.is_empty()) {
        match crate::db::invitations::find_by_token(&auth.service.users, &token).await {
            Ok(Some(invitation)) => Html(
                SignupFormTemplate {
                    invite_token: token,
                    email: invitation.email,
                }
                .render()
                .unwrap(),
            )
            .into_response(),
            Ok(None) => message_page(
                "Invitation expired",
                "This invitation is no longer valid. Ask someone on your team to send a new one.",
                "/login",
                "Back to sign in",
            ),
            Err(error) => {
                println!("Invitation lookup failed: {error}");
                message_page(
                    "Something went wrong",
                    "We couldn't check this invitation. Please try again later.",
                    "/login",
                    "Back to sign in",
                )
            }
        }
    } else {
        message_page(
            "Invitation required",
            "Ask someone on your team to invite you to Exchange-Ops.",
            "/login",
            "Back to sign in",
        )
    };
    no_referrer(response)
}

#[derive(Debug, Deserialize)]
pub struct SignupForm {
    pub invite_token: String,
    pub name: String,
    pub email: String,
    pub password: String,
}

/// Handles signup POST: creates the user and sends the verification email
pub async fn signup_post(
    State(auth): State<Auth>,
    client: ClientInfo,
    Form(input): Form<SignupForm>,
) -> Response {
    let name = input.name.trim().to_string();
    let email = input.email.trim().to_string();
    let invitation = match crate::db::invitations::find_for_signup(
        &auth.service.users,
        &input.invite_token,
        &email,
    )
    .await
    {
        Ok(Some(invitation)) => invitation,
        Ok(None) => return error_fragment("This invitation is invalid or has expired."),
        Err(error) => {
            println!("Invitation validation failed: {error}");
            return error_fragment("Something went wrong. Please try again.");
        }
    };
    let check_email = format!(
        "Almost done! We sent a verification link to {email}. Open it to activate your account."
    );

    let new_user = NewUser {
        email: email.clone(),
        name: (!name.is_empty()).then_some(name),
        password: input.password,
    };
    match auth
        .service
        .signup(new_user, client.ip, client.user_agent)
        .await
    {
        Ok(_) => {
            if let Err(error) =
                crate::db::invitations::accept(&auth.service.users, invitation.id).await
            {
                println!("Could not mark invitation as accepted: {error}");
            }
            success_fragment(&check_email)
        }
        // Send verification for an invited address that already has an account, then consume the invite.
        Err(AuthError::EmailTaken) => {
            if let Err(e) = auth.service.resend_verification(&email).await {
                println!("Verification resend failed: {e}");
            }
            if let Err(error) =
                crate::db::invitations::accept(&auth.service.users, invitation.id).await
            {
                println!("Could not mark invitation as accepted: {error}");
            }
            success_fragment(&check_email)
        }
        Err(AuthError::WeakPassword(min)) => {
            error_fragment(&format!("Password must be at least {min} characters."))
        }
        Err(e) => {
            println!("Signup failed: {e}");
            error_fragment("Something went wrong. Please try again.")
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct InviteForm {
    pub email: String,
}

/// Sends an email-bound invitation from the signed-in dashboard.
pub async fn invite_post(
    State(auth): State<Auth>,
    jar: SignedCookieJar,
    Form(input): Form<InviteForm>,
) -> Response {
    let Some(inviter) = current_user(&auth, &jar).await else {
        return Redirect::to("/login").into_response();
    };
    let email = input.email.trim().to_lowercase();
    if email.is_empty() {
        return error_fragment("Enter an email address.");
    }

    let token = rs_auth_core::crypto::token::generate_token(64);
    if let Err(error) =
        crate::db::invitations::create(&auth.service.users, &email, &token, inviter.id).await
    {
        println!("Could not create invitation: {error}");
        return error_fragment("Couldn't send the invitation. Please try again.");
    }

    if let Err(error) = auth.service.email.send_invitation(&email, &token).await {
        println!("Could not email invitation: {error}");
        // Burn the unused token so a failed email can't leave a live invitation behind.
        if let Ok(Some(invitation)) =
            crate::db::invitations::find_by_token(&auth.service.users, &token).await
        {
            let _ = crate::db::invitations::accept(&auth.service.users, invitation.id).await;
        }
        return error_fragment("Couldn't send the invitation. Please try again.");
    }

    success_fragment(&format!("Invitation sent to {email}."))
}

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: String,
}

/// Handles the link from the verification email; signs the user in on success
pub async fn verify_form(Query(query): Query<TokenQuery>) -> Response {
    let html = VerifyFormTemplate { token: query.token }.render().unwrap();
    no_referrer(Html(html).into_response())
}

#[derive(Debug, Deserialize)]
pub struct VerifyForm {
    pub token: String,
}

/// A verification link only renders a confirmation page; consume the token after an explicit POST.
pub async fn verify_post(
    State(auth): State<Auth>,
    jar: SignedCookieJar,
    client: ClientInfo,
    Form(input): Form<VerifyForm>,
) -> Response {
    match auth
        .service
        .verify_email(&input.token, client.ip, client.user_agent)
        .await
    {
        Ok(result) => match result.session_token {
            Some(token) => {
                let jar =
                    set_session_cookie(jar, &token, &auth.config.cookie, auth.config.session_ttl);
                (jar, Redirect::to("/")).into_response()
            }
            None => message_page(
                "Email verified",
                "Your email address is verified. You can now log in.",
                "/login",
                "Log in",
            ),
        },
        Err(e) => {
            println!("Email verification failed: {e}");
            message_page(
                "Link expired",
                "This verification link is invalid or has expired. Sign up again to get a new one.",
                "/signup",
                "Back to sign up",
            )
        }
    }
}

/// Renders the forgot-password form
pub async fn forgot_form() -> Html<String> {
    Html(ForgotFormTemplate {}.render().unwrap())
}

#[derive(Debug, Deserialize)]
pub struct ForgotForm {
    pub email: String,
}

/// Handles forgot-password POST: emails a reset link if the account exists
pub async fn forgot_post(State(auth): State<Auth>, Form(input): Form<ForgotForm>) -> Response {
    if let Err(e) = auth.service.request_password_reset(&input.email).await {
        // Logged only: an error here would reveal that the account exists
        println!("Password reset request failed: {e}");
    }
    success_fragment(
        "If an account exists for that email, we've sent a link to choose a new password.",
    )
}

/// Renders the form behind the link in the password-reset email
pub async fn reset_form(Query(query): Query<TokenQuery>) -> Response {
    no_referrer(Html(ResetFormTemplate { token: query.token }.render().unwrap()).into_response())
}

fn no_referrer(mut response: Response) -> Response {
    response.headers_mut().insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_static("no-store"),
    );
    response
}

#[derive(Debug, Deserialize)]
pub struct ResetForm {
    pub token: String,
    pub password: String,
    pub confirm: String,
}

/// Handles reset POST: sets the new password and signs out every existing session
pub async fn reset_post(State(auth): State<Auth>, Form(input): Form<ResetForm>) -> Response {
    if input.password != input.confirm {
        return error_fragment("The passwords don't match.");
    }
    match auth
        .service
        .reset_password(&input.token, &input.password)
        .await
    {
        Ok(_) => success_fragment("Your password has been changed. You can now log in."),
        Err(AuthError::WeakPassword(min)) => {
            error_fragment(&format!("Password must be at least {min} characters."))
        }
        Err(AuthError::InvalidToken) => error_fragment(
            "This reset link is invalid or has expired. Request a new one from the login page.",
        ),
        Err(e) => {
            println!("Password reset failed: {e}");
            error_fragment("Something went wrong. Please try again.")
        }
    }
}

/// Handles signout POST: deletes the session and clears the cookie
pub async fn signout(State(auth): State<Auth>, jar: SignedCookieJar) -> Response {
    if let Ok(optional) = resolve_optional_user(&auth, &jar).await
        && let Some(session) = optional.session
        && let Err(e) = auth.service.logout(session.id).await
    {
        println!("Failed to delete session: {e}");
    }
    let jar = remove_session_cookie(jar, &auth.config.cookie);
    (jar, Redirect::to("/")).into_response()
}
