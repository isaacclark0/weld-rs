use rs_auth_axum::AuthState;
use rs_auth_core::AuthService;
use rs_auth_mysql::AuthDb;
use std::net::SocketAddr;

use exchange_ops::{
    auth::{self, Auth},
    db::{migrations, pool},
    email::SmtpEmailSender,
    routes,
};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let port = std::env::var("APP_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(4001);
    let app_url = std::env::var("APP_PUBLIC_URL")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("http://localhost:{port}"))
        .trim_end_matches('/')
        .to_owned();

    let auth_pool = tokio::task::spawn_blocking(|| {
        let pool = pool::establish_pool();
        migrations::run(&pool)?;
        Ok::<_, String>(pool)
    })
    .await
    .expect("Database startup worker panicked")
    .expect("Database migration failed");

    let secret = std::env::var("AUTH_SECRET").expect("AUTH_SECRET must be set (see .env.example)");
    let db = AuthDb::new(auth_pool);
    auth::seed_dev_user(&db)
        .await
        .expect("Failed to seed development test account");

    let auth: Auth = AuthState::new(AuthService::new(
        auth::config(secret, &app_url),
        db.clone(),
        db.clone(),
        db.clone(),
        db.clone(),
        db,
        SmtpEmailSender::from_env(app_url.clone()),
    ));

    let app = routes::router(auth, &app_url);
    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("Failed to bind HTTP listener");
    println!("Listening on {app_url}");

    if let Err(error) = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    {
        eprintln!("Server error: {error}");
        std::process::exit(1);
    }
}
