//! MariaDB/MySQL persistence layer for rs-auth.
//!
//! Implements the store traits from [`rs_auth_core`] using [`diesel`] with a
//! MySQL backend (tested against MariaDB).
//!
//! MariaDB-specific behavior:
//! - MariaDB has no `INSERT ... RETURNING`, so inserted rows are read back by
//!   their auto-increment id.
//! - Timestamps are `DATETIME(6)` columns holding UTC, written from Rust and
//!   compared against `UTC_TIMESTAMP(6)` (`TIMESTAMP` would overflow in 2038).
//! - Emails are unique case-insensitively through the table's `_ci` collation.

mod account;
pub mod db;
mod oauth_state;
mod schema;
mod session;
mod user;
mod verification;

pub use db::AuthDb;
