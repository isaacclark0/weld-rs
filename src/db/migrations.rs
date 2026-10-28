use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use rs_auth_mysql::db::AuthPool;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// Apply the app's single, ordered migration set, including the rs-auth tables.
pub fn run(pool: &AuthPool) -> Result<(), String> {
    let mut connection = pool.get().map_err(|error| error.to_string())?;
    connection
        .run_pending_migrations(MIGRATIONS)
        .map_err(|error| error.to_string())?;
    Ok(())
}
