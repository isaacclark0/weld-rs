# Auth library changes

## Local fork

- Added a Diesel-backed MariaDB storage adapter.
- Moved app and auth schema changes into the repository-root migration set.
- Kept the shared auth service and Axum integration in separate crates.
