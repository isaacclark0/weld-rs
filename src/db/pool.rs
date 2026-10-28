use diesel::MysqlConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use dotenvy::dotenv;
use rs_auth_mysql::db::AuthPool;
use std::env;

/// Create the application-wide MariaDB connection pool.
pub fn establish_pool() -> AuthPool {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<MysqlConnection>::new(database_url);
    Pool::builder()
        .build(manager)
        .expect("Failed to create database connection pool")
}
