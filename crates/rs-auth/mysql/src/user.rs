use async_trait::async_trait;
use diesel::{
    OptionalExtension, Queryable, Selectable,
    dsl::sql,
    insert_into,
    prelude::*,
    result::{DatabaseErrorKind, Error as DieselError},
    sql_types::Bigint,
};
use rs_auth_core::{error::AuthError, store::UserStore, types::User};
use time::OffsetDateTime;

use crate::{
    db::{AuthDb, from_naive, to_naive},
    schema::users,
};

#[derive(Queryable, Selectable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
struct UserRow {
    id: i64,
    email: String,
    name: Option<String>,
    password_hash: Option<String>,
    email_verified_at: Option<chrono::NaiveDateTime>,
    image: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = users)]
struct NewUserRow {
    email: String,
    name: Option<String>,
    password_hash: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            email: row.email,
            name: row.name,
            password_hash: row.password_hash,
            email_verified_at: row.email_verified_at.map(from_naive),
            image: row.image,
            created_at: from_naive(row.created_at),
            updated_at: from_naive(row.updated_at),
        }
    }
}

fn store_error(error: impl std::fmt::Display) -> AuthError {
    AuthError::Store(error.to_string())
}

#[async_trait]
impl UserStore for AuthDb {
    async fn create_user(
        &self,
        email: &str,
        name: Option<&str>,
        password_hash: Option<&str>,
    ) -> Result<User, AuthError> {
        let now = to_naive(OffsetDateTime::now_utc());
        let new_user = NewUserRow {
            email: email.to_lowercase(),
            name: name.map(str::to_owned),
            password_hash: password_hash.map(str::to_owned),
            created_at: now,
            updated_at: now,
        };

        self.with_connection(move |connection| {
            if let Err(error) = insert_into(users::table)
                .values(&new_user)
                .execute(connection)
            {
                return Err(match error {
                    DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _) => {
                        AuthError::EmailTaken
                    }
                    other => store_error(other),
                });
            }

            let id: i64 = diesel::select(sql::<Bigint>("LAST_INSERT_ID()"))
                .get_result(connection)
                .map_err(store_error)?;
            users::table
                .find(id)
                .select(UserRow::as_select())
                .first::<UserRow>(connection)
                .map(User::from)
                .map_err(store_error)
        })
        .await
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>, AuthError> {
        let email = email.to_lowercase();
        self.with_connection(move |connection| {
            users::table
                .filter(users::email.eq(email))
                .select(UserRow::as_select())
                .first::<UserRow>(connection)
                .optional()
                .map(|row| row.map(User::from))
                .map_err(store_error)
        })
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<User>, AuthError> {
        self.with_connection(move |connection| {
            users::table
                .find(id)
                .select(UserRow::as_select())
                .first::<UserRow>(connection)
                .optional()
                .map(|row| row.map(User::from))
                .map_err(store_error)
        })
        .await
    }

    async fn set_email_verified(&self, id: i64) -> Result<(), AuthError> {
        self.with_connection(move |connection| {
            diesel::update(users::table.find(id))
                .set((
                    users::email_verified_at.eq(diesel::dsl::now),
                    users::updated_at.eq(diesel::dsl::now),
                ))
                .execute(connection)
                .map(|_| ())
                .map_err(store_error)
        })
        .await
    }

    async fn update_password(&self, id: i64, password_hash: &str) -> Result<(), AuthError> {
        let password_hash = password_hash.to_owned();
        self.with_connection(move |connection| {
            diesel::update(users::table.find(id))
                .set((
                    users::password_hash.eq(password_hash),
                    users::updated_at.eq(diesel::dsl::now),
                ))
                .execute(connection)
                .map(|_| ())
                .map_err(store_error)
        })
        .await
    }

    async fn delete_user(&self, id: i64) -> Result<(), AuthError> {
        self.with_connection(move |connection| {
            diesel::delete(users::table.find(id))
                .execute(connection)
                .map(|_| ())
                .map_err(store_error)
        })
        .await
    }
}
