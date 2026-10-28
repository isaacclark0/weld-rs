# Exchange-Ops

Exchange-Ops is an invite-only team app built with Rust, Axum, Askama, HTMX, Diesel, and MariaDB. The browser UI is rendered by the Rust server; there is no separate frontend service to build or deploy.

## Quick start

### Requirements

- Git
- Docker with the Docker Compose v2 plugin

The Compose setup provides the Rust toolchain, MariaDB, Tailwind CSS watcher, and local email inbox. You do not need Rust or Node installed on your host to run the app this way.

### Start the app

1. Clone the repository and enter the project directory.
2. Create your local environment file:

   ```bash
   cp .env.example .env
   ```

3. Edit `.env`. Change `DB_PASSWORD` and replace `AUTH_SECRET` with a random value. Generate a suitable secret with:

   ```bash
   openssl rand -hex 32
   ```

   The command prints 64 characters; `AUTH_SECRET` must be at least 64 characters.

4. Build and start the full local stack with one command:

   ```bash
   docker compose up --build
   ```

5. Open [http://localhost:4001](http://localhost:4001). Sign in with the development account below, then invite another user from the dashboard.

The app applies the Diesel migrations when it starts. The local Compose configuration creates this verified development account by default:

- Email: `test@example.com`
- Password: `local-test-password`

Verification, password reset, and invitation emails are caught by Mailcrab at [http://localhost:1080](http://localhost:1080).

## Local commands

Run these from the repository root:

| Task | Command |
| --- | --- |
| Start all services in the foreground | `docker compose up` |
| Start all services in the background | `docker compose up -d` |
| Rebuild images and start | `docker compose up --build` |
| Check service status | `docker compose ps` |
| Follow app logs | `docker compose logs -f app` |
| Restart the app container | `docker compose restart app` |
| Stop the services | `docker compose down` |
| Stop services and delete local database/build volumes | `docker compose down -v` |

`docker compose down -v` deletes the local MariaDB data. Use it only when you want a fresh development database.

The development app container runs `cargo watch`, so Rust source, templates, migrations, and Cargo manifest/lockfile changes trigger a rebuild automatically. Tailwind CSS is watched by the `css` service.

### Local ports

- App: `APP_PORT` (default `4001`)
- Mailcrab: `MAILCRAB_PORT` (default `1080`)
- MariaDB on the host: `DB_PORT` (default `3307`)

Change these values in `.env` if the defaults conflict with another local service.

## Running Rust on the host

This is optional; Docker Compose is the recommended development path.

Install Rust and the MariaDB development headers for your operating system. The container toolchain is Rust 1.88. Start MariaDB, Mailcrab, and the CSS watcher in Docker:

```bash
docker compose up -d db mailcrab css
```

The `.env` file uses a host connection URL on `127.0.0.1:3307` and SMTP on `localhost:1025`. If you changed the database credentials or port, update the literal `DATABASE_URL` value to match. Then run the app from the repository root with:

```bash
cargo run
```

Migrations still run automatically. If you changed `APP_PORT`, update `APP_PUBLIC_URL` in `.env` to the matching `http://localhost:<port>` address. Compose sets the public URL for its own app container.

To stop the local supporting services:

```bash
docker compose down
```

## Configuration

`.env.example` documents the development variables. Copy it to `.env`; do not commit `.env`.

| Variable | Purpose |
| --- | --- |
| `DB_USER`, `DB_PASSWORD`, `DB_NAME` | MariaDB database and application user |
| `DB_PORT` | MariaDB host port; the container listens on `3306` internally |
| `DATABASE_URL` | Connection string for running the app or Diesel CLI on the host; Compose overrides it for the app container |
| `APP_PORT` | HTTP port |
| `APP_PUBLIC_URL` | Base URL used in email links |
| `AUTH_SECRET` | Session-cookie signing secret, at least 64 characters |
| `DEV_SEED_EMAIL`, `DEV_SEED_PASSWORD` | Optional verified development account; Compose supplies local defaults |
| `SMTP_HOST`, `SMTP_PORT`, `SMTP_TLS` | SMTP server settings; Compose points these at Mailcrab |
| `SMTP_USERNAME`, `SMTP_PASSWORD` | Optional SMTP credentials |
| `MAIL_FROM` | Sender address used in auth emails |

The Compose file is for local development and enables the test account by default. For a deployment, use the production `Dockerfile`, provide secure environment values, and leave both `DEV_SEED_EMAIL` and `DEV_SEED_PASSWORD` unset.

## Project structure

```text
.
├── src/                         # Exchange-Ops backend
│   ├── main.rs                  # Startup, configuration, dependency wiring
│   ├── routes.rs                # Axum routes and signed-in home page
│   ├── auth.rs                  # Auth configuration and app-level auth helpers
│   ├── auth/handlers.rs         # HTML form handlers and invite-only signup
│   ├── db/                      # Pool, migrations, invitation repository/schema
│   ├── email.rs                 # SMTP email sender
│   ├── security.rs              # Request protections and rate limits
│   └── templates/mod.rs         # Rust data passed into Askama templates
├── templates/                   # Askama HTML pages and reusable components
├── static/                      # CSS source, auth styles, and vendored HTMX
├── migrations/                  # One ordered migration history for app and auth tables
└── crates/rs-auth/              # Shared authentication crates
    ├── core/                    # Auth service, core types, and storage traits
    ├── mysql/                   # Diesel/MariaDB storage adapter and table schema
    ├── axum/                    # Axum cookies, extractors, middleware, handlers
    └── auth/                    # Convenience re-exports of the auth crates
```

### Request and data flow

The browser submits an HTML form to an Axum handler. The handler validates the request and calls the shared auth service. The service uses storage traits implemented by `rs-auth-mysql`; that adapter runs synchronous Diesel work on Tokio's blocking pool and maps database rows into auth types. Askama renders full pages and HTMX response fragments.

`User` is the complete persisted auth record, including its password hash. `PublicUser` omits the password hash and is used when exposing a user to the app. `NewUser` is the auth-service signup input; app-specific HTTP forms are separate `Deserialize` structs in `src/auth/handlers.rs`.

## Database migrations

All app and auth migrations live in the repository-root `migrations/` directory and run at startup. Applied migrations are immutable: create a new migration for each schema change rather than editing one that may already exist in a database.

Generate a migration in the development container with:

```bash
docker compose exec app diesel migration generate describe_the_change
```

Add SQL to the generated `up.sql` and `down.sql`. The invitation table is app-owned; auth tables are used by the MySQL adapter. If you change auth table columns, update the matching schema and row types in `crates/rs-auth/mysql/` as well as the migration.

The historical `posts` create/drop entries are retained to preserve migration history; the current schema has no `posts` table.

## Dependencies and upgrades

The Rust packages are members of the root Cargo workspace and share the root `Cargo.lock`.

### Rust dependencies

- Shared dependencies belong in `[workspace.dependencies]` in the root `Cargo.toml`.
- Add a shared dependency to a crate with `{ workspace = true }` in that crate's manifest. Add an app-only dependency under the root package's `[dependencies]`.
- To upgrade dependencies within their declared version ranges, run:

  ```bash
  cargo update
  ```

- To update one dependency and its compatible transitive dependencies, run:

  ```bash
  cargo update -p diesel
  ```

- If intentionally moving to a newer release line, update the version requirement in the appropriate `Cargo.toml` first, then run `cargo update`.
- Review and commit `Cargo.lock` with the manifest changes. Check the workspace before merging:

  ```bash
  cargo fmt --check
  cargo check --workspace
  cargo test --workspace
  ```

  The MariaDB integration tests need `RS_AUTH_MYSQL_TEST_URL` set to a database server URL for a user allowed to create and drop test databases. Without it, those integration tests skip.

### Node and CSS dependencies

`package.json` contains Tailwind and Prettier development dependencies; `package-lock.json` pins their resolved versions.

- Reinstall the locked Node packages with `npm ci`.
- Upgrade packages within the ranges in `package.json` with `npm update`.
- Change a direct package requirement with `npm install --save-dev <package>@<version>`.
- Review and commit both `package.json` and `package-lock.json`.

The Compose CSS service and production image install from the lockfile. Rebuild after changing dependencies:

```bash
docker compose build app
docker compose restart css
```

### Rust and base images

The development and production Dockerfiles use Rust 1.88; Compose uses MariaDB 11.4 and Node 22 for CSS. Update those image tags in `Dockerfile`, `Dockerfile.dev`, or `docker-compose.yml` as appropriate, then rebuild the affected service.

The auth source is vendored under `crates/rs-auth/`. Record local changes in `crates/rs-auth/UPSTREAM.md` when syncing updates from its upstream repository.

## Production image

`Dockerfile` builds the app binary and CSS assets into a smaller runtime image. The development Compose stack is not a production deployment: it contains Mailcrab and supplies development seed credentials. For deployment, provide MariaDB and SMTP services, set the required environment variables securely, and do not configure development seed credentials.
