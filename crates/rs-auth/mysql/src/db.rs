use diesel::mysql::MysqlConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use rs_auth_core::error::AuthError;

pub type AuthPool = Pool<ConnectionManager<MysqlConnection>>;

/// MariaDB/MySQL database wrapper for rs-auth storage backends.
#[derive(Clone, Debug)]
pub struct AuthDb {
    /// Diesel MySQL connection pool.
    pool: AuthPool,
}

impl AuthDb {
    pub fn new(pool: AuthPool) -> Self {
        Self { pool }
    }

    /// Run a synchronous Diesel operation on Tokio's blocking thread pool.
    ///
    /// The store traits are async, while Diesel's MySQL connection is synchronous. Keeping the
    /// worker boundary here prevents database waits from blocking request executor threads.
    pub async fn with_connection<T, F>(&self, operation: F) -> Result<T, AuthError>
    where
        T: Send + 'static,
        F: FnOnce(&mut MysqlConnection) -> Result<T, AuthError> + Send + 'static,
    {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = pool
                .get()
                .map_err(|error| AuthError::Store(error.to_string()))?;
            operation(&mut connection)
        })
        .await
        .map_err(|error| AuthError::Store(format!("database worker failed: {error}")))?
    }
}

pub(crate) fn to_naive(value: time::OffsetDateTime) -> chrono::NaiveDateTime {
    chrono::DateTime::from_timestamp(value.unix_timestamp(), value.nanosecond())
        .expect("valid OffsetDateTime")
        .naive_utc()
}

pub(crate) fn from_naive(value: chrono::NaiveDateTime) -> time::OffsetDateTime {
    let value = value.and_utc();
    time::OffsetDateTime::from_unix_timestamp(value.timestamp())
        .expect("database timestamp is in the supported range")
        .replace_nanosecond(value.timestamp_subsec_nanos())
        .expect("valid database timestamp")
}
