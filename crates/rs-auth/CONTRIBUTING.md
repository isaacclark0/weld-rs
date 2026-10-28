# Contributing

The auth workspace contains four crates:

- `core/` for authentication logic, shared data types, and storage interfaces.
- `mysql/` for Diesel-backed MariaDB persistence.
- `axum/` for HTTP integration.
- `auth/` for convenience re-exports.

Keep application-owned features such as invitations in the root app's modules and migrations. Keep shared authentication behavior in `core/`, MariaDB queries in `mysql/`, and Axum-specific behavior in `axum/`.

From the repository root, use the app's normal `cargo fmt` and `cargo check` commands in the development container. Run database integration checks only when a local MariaDB test URL is configured.

This is a vendored auth library maintained with the app. Record upstream provenance and notable local changes in `UPSTREAM.md`.
