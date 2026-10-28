use async_trait::async_trait;
use diesel::{OptionalExtension, Queryable, Selectable, insert_into, prelude::*};
use rs_auth_core::{
    error::AuthError,
    store::OAuthStateStore,
    types::{NewOAuthState, OAuthIntent, OAuthState},
};
use time::OffsetDateTime;

use crate::{
    db::{AuthDb, from_naive, to_naive},
    schema::oauth_states,
};

#[derive(Queryable, Selectable)]
#[diesel(table_name = oauth_states)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
struct OAuthStateRow {
    id: i64,
    provider_id: String,
    csrf_state: String,
    pkce_verifier: String,
    intent: String,
    link_user_id: Option<i64>,
    expires_at: chrono::NaiveDateTime,
    created_at: chrono::NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = oauth_states)]
struct NewOAuthStateRow {
    provider_id: String,
    csrf_state: String,
    pkce_verifier: String,
    intent: String,
    link_user_id: Option<i64>,
    expires_at: chrono::NaiveDateTime,
    created_at: chrono::NaiveDateTime,
}

impl From<OAuthStateRow> for OAuthState {
    fn from(row: OAuthStateRow) -> Self {
        Self {
            id: row.id,
            provider_id: row.provider_id,
            csrf_state: row.csrf_state,
            pkce_verifier: row.pkce_verifier,
            intent: if row.intent == "link" {
                OAuthIntent::Link
            } else {
                OAuthIntent::Login
            },
            link_user_id: row.link_user_id,
            expires_at: from_naive(row.expires_at),
            created_at: from_naive(row.created_at),
        }
    }
}

fn store_error(error: impl std::fmt::Display) -> AuthError {
    AuthError::Store(error.to_string())
}

#[async_trait]
impl OAuthStateStore for AuthDb {
    async fn create_oauth_state(&self, state: NewOAuthState) -> Result<OAuthState, AuthError> {
        let now = to_naive(OffsetDateTime::now_utc());
        let intent = match state.intent {
            OAuthIntent::Login => "login".to_owned(),
            OAuthIntent::Link => "link".to_owned(),
        };
        let new_state = NewOAuthStateRow {
            provider_id: state.provider_id,
            csrf_state: state.csrf_state,
            pkce_verifier: state.pkce_verifier,
            intent,
            link_user_id: state.link_user_id,
            expires_at: to_naive(state.expires_at),
            created_at: now,
        };

        self.with_connection(move |connection| {
            insert_into(oauth_states::table)
                .values(&new_state)
                .execute(connection)
                .map_err(store_error)?;
            let id: i64 = diesel::select(diesel::dsl::sql::<diesel::sql_types::Bigint>(
                "LAST_INSERT_ID()",
            ))
            .get_result(connection)
            .map_err(store_error)?;
            oauth_states::table
                .find(id)
                .select(OAuthStateRow::as_select())
                .first::<OAuthStateRow>(connection)
                .map(OAuthState::from)
                .map_err(store_error)
        })
        .await
    }

    async fn find_by_csrf_state(&self, csrf_state: &str) -> Result<Option<OAuthState>, AuthError> {
        let csrf_state = csrf_state.to_owned();
        self.with_connection(move |connection| {
            oauth_states::table
                .filter(oauth_states::csrf_state.eq(csrf_state))
                .select(OAuthStateRow::as_select())
                .first::<OAuthStateRow>(connection)
                .optional()
                .map(|row| row.map(OAuthState::from))
                .map_err(store_error)
        })
        .await
    }

    async fn delete_oauth_state(&self, id: i64) -> Result<(), AuthError> {
        self.with_connection(move |connection| {
            diesel::delete(oauth_states::table.find(id))
                .execute(connection)
                .map(|_| ())
                .map_err(store_error)
        })
        .await
    }

    async fn delete_expired_oauth_states(&self) -> Result<u64, AuthError> {
        self.with_connection(|connection| {
            diesel::delete(
                oauth_states::table.filter(oauth_states::expires_at.lt(diesel::dsl::now)),
            )
            .execute(connection)
            .map(|count| count as u64)
            .map_err(store_error)
        })
        .await
    }
}
