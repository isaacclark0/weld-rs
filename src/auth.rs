//! Authentication service configuration and application-facing auth helpers.

pub mod handlers;

use axum_extra::extract::SignedCookieJar;
use rs_auth_axum::{AuthState, extract::resolve_optional_user};
use rs_auth_core::{AuthConfig, error::AuthError, store::UserStore, types::PublicUser};
use rs_auth_mysql::AuthDb;

use crate::email::SmtpEmailSender;

/// Auth state with all persistence stores backed by MariaDB and emails sent over SMTP.
pub type Auth = AuthState<AuthDb, AuthDb, AuthDb, AuthDb, AuthDb, SmtpEmailSender>;

/// Configure the auth service for the app's verified email/password flow.
pub fn config(secret: String, app_url: &str) -> AuthConfig {
    assert!(
        secret.len() >= 64,
        "AUTH_SECRET must be at least 64 characters (generate one with `openssl rand -hex 32`)"
    );
    let mut config = AuthConfig {
        secret,
        ..AuthConfig::default()
    };
    config.email.send_verification_on_signup = true;
    config.email.require_verification_to_login = true;
    config.email.auto_sign_in_after_signup = false;
    config.email.auto_sign_in_after_verification = true;
    config.cookie.secure = app_url.starts_with("https://");
    config
}

/// Return the signed-in user when the request has a valid session cookie.
pub async fn current_user(auth: &Auth, jar: &SignedCookieJar) -> Option<PublicUser> {
    match resolve_optional_user(auth, jar).await {
        Ok(optional) => optional.user,
        Err(error) => {
            eprintln!("Session lookup failed: {}", error.0);
            None
        }
    }
}

/// Name displayed in the dashboard, falling back to the user's email.
pub fn display_name(user: &PublicUser) -> String {
    user.name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| user.email.clone())
}

/// Seed a verified local account when both development seed values are configured.
pub async fn seed_dev_user(db: &AuthDb) -> Result<(), AuthError> {
    let email = std::env::var("DEV_SEED_EMAIL").ok();
    let password = std::env::var("DEV_SEED_PASSWORD").ok();
    let (Some(email), Some(password)) = (email, password) else {
        if std::env::var_os("DEV_SEED_EMAIL").is_some()
            || std::env::var_os("DEV_SEED_PASSWORD").is_some()
        {
            return Err(AuthError::Internal(
                "DEV_SEED_EMAIL and DEV_SEED_PASSWORD must be set together".to_string(),
            ));
        }
        return Ok(());
    };

    let user = match db.find_by_email(&email).await? {
        Some(user) => user,
        None => {
            let password_hash = rs_auth_core::crypto::hash::hash_password(&password)?;
            db.create_user(&email, Some("Test User"), Some(&password_hash))
                .await?
        }
    };

    if user.email_verified_at.is_none() {
        db.set_email_verified(user.id).await?;
    }
    println!("Development test account is ready: {email}");
    Ok(())
}
