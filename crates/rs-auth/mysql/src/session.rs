use async_trait::async_trait;
use diesel::{OptionalExtension, Queryable, Selectable, insert_into, prelude::*};
use rs_auth_core::{
    error::AuthError,
    store::SessionStore,
    types::{NewSession, Session},
};

use crate::{
    db::{AuthDb, from_naive, to_naive},
    schema::sessions,
};

#[derive(Queryable, Selectable)]
#[diesel(table_name = sessions)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
struct SessionRow {
    id: i64,
    token_hash: String,
    user_id: i64,
    expires_at: chrono::NaiveDateTime,
    ip_address: Option<String>,
    user_agent: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = sessions)]
struct NewSessionRow {
    token_hash: String,
    user_id: i64,
    expires_at: chrono::NaiveDateTime,
    ip_address: Option<String>,
    user_agent: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<SessionRow> for Session {
    fn from(row: SessionRow) -> Self {
        Self {
            id: row.id,
            token_hash: row.token_hash,
            user_id: row.user_id,
            expires_at: from_naive(row.expires_at),
            ip_address: row.ip_address,
            user_agent: row.user_agent,
            created_at: from_naive(row.created_at),
            updated_at: from_naive(row.updated_at),
        }
    }
}

fn store_error(error: impl std::fmt::Display) -> AuthError {
    AuthError::Store(error.to_string())
}

#[async_trait]
impl SessionStore for AuthDb {
    async fn create_session(&self, session: NewSession) -> Result<Session, AuthError> {
        let now = to_naive(time::OffsetDateTime::now_utc());
        let new_session = NewSessionRow {
            token_hash: session.token_hash,
            user_id: session.user_id,
            expires_at: to_naive(session.expires_at),
            ip_address: session.ip_address,
            user_agent: session.user_agent,
            created_at: now,
            updated_at: now,
        };

        self.with_connection(move |connection| {
            insert_into(sessions::table)
                .values(&new_session)
                .execute(connection)
                .map_err(store_error)?;
            let id: i64 = diesel::select(diesel::dsl::sql::<diesel::sql_types::Bigint>(
                "LAST_INSERT_ID()",
            ))
            .get_result(connection)
            .map_err(store_error)?;
            sessions::table
                .find(id)
                .select(SessionRow::as_select())
                .first::<SessionRow>(connection)
                .map(Session::from)
                .map_err(store_error)
        })
        .await
    }

    async fn find_by_token_hash(&self, token_hash: &str) -> Result<Option<Session>, AuthError> {
        let token_hash = token_hash.to_owned();
        self.with_connection(move |connection| {
            sessions::table
                .filter(sessions::token_hash.eq(token_hash))
                .select(SessionRow::as_select())
                .first::<SessionRow>(connection)
                .optional()
                .map(|row| row.map(Session::from))
                .map_err(store_error)
        })
        .await
    }

    async fn find_by_user_id(&self, user_id: i64) -> Result<Vec<Session>, AuthError> {
        self.with_connection(move |connection| {
            sessions::table
                .filter(sessions::user_id.eq(user_id))
                .filter(sessions::expires_at.gt(diesel::dsl::now))
                .order(sessions::created_at.desc())
                .select(SessionRow::as_select())
                .load::<SessionRow>(connection)
                .map(|rows| rows.into_iter().map(Session::from).collect())
                .map_err(store_error)
        })
        .await
    }

    async fn delete_session(&self, id: i64) -> Result<(), AuthError> {
        self.with_connection(move |connection| {
            diesel::delete(sessions::table.find(id))
                .execute(connection)
                .map(|_| ())
                .map_err(store_error)
        })
        .await
    }

    async fn delete_by_user_id(&self, user_id: i64) -> Result<(), AuthError> {
        self.with_connection(move |connection| {
            diesel::delete(sessions::table.filter(sessions::user_id.eq(user_id)))
                .execute(connection)
                .map(|_| ())
                .map_err(store_error)
        })
        .await
    }

    async fn delete_expired(&self) -> Result<u64, AuthError> {
        self.with_connection(|connection| {
            diesel::delete(sessions::table.filter(sessions::expires_at.lt(diesel::dsl::now)))
                .execute(connection)
                .map(|count| count as u64)
                .map_err(store_error)
        })
        .await
    }
}
