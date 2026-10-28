use async_trait::async_trait;
use diesel::{OptionalExtension, Queryable, Selectable, insert_into, prelude::*};
use rs_auth_core::{
    error::AuthError,
    store::VerificationStore,
    types::{NewVerification, Verification},
};
use time::OffsetDateTime;

use crate::{
    db::{AuthDb, from_naive, to_naive},
    schema::verifications,
};

#[derive(Queryable, Selectable)]
#[diesel(table_name = verifications)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
struct VerificationRow {
    id: i64,
    identifier: String,
    token_hash: String,
    expires_at: chrono::NaiveDateTime,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = verifications)]
struct NewVerificationRow {
    identifier: String,
    token_hash: String,
    expires_at: chrono::NaiveDateTime,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<VerificationRow> for Verification {
    fn from(row: VerificationRow) -> Self {
        Self {
            id: row.id,
            identifier: row.identifier,
            token_hash: row.token_hash,
            expires_at: from_naive(row.expires_at),
            created_at: from_naive(row.created_at),
            updated_at: from_naive(row.updated_at),
        }
    }
}

fn store_error(error: impl std::fmt::Display) -> AuthError {
    AuthError::Store(error.to_string())
}

#[async_trait]
impl VerificationStore for AuthDb {
    async fn create_verification(
        &self,
        verification: NewVerification,
    ) -> Result<Verification, AuthError> {
        let now = to_naive(OffsetDateTime::now_utc());
        let new_verification = NewVerificationRow {
            identifier: verification.identifier,
            token_hash: verification.token_hash,
            expires_at: to_naive(verification.expires_at),
            created_at: now,
            updated_at: now,
        };

        self.with_connection(move |connection| {
            insert_into(verifications::table)
                .values(&new_verification)
                .execute(connection)
                .map_err(store_error)?;
            let id: i64 = diesel::select(diesel::dsl::sql::<diesel::sql_types::Bigint>(
                "LAST_INSERT_ID()",
            ))
            .get_result(connection)
            .map_err(store_error)?;
            verifications::table
                .find(id)
                .select(VerificationRow::as_select())
                .first::<VerificationRow>(connection)
                .map(Verification::from)
                .map_err(store_error)
        })
        .await
    }

    async fn find_by_identifier(
        &self,
        identifier: &str,
    ) -> Result<Option<Verification>, AuthError> {
        let identifier = identifier.to_owned();
        self.with_connection(move |connection| {
            verifications::table
                .filter(verifications::identifier.eq(identifier))
                .select(VerificationRow::as_select())
                .first::<VerificationRow>(connection)
                .optional()
                .map(|row| row.map(Verification::from))
                .map_err(store_error)
        })
        .await
    }

    async fn find_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<Verification>, AuthError> {
        let token_hash = token_hash.to_owned();
        self.with_connection(move |connection| {
            verifications::table
                .filter(verifications::token_hash.eq(token_hash))
                .select(VerificationRow::as_select())
                .first::<VerificationRow>(connection)
                .optional()
                .map(|row| row.map(Verification::from))
                .map_err(store_error)
        })
        .await
    }

    async fn delete_verification(&self, id: i64) -> Result<(), AuthError> {
        self.with_connection(move |connection| {
            diesel::delete(verifications::table.find(id))
                .execute(connection)
                .map(|_| ())
                .map_err(store_error)
        })
        .await
    }

    async fn delete_by_identifier(&self, identifier: &str) -> Result<(), AuthError> {
        let identifier = identifier.to_owned();
        self.with_connection(move |connection| {
            diesel::delete(verifications::table.filter(verifications::identifier.eq(identifier)))
                .execute(connection)
                .map(|_| ())
                .map_err(store_error)
        })
        .await
    }

    async fn delete_expired(&self) -> Result<u64, AuthError> {
        self.with_connection(|connection| {
            diesel::delete(
                verifications::table.filter(verifications::expires_at.lt(diesel::dsl::now)),
            )
            .execute(connection)
            .map(|count| count as u64)
            .map_err(store_error)
        })
        .await
    }
}
