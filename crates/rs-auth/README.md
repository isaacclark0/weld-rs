# Exchange-Ops authentication crates

This directory contains the authentication library used by the Exchange-Ops app.

## Packages

- `core/` contains auth service logic, public and private user types, token handling, and storage traits.
- `mysql/` implements those storage traits with Diesel and MariaDB.
- `axum/` provides Axum state, extractors, cookies, middleware, and optional JSON routes.
- `auth/` re-exports the core, MySQL, and Axum packages for consumers that want one dependency.

The web app is in the repository root. Its HTML forms and Askama templates are in `src/auth/handlers.rs` and `templates/`; its app-owned invitation table is managed by the root `migrations/` directory.

## Data types

- `User` is the full persisted auth record and includes the password hash.
- `PublicUser` is safe to pass to response and template code; it has no password hash.
- `NewUser` is the service input and contains a plaintext password only until the auth service hashes it.
- HTTP forms are separate `Deserialize` request types in the app's auth handlers.

The app and auth crates share the root Cargo workspace and lockfile.
