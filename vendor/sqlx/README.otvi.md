# Vendored sqlx

This directory contains a minimal `sqlx` facade crate that replaces the
upstream `sqlx` crate via a `[patch.crates-io]` override in the workspace
`Cargo.toml`.

## Why

The upstream `sqlx` crate (`sqlx` v0.8.6 on crates.io) bundles drivers for
MySQL, PostgreSQL, SQLite, and MSSQL. OTVI only needs **SQLite** and
**PostgreSQL**. This facade:

- Reduces compile time by excluding unused drivers (MySQL, MSSQL).
- Locks the exact `sqlx-core` / `sqlx-postgres` / `sqlx-sqlite` versions.
- Provides a single `install_default_drivers()` entry point that registers
  only the SQLite and PostgreSQL `Any` driver backends.

## What changed

The `src/lib.rs` re-exports everything from `sqlx-core` and conditionally from
`sqlx-postgres` / `sqlx-sqlite`. The `Cargo.toml` only depends on the two
drivers OTVI uses. All version numbers match the upstream 0.8.6 release.

## Upstream tracking

- **Upstream repo:** https://github.com/launchbadge/sqlx
- **Upstream version:** 0.8.6
- **Relevant issue:** None — this is a build-time optimization, not a bug fix.

## Re-vendoring

To update to a newer upstream sqlx version:

1. Update the `version` in `vendor/sqlx/Cargo.toml` to match.
2. Update the `sqlx-core`, `sqlx-postgres`, and `sqlx-sqlite` dependency
   versions to match the new release.
3. Compare `src/lib.rs` against the upstream `sqlx/src/lib.rs` for any new
   re-exports or module changes.
4. Run `cargo check -p otvi-server` and `cargo test -p otvi-server` to verify.

## Known limitations

- `FromRow` derive with `AnyPool` has a [known type-mapping
  bug](https://github.com/launchbadge/sqlx/issues/3635). OTVI works around
  this by manually extracting columns with `row.get()` / `row.try_get()`
  instead of deriving `FromRow` on row types.