# Vendored fork of rs-auth

This directory is a fork of [rs-auth](https://github.com/rs-auth/rs-auth), copied into the
Exchange-Ops repo so it can be changed alongside the app.

- Upstream: https://github.com/rs-auth/rs-auth
- Forked from: `f80be3ebe2dfc92ad11d7fd721306df5e52ed328` (v0.1.2 + docs commit, 2026-04-16)
- License: MIT OR Apache-2.0 (see `LICENSE-MIT` and `LICENSE-APACHE`)
- Not copied: the `docs/` website and `.github/` workflows

## Changes from upstream

- Added `mysql/` (`rs-auth-mysql`): a MariaDB Diesel store implementing the shared auth store
  traits. The Exchange-Ops app owns the shared migrations in the repository-root
  `migrations/` directory.

To pull in upstream changes, diff this directory against a newer upstream checkout and apply
the relevant changes by hand.
