use async_trait::async_trait;
use diesel::{OptionalExtension, Queryable, Selectable, insert_into, prelude::*};
use rs_auth_core::{
    error::AuthError,
    store::AccountStore,
    types::{Account, NewAccount},
};
use time::OffsetDateTime;

use crate::{
    db::{AuthDb, from_naive, to_naive},
    schema::accounts,
};

#[derive(Queryable, Selectable)]
#[diesel(table_name = accounts)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
struct AccountRow {
    id: i64,
    user_id: i64,
    provider_id: String,
    account_id: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
    access_token_expires_at: Option<chrono::NaiveDateTime>,
    scope: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = accounts)]
struct NewAccountRow {
    user_id: i64,
    provider_id: String,
    account_id: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
    access_token_expires_at: Option<chrono::NaiveDateTime>,
    scope: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<AccountRow> for Account {
    fn from(row: AccountRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            provider_id: row.provider_id,
            account_id: row.account_id,
            access_token: row.access_token,
            refresh_token: row.refresh_token,
            access_token_expires_at: row.access_token_expires_at.map(from_naive),
            scope: row.scope,
            created_at: from_naive(row.created_at),
            updated_at: from_naive(row.updated_at),
        }
    }
}

fn store_error(error: impl std::fmt::Display) -> AuthError {
    AuthError::Store(error.to_string())
}

#[async_trait]
impl AccountStore for AuthDb {
    async fn create_account(&self, account: NewAccount) -> Result<Account, AuthError> {
        let now = to_naive(OffsetDateTime::now_utc());
        let new_account = NewAccountRow {
            user_id: account.user_id,
            provider_id: account.provider_id,
            account_id: account.account_id,
            access_token: account.access_token,
            refresh_token: account.refresh_token,
            access_token_expires_at: account.access_token_expires_at.map(to_naive),
            scope: account.scope,
            created_at: now,
            updated_at: now,
        };

        self.with_connection(move |connection| {
            insert_into(accounts::table)
                .values(&new_account)
                .execute(connection)
                .map_err(store_error)?;
            let id: i64 = diesel::select(diesel::dsl::sql::<diesel::sql_types::Bigint>(
                "LAST_INSERT_ID()",
            ))
            .get_result(connection)
            .map_err(store_error)?;
            accounts::table
                .find(id)
                .select(AccountRow::as_select())
                .first::<AccountRow>(connection)
                .map(Account::from)
                .map_err(store_error)
        })
        .await
    }

    async fn find_by_provider(
        &self,
        provider_id: &str,
        account_id: &str,
    ) -> Result<Option<Account>, AuthError> {
        let provider_id = provider_id.to_owned();
        let account_id = account_id.to_owned();
        self.with_connection(move |connection| {
            accounts::table
                .filter(accounts::provider_id.eq(provider_id))
                .filter(accounts::account_id.eq(account_id))
                .select(AccountRow::as_select())
                .first::<AccountRow>(connection)
                .optional()
                .map(|row| row.map(Account::from))
                .map_err(store_error)
        })
        .await
    }

    async fn find_by_user_id(&self, user_id: i64) -> Result<Vec<Account>, AuthError> {
        self.with_connection(move |connection| {
            accounts::table
                .filter(accounts::user_id.eq(user_id))
                .order(accounts::created_at.desc())
                .select(AccountRow::as_select())
                .load::<AccountRow>(connection)
                .map(|rows| rows.into_iter().map(Account::from).collect())
                .map_err(store_error)
        })
        .await
    }

    async fn delete_account(&self, id: i64) -> Result<(), AuthError> {
        self.with_connection(move |connection| {
            diesel::delete(accounts::table.find(id))
                .execute(connection)
                .map(|_| ())
                .map_err(store_error)
        })
        .await
    }

    async fn update_account(
        &self,
        id: i64,
        access_token: Option<String>,
        refresh_token: Option<String>,
        expires_at: Option<OffsetDateTime>,
        scope: Option<String>,
    ) -> Result<(), AuthError> {
        let expires_at = expires_at.map(to_naive);
        self.with_connection(move |connection| {
            diesel::update(accounts::table.find(id))
                .set((
                    accounts::access_token.eq(access_token),
                    accounts::refresh_token.eq(refresh_token),
                    accounts::access_token_expires_at.eq(expires_at),
                    accounts::scope.eq(scope),
                    accounts::updated_at.eq(diesel::dsl::now),
                ))
                .execute(connection)
                .map(|_| ())
                .map_err(store_error)
        })
        .await
    }
}
