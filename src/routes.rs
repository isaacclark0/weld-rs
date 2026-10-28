//! Axum routes and page handlers for the server-rendered app.

use askama::Template;
use axum::{
    Router,
    extract::State,
    middleware,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::SignedCookieJar;
use rs_auth_axum::extract::resolve_optional_user;
use tower_http::{compression::CompressionLayer, services::ServeDir};

use crate::{
    auth::{self, Auth},
    security::SecurityState,
    templates::MainTemplate,
};

pub fn router(auth: Auth, app_url: &str) -> Router {
    Router::new()
        .route("/", get(home))
        .route(
            "/login",
            get(auth::handlers::login_form).post(auth::handlers::login_post),
        )
        .route(
            "/signup",
            get(auth::handlers::signup_form).post(auth::handlers::signup_post),
        )
        .route(
            "/verify",
            get(auth::handlers::verify_form).post(auth::handlers::verify_post),
        )
        .route(
            "/forgot",
            get(auth::handlers::forgot_form).post(auth::handlers::forgot_post),
        )
        .route(
            "/reset",
            get(auth::handlers::reset_form).post(auth::handlers::reset_post),
        )
        .route("/invites", post(auth::handlers::invite_post))
        .route("/signout", post(auth::handlers::signout))
        .nest_service("/static", ServeDir::new("static"))
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn_with_state(
            SecurityState::new(app_url),
            crate::security::protect_requests,
        ))
        .with_state(auth)
}

async fn home(State(auth): State<Auth>, jar: SignedCookieJar) -> Response {
    let current = match resolve_optional_user(&auth, &jar).await {
        Ok(current) => current,
        Err(error) => {
            eprintln!("Session lookup failed: {}", error.0);
            return Redirect::to("/login").into_response();
        }
    };

    match current.user {
        Some(user) => Html(
            MainTemplate {
                username: auth::display_name(&user),
                email: user.email,
                email_verified: user.email_verified_at.is_some(),
            }
            .render()
            .unwrap(),
        )
        .into_response(),
        None => Redirect::to("/login").into_response(),
    }
}
