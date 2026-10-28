//! Persistence operations for email-bound, single-use user invitations.

use chrono::NaiveDateTime;
use diesel::{OptionalExtension, Queryable, Selectable, insert_into, prelude::*};
use rs_auth_core::{crypto::token::hash_token, error::AuthError};
use rs_auth_mysql::AuthDb;

use crate::db::schema::invitations;

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = invitations)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct Invitation {
    pub id: i64,
    pub email: String,
    pub invited_by: i64,
    pub expires_at: NaiveDateTime,
    pub accepted_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = invitations)]
struct NewInvitation {
    email: String,
    token_hash: String,
    invited_by: i64,
    expires_at: NaiveDateTime,
    created_at: NaiveDateTime,
}

fn store_error(error: impl std::fmt::Display) -> AuthError {
    AuthError::Store(error.to_string())
}

pub async fn create(
    db: &AuthDb,
    email: &str,
    token: &str,
    invited_by: i64,
) -> Result<(), AuthError> {
    let now = chrono::Utc::now().naive_utc();
    let invitation = NewInvitation {
        email: email.trim().to_lowercase(),
        token_hash: hash_token(token),
        invited_by,
        expires_at: now + chrono::Duration::days(7),
        created_at: now,
    };

    db.with_connection(move |connection| {
        insert_into(invitations::table)
            .values(&invitation)
            .execute(connection)
            .map(|_| ())
            .map_err(store_error)
    })
    .await
}

pub async fn find_by_token(db: &AuthDb, token: &str) -> Result<Option<Invitation>, AuthError> {
    let token_hash = hash_token(token);
    db.with_connection(move |connection| {
        invitations::table
            .filter(invitations::token_hash.eq(token_hash))
            .filter(invitations::accepted_at.is_null())
            .filter(invitations::expires_at.gt(diesel::dsl::now))
            .select(Invitation::as_select())
            .first::<Invitation>(connection)
            .optional()
            .map_err(store_error)
    })
    .await
}

pub async fn find_for_signup(
    db: &AuthDb,
    token: &str,
    email: &str,
) -> Result<Option<Invitation>, AuthError> {
    let token_hash = hash_token(token);
    let email = email.trim().to_lowercase();
    db.with_connection(move |connection| {
        invitations::table
            .filter(invitations::token_hash.eq(token_hash))
            .filter(invitations::email.eq(email))
            .filter(invitations::accepted_at.is_null())
            .filter(invitations::expires_at.gt(diesel::dsl::now))
            .select(Invitation::as_select())
            .first::<Invitation>(connection)
            .optional()
            .map_err(store_error)
    })
    .await
}

pub async fn accept(db: &AuthDb, id: i64) -> Result<(), AuthError> {
    db.with_connection(move |connection| {
        let changed = diesel::update(
            invitations::table
                .filter(invitations::id.eq(id))
                .filter(invitations::accepted_at.is_null())
                .filter(invitations::expires_at.gt(diesel::dsl::now)),
        )
        .set(invitations::accepted_at.eq(diesel::dsl::now))
        .execute(connection)
        .map_err(store_error)?;

        if changed == 1 {
            Ok(())
        } else {
            Err(AuthError::InvalidToken)
        }
    })
    .await
}
