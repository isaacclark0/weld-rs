use async_trait::async_trait;
use diesel::{
    RunQueryDsl,
    mysql::MysqlConnection,
    r2d2::{ConnectionManager, Pool},
    sql_query,
};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use rs_auth_core::{
    AuthConfig, AuthService,
    crypto::token,
    email::EmailSender,
    error::AuthError,
    store::{SessionStore, VerificationStore},
    types::*,
};
use rs_auth_mysql::{AuthDb, db::AuthPool};
use std::sync::{Arc, Mutex};
use time::{Duration, OffsetDateTime};

/// Test email sender that captures tokens for verification
#[derive(Clone)]
struct TestEmailSender {
    tokens: Arc<Mutex<Vec<String>>>,
}

impl TestEmailSender {
    fn new() -> Self {
        Self {
            tokens: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn last_token(&self) -> Option<String> {
        self.tokens.lock().unwrap().last().cloned()
    }
}

#[async_trait]
impl EmailSender for TestEmailSender {
    async fn send_verification_email(&self, _user: &User, token: &str) -> Result<(), AuthError> {
        self.tokens.lock().unwrap().push(token.to_string());
        Ok(())
    }

    async fn send_password_reset_email(&self, _user: &User, token: &str) -> Result<(), AuthError> {
        self.tokens.lock().unwrap().push(token.to_string());
        Ok(())
    }
}

/// Set to a MariaDB/MySQL server URL *without* a database name, for a user allowed to create
/// databases, e.g. `mysql://root:password@127.0.0.1:3306`. Tests are skipped when it's unset.
const TEST_URL_VAR: &str = "RS_AUTH_MYSQL_TEST_URL";
const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../../../migrations");

/// Drops the per-test database when the test finishes.
struct TestDatabase {
    server_url: String,
    name: String,
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let server_url = self.server_url.clone();
        let name = self.name.clone();
        // Database cleanup is synchronous; keep it off the async test thread.
        let _ = std::thread::spawn(move || {
            let manager = ConnectionManager::<MysqlConnection>::new(server_url);
            if let Ok(pool) = Pool::builder().build(manager) {
                if let Ok(mut conn) = pool.get() {
                    let _ =
                        sql_query(format!("DROP DATABASE IF EXISTS `{name}`")).execute(&mut conn);
                }
            }
        })
        .join();
    }
}

/// Helper function to set up a fresh test database and service.
/// Returns None if `RS_AUTH_MYSQL_TEST_URL` is not set (tests will skip gracefully).
async fn setup() -> Option<(
    AuthPool,
    AuthService<AuthDb, AuthDb, AuthDb, AuthDb, AuthDb, TestEmailSender>,
    TestDatabase,
)> {
    let Ok(server_url) = std::env::var(TEST_URL_VAR) else {
        eprintln!("{TEST_URL_VAR} not set, skipping");
        return None;
    };
    let server_url = server_url.trim_end_matches('/').to_string();

    let name = format!("rs_auth_test_{}", token::generate_token(12).to_lowercase());
    let admin_manager = ConnectionManager::<MysqlConnection>::new(&server_url);
    let admin = Pool::builder().build(admin_manager).unwrap();
    let mut admin_conn = admin.get().unwrap();
    sql_query(format!("CREATE DATABASE `{name}`"))
        .execute(&mut admin_conn)
        .unwrap();
    drop(admin_conn);
    drop(admin);
    let database = TestDatabase {
        server_url: server_url.clone(),
        name: name.clone(),
    };

    let database_url = format!("{server_url}/{name}");
    let manager = ConnectionManager::<MysqlConnection>::new(database_url);
    let pool = Pool::builder().build(manager).unwrap();
    pool.get()
        .unwrap()
        .run_pending_migrations(MIGRATIONS)
        .unwrap();

    let db = AuthDb::new(pool.clone());
    let email_sender = TestEmailSender::new();
    let service = AuthService::new(
        AuthConfig::default(),
        db.clone(),
        db.clone(),
        db.clone(),
        db.clone(),
        db.clone(),
        email_sender,
    );

    Some((pool, service, database))
}

#[tokio::test]
async fn test_signup_creates_user_and_returns_session() {
    let Some((pool, service, _database)) = setup().await else {
        return;
    };

    let result = service
        .signup(
            NewUser {
                email: "test@example.com".to_string(),
                name: Some("Test User".to_string()),
                password: "password123".to_string(),
            },
            None,
            None,
        )
        .await
        .unwrap();

    assert_eq!(result.user.email, "test@example.com");
    assert_eq!(result.user.name, Some("Test User".to_string()));
    assert!(result.session.is_some(), "should auto-login after signup");
    assert!(
        result.session_token.is_some(),
        "should return session token"
    );

    drop(pool);
}

#[tokio::test]
async fn test_signup_duplicate_email_returns_error() {
    let Some((pool, service, _database)) = setup().await else {
        return;
    };

    let new_user = NewUser {
        email: "duplicate@example.com".to_string(),
        name: Some("User".to_string()),
        password: "password123".to_string(),
    };

    // First signup should succeed
    service.signup(new_user.clone(), None, None).await.unwrap();

    // Second signup with same email should fail
    let result = service.signup(new_user, None, None).await;
    assert!(matches!(result, Err(AuthError::EmailTaken)));

    drop(pool);
}

#[tokio::test]
async fn test_login_valid_credentials_returns_session() {
    let Some((pool, service, _database)) = setup().await else {
        return;
    };

    // Create user
    service
        .signup(
            NewUser {
                email: "login@example.com".to_string(),
                name: Some("Login User".to_string()),
                password: "password123".to_string(),
            },
            None,
            None,
        )
        .await
        .unwrap();

    // Login with valid credentials
    let result = service
        .login("login@example.com", "password123", None, None)
        .await
        .unwrap();

    assert_eq!(result.user.email, "login@example.com");
    assert!(!result.session_token.is_empty());

    drop(pool);
}

#[tokio::test]
async fn test_login_wrong_password_returns_error() {
    let Some((pool, service, _database)) = setup().await else {
        return;
    };

    // Create user
    service
        .signup(
            NewUser {
                email: "wrongpass@example.com".to_string(),
                name: Some("User".to_string()),
                password: "password123".to_string(),
            },
            None,
            None,
        )
        .await
        .unwrap();

    // Login with wrong password
    let result = service
        .login("wrongpass@example.com", "wrongpassword", None, None)
        .await;

    assert!(matches!(result, Err(AuthError::InvalidCredentials)));

    drop(pool);
}

#[tokio::test]
async fn test_login_nonexistent_email_returns_error() {
    let Some((pool, service, _database)) = setup().await else {
        return;
    };

    let result = service
        .login("nonexistent@example.com", "password123", None, None)
        .await;

    assert!(matches!(result, Err(AuthError::InvalidCredentials)));

    drop(pool);
}

#[tokio::test]
async fn test_logout_deletes_session() {
    let Some((pool, service, _database)) = setup().await else {
        return;
    };

    // Create user and get session
    let signup_result = service
        .signup(
            NewUser {
                email: "logout@example.com".to_string(),
                name: Some("Logout User".to_string()),
                password: "password123".to_string(),
            },
            None,
            None,
        )
        .await
        .unwrap();

    let session = signup_result.session.unwrap();
    let session_token = signup_result.session_token.unwrap();

    // Verify session exists
    let session_result = service.get_session(&session_token).await;
    assert!(session_result.is_ok());

    // Logout
    service.logout(session.id).await.unwrap();

    // Verify session is deleted
    let result = service.get_session(&session_token).await;
    assert!(matches!(result, Err(AuthError::SessionNotFound)));

    drop(pool);
}

#[tokio::test]
async fn test_get_session_returns_user_and_session() {
    let Some((pool, service, _database)) = setup().await else {
        return;
    };

    // Create user and get session
    let signup_result = service
        .signup(
            NewUser {
                email: "getsession@example.com".to_string(),
                name: Some("Get Session User".to_string()),
                password: "password123".to_string(),
            },
            None,
            None,
        )
        .await
        .unwrap();

    let session_token = signup_result.session_token.unwrap();

    // Get session
    let result = service.get_session(&session_token).await.unwrap();

    assert_eq!(result.user.email, "getsession@example.com");
    assert_eq!(result.session.user_id, result.user.id);

    drop(pool);
}

#[tokio::test]
async fn test_get_session_expired_returns_error() {
    let Some((pool, service, _database)) = setup().await else {
        return;
    };

    // Create user
    let signup_result = service
        .signup(
            NewUser {
                email: "expired@example.com".to_string(),
                name: Some("Expired User".to_string()),
                password: "password123".to_string(),
            },
            None,
            None,
        )
        .await
        .unwrap();

    let user_id = signup_result.user.id;

    // Create an expired session manually
    let raw_token = token::generate_token(32);
    let expired_session = NewSession {
        token_hash: token::hash_token(&raw_token),
        user_id,
        expires_at: OffsetDateTime::now_utc() - Duration::hours(1), // Expired 1 hour ago
        ip_address: None,
        user_agent: None,
    };

    let db = AuthDb::new(pool.clone());
    db.create_session(expired_session).await.unwrap();

    // Try to get expired session
    let result = service.get_session(&raw_token).await;
    assert!(matches!(result, Err(AuthError::SessionNotFound)));

    drop(pool);
}

#[tokio::test]
async fn test_email_verification_marks_user_verified() {
    let Some((pool, _service, _container)) = setup().await else {
        return;
    };

    // Create config that sends verification email
    let mut config = AuthConfig::default();
    config.email.send_verification_on_signup = true;

    let db = AuthDb::new(pool.clone());
    let email_sender = TestEmailSender::new();
    let service = AuthService::new(
        config,
        db.clone(),
        db.clone(),
        db.clone(),
        db.clone(),
        db.clone(),
        email_sender.clone(),
    );

    // Signup
    let signup_result = service
        .signup(
            NewUser {
                email: "verify@example.com".to_string(),
                name: Some("Verify User".to_string()),
                password: "password123".to_string(),
            },
            None,
            None,
        )
        .await
        .unwrap();

    assert!(signup_result.user.email_verified_at.is_none());

    // Get verification token
    let verification_token = email_sender.last_token().unwrap();

    // Verify email
    let result = service
        .verify_email(&verification_token, None, None)
        .await
        .unwrap();

    assert!(result.user.email_verified_at.is_some());

    drop(pool);
}

#[tokio::test]
async fn test_email_verification_expired_token_returns_error() {
    let Some((pool, service, _database)) = setup().await else {
        return;
    };

    // Create user
    let signup_result = service
        .signup(
            NewUser {
                email: "expiredverify@example.com".to_string(),
                name: Some("User".to_string()),
                password: "password123".to_string(),
            },
            None,
            None,
        )
        .await
        .unwrap();

    // Create an expired verification token manually
    let raw_token = token::generate_token(32);
    let expired_verification = NewVerification {
        identifier: format!("email-verify:{}", signup_result.user.email),
        token_hash: token::hash_token(&raw_token),
        expires_at: OffsetDateTime::now_utc() - Duration::hours(1), // Expired 1 hour ago
    };

    let db = AuthDb::new(pool.clone());
    db.create_verification(expired_verification).await.unwrap();

    // Try to verify with expired token
    let result = service.verify_email(&raw_token, None, None).await;
    assert!(matches!(result, Err(AuthError::InvalidToken)));

    drop(pool);
}

#[tokio::test]
async fn test_forgot_password_always_returns_ok() {
    let Some((pool, service, _database)) = setup().await else {
        return;
    };

    // Request reset for nonexistent email should still return Ok
    let result = service
        .request_password_reset("nonexistent@example.com")
        .await;
    assert!(result.is_ok());

    // Create user
    service
        .signup(
            NewUser {
                email: "reset@example.com".to_string(),
                name: Some("Reset User".to_string()),
                password: "password123".to_string(),
            },
            None,
            None,
        )
        .await
        .unwrap();

    // Request reset for existing email should also return Ok
    let result = service.request_password_reset("reset@example.com").await;
    assert!(result.is_ok());

    drop(pool);
}

#[tokio::test]
async fn test_reset_password_changes_password_and_revokes_sessions() {
    let Some((pool, _service, _container)) = setup().await else {
        return;
    };

    let db = AuthDb::new(pool.clone());
    let email_sender = TestEmailSender::new();
    let service = AuthService::new(
        AuthConfig::default(),
        db.clone(),
        db.clone(),
        db.clone(),
        db.clone(),
        db.clone(),
        email_sender.clone(),
    );

    // Create user and login
    let signup_result = service
        .signup(
            NewUser {
                email: "resetpass@example.com".to_string(),
                name: Some("Reset Pass User".to_string()),
                password: "oldpassword123".to_string(),
            },
            None,
            None,
        )
        .await
        .unwrap();

    let old_session_token = signup_result.session_token.unwrap();

    // Request password reset
    service
        .request_password_reset("resetpass@example.com")
        .await
        .unwrap();

    let reset_token = email_sender.last_token().unwrap();

    // Reset password
    service
        .reset_password(&reset_token, "newpassword123")
        .await
        .unwrap();

    // Old session should be revoked
    let result = service.get_session(&old_session_token).await;
    assert!(matches!(result, Err(AuthError::SessionNotFound)));

    // Old password should not work
    let result = service
        .login("resetpass@example.com", "oldpassword123", None, None)
        .await;
    assert!(matches!(result, Err(AuthError::InvalidCredentials)));

    // New password should work
    let result = service
        .login("resetpass@example.com", "newpassword123", None, None)
        .await;
    assert!(result.is_ok());

    drop(pool);
}

#[tokio::test]
async fn test_reset_password_expired_token_returns_error() {
    let Some((pool, service, _database)) = setup().await else {
        return;
    };

    // Create user
    let signup_result = service
        .signup(
            NewUser {
                email: "expiredreset@example.com".to_string(),
                name: Some("User".to_string()),
                password: "password123".to_string(),
            },
            None,
            None,
        )
        .await
        .unwrap();

    // Create an expired reset token manually
    let raw_token = token::generate_token(32);
    let expired_verification = NewVerification {
        identifier: format!("password-reset:{}", signup_result.user.email),
        token_hash: token::hash_token(&raw_token),
        expires_at: OffsetDateTime::now_utc() - Duration::hours(1), // Expired 1 hour ago
    };

    let db = AuthDb::new(pool.clone());
    db.create_verification(expired_verification).await.unwrap();

    // Try to reset with expired token
    let result = service.reset_password(&raw_token, "newpassword123").await;
    assert!(matches!(result, Err(AuthError::InvalidToken)));

    drop(pool);
}

#[tokio::test]
async fn test_list_sessions_returns_all_active_sessions() {
    let Some((pool, service, _database)) = setup().await else {
        return;
    };

    // Create user
    let signup_result = service
        .signup(
            NewUser {
                email: "listsessions@example.com".to_string(),
                name: Some("List Sessions User".to_string()),
                password: "password123".to_string(),
            },
            None,
            None,
        )
        .await
        .unwrap();

    let user_id = signup_result.user.id;

    // Create additional sessions by logging in
    service
        .login("listsessions@example.com", "password123", None, None)
        .await
        .unwrap();

    service
        .login(
            "listsessions@example.com",
            "password123",
            Some("192.168.1.1".to_string()),
            Some("Mozilla/5.0".to_string()),
        )
        .await
        .unwrap();

    // List sessions
    let sessions = service.list_sessions(user_id).await.unwrap();

    // Should have 3 sessions total (1 from signup + 2 from login)
    assert_eq!(sessions.len(), 3);
    assert!(sessions.iter().all(|s| s.user_id == user_id));

    drop(pool);
}
