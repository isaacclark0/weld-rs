//! Sends rs-auth's verification and password-reset emails over SMTP.
//!
//! In development the app is pointed at the mailcrab container (http://localhost:1080).

use async_trait::async_trait;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, MultiPart},
    transport::smtp::authentication::Credentials,
};
use rs_auth_core::{email::EmailSender, error::AuthError, types::User};
use std::env;

pub struct SmtpEmailSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
    app_url: String,
}

impl SmtpEmailSender {
    /// Configures SMTP from `SMTP_HOST`, `SMTP_PORT`, `SMTP_TLS` (none, starttls or tls),
    /// `SMTP_USERNAME`, `SMTP_PASSWORD` and `MAIL_FROM`.
    pub fn from_env(app_url: String) -> Self {
        let host = env::var("SMTP_HOST").unwrap_or_else(|_| "localhost".to_string());
        let tls = env::var("SMTP_TLS").unwrap_or_else(|_| "starttls".to_string());

        let builder = match tls.as_str() {
            // Only for local development (mailcrab); never send real mail unencrypted
            "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&host),
            "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)
                .expect("Invalid SMTP_HOST"),
            "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&host).expect("Invalid SMTP_HOST"),
            other => panic!("SMTP_TLS must be none, starttls or tls (got `{other}`)"),
        };
        let mut builder = match env::var("SMTP_PORT").ok().and_then(|p| p.parse().ok()) {
            Some(port) => builder.port(port),
            None => builder,
        };
        let username = env::var("SMTP_USERNAME").unwrap_or_default();
        if !username.is_empty() {
            let password = env::var("SMTP_PASSWORD").unwrap_or_default();
            builder = builder.credentials(Credentials::new(username, password));
        }

        let from = env::var("MAIL_FROM")
            .unwrap_or_else(|_| "Exchange-Ops <no-reply@localhost>".to_string())
            .parse()
            .expect(
                "MAIL_FROM must be an email address, e.g. `Exchange-Ops <no-reply@example.com>`",
            );

        Self {
            transport: builder.build(),
            from,
            app_url,
        }
    }

    async fn send(
        &self,
        user: &User,
        subject: &str,
        intro: &str,
        link: &str,
        action: &str,
    ) -> Result<(), AuthError> {
        let to = Mailbox::new(
            user.name.clone(),
            user.email
                .parse()
                .map_err(|e| AuthError::Internal(format!("invalid recipient address: {e}")))?,
        );
        self.send_to(to, subject, intro, link, action).await
    }

    async fn send_to(
        &self,
        to: Mailbox,
        subject: &str,
        intro: &str,
        link: &str,
        action: &str,
    ) -> Result<(), AuthError> {
        let text = format!(
            "{intro}\n\n{action}:\n{link}\n\nThis link expires soon. If you didn't request this, you can ignore this email."
        );
        let html = format!(
            r#"<p>{intro}</p><p><a href="{link}" style="display:inline-block;padding:10px 16px;background:#3b82f6;color:#fff;border-radius:4px;text-decoration:none">{action}</a></p><p style="color:#6b7280">This link expires soon. If you didn't request this, you can ignore this email.</p>"#
        );
        let message = Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(subject)
            .multipart(MultiPart::alternative_plain_html(text, html))
            .map_err(|e| AuthError::Internal(format!("could not build email: {e}")))?;

        self.transport
            .send(message)
            .await
            .map(|_| ())
            .map_err(|e| AuthError::Internal(format!("could not send email: {e}")))
    }

    pub async fn send_invitation(&self, email: &str, token: &str) -> Result<(), AuthError> {
        let to = Mailbox::new(
            None,
            email
                .parse()
                .map_err(|e| AuthError::Internal(format!("invalid recipient address: {e}")))?,
        );
        let link = format!("{}/signup?invite={token}", self.app_url);
        self.send_to(
            to,
            "You're invited to Exchange-Ops",
            "You have been invited to join Exchange-Ops.",
            &link,
            "Create your account",
        )
        .await
    }
}

#[async_trait]
impl EmailSender for SmtpEmailSender {
    async fn send_verification_email(&self, user: &User, token: &str) -> Result<(), AuthError> {
        let link = format!("{}/verify?token={token}", self.app_url);
        self.send(
            user,
            "Verify your Exchange-Ops account",
            "Welcome to Exchange-Ops! Please confirm your email address to finish setting up your account.",
            &link,
            "Verify email",
        )
        .await
    }

    async fn send_password_reset_email(&self, user: &User, token: &str) -> Result<(), AuthError> {
        let link = format!("{}/reset?token={token}", self.app_url);
        self.send(
            user,
            "Reset your Exchange-Ops password",
            "We received a request to reset your Exchange-Ops password.",
            &link,
            "Choose a new password",
        )
        .await
    }
}
