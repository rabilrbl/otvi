//! Database layer.
//!
//! Uses `sqlx::AnyPool` so the database backend is selected at runtime from
//! `DATABASE_URL`:
//!
//! | Backend    | Example URL                              |
//! |------------|------------------------------------------|
//! | SQLite     | `sqlite://data.db` (default)             |
//! | PostgreSQL | `postgres://user:pass@host/dbname`       |
//! | MySQL      | `mysql://user:pass@host/dbname`          |
//!
//! All queries use `?` placeholders; sqlx translates them to `$1`, `$2`, …
//! for PostgreSQL automatically.

use std::collections::HashMap;

use anyhow::Context;
use chrono::Utc;
use sqlx::{
    AnyPool, Row,
    any::{AnyConnectOptions, AnyPoolOptions},
};
use std::str::FromStr;
use uuid::Uuid;

use otvi_core::types::UserRole;

/// The shared database connection pool.
pub type Db = AnyPool;

// ── Initialisation ────────────────────────────────────────────────────────

/// Open or create the database and run any pending migrations.
///
/// Call once at startup; `install_default_drivers()` must be called before
/// this so sqlx knows which driver to use.
pub async fn init(database_url: &str) -> anyhow::Result<Db> {
    // For SQLite URLs, ensure the database file (and any parent directories)
    // exist before sqlx tries to open it.  sqlx's AnyPool doesn't expose
    // `create_if_missing` through its generic options type, so we handle it
    // here instead.
    if database_url.starts_with("sqlite:") {
        // sqlite://./foo/data.db  → strip "sqlite://"
        // sqlite:///abs/data.db   → strip "sqlite://"
        // sqlite://data.db        → strip "sqlite://"
        let raw = database_url
            .trim_start_matches("sqlite://")
            // Drop any URL query params (?journal_mode=wal etc.)
            .split('?')
            .next()
            .unwrap_or("data.db");

        // Skip in-memory databases.
        if !raw.starts_with(':') {
            let path = std::path::Path::new(raw);
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent).context("Failed to create database directory")?;
            }
            if !path.exists() {
                std::fs::File::create(path).context("Failed to create SQLite database file")?;
            }
        }
    }

    let opts = AnyConnectOptions::from_str(database_url)
        .with_context(|| format!("Invalid database URL: {database_url}"))?;

    let pool = AnyPoolOptions::new()
        .max_connections(10)
        .connect_with(opts)
        .await
        .with_context(|| format!("Failed to connect to database: {database_url}"))?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("Database migration failed")?;

    Ok(pool)
}

// ── Users ─────────────────────────────────────────────────────────────────

/// Row fetched from the `users` table.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct UserRow {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: String,
    pub must_change_password: bool,
}

/// Optional provider allow-list attached to a user (empty = all providers).
#[allow(dead_code)]
pub struct UserWithProviders {
    pub row: UserRow,
    pub providers: Vec<String>,
}

pub async fn user_count(db: &Db) -> anyhow::Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) AS cnt FROM users")
        .fetch_one(db)
        .await?;
    Ok(row.try_get::<i64, _>("cnt")?)
}

pub async fn get_user_by_username(db: &Db, username: &str) -> anyhow::Result<Option<UserRow>> {
    let row = sqlx::query(
        "SELECT id, username, password_hash, role, created_at, must_change_password \
         FROM users WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(db)
    .await?;

    Ok(row.map(|r| UserRow {
        id: r.get("id"),
        username: r.get("username"),
        password_hash: r.get("password_hash"),
        role: r.get("role"),
        created_at: r.get("created_at"),
        must_change_password: r.get::<i64, _>("must_change_password") != 0,
    }))
}

#[allow(dead_code)]
pub async fn get_user_by_id(db: &Db, id: &str) -> anyhow::Result<Option<UserRow>> {
    let row = sqlx::query(
        "SELECT id, username, password_hash, role, created_at, must_change_password \
         FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;

    Ok(row.map(|r| UserRow {
        id: r.get("id"),
        username: r.get("username"),
        password_hash: r.get("password_hash"),
        role: r.get("role"),
        created_at: r.get("created_at"),
        must_change_password: r.get::<i64, _>("must_change_password") != 0,
    }))
}

pub async fn list_users(db: &Db) -> anyhow::Result<Vec<UserRow>> {
    let rows = sqlx::query(
        "SELECT id, username, password_hash, role, created_at, must_change_password \
         FROM users ORDER BY created_at",
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| UserRow {
            id: r.get("id"),
            username: r.get("username"),
            password_hash: r.get("password_hash"),
            role: r.get("role"),
            created_at: r.get("created_at"),
            must_change_password: r.get::<i64, _>("must_change_password") != 0,
        })
        .collect())
}

pub async fn create_user(
    db: &Db,
    username: &str,
    password_hash: &str,
    role: &UserRole,
    must_change_password: bool,
) -> anyhow::Result<String> {
    let id = Uuid::new_v4().to_string();
    let role_str = match role {
        UserRole::Admin => "admin",
        UserRole::User => "user",
    };
    let now = Utc::now().to_rfc3339();
    let mcp: i64 = if must_change_password { 1 } else { 0 };

    sqlx::query(
        "INSERT INTO users (id, username, password_hash, role, created_at, must_change_password) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(username)
    .bind(password_hash)
    .bind(role_str)
    .bind(&now)
    .bind(mcp)
    .execute(db)
    .await?;

    Ok(id)
}

/// Update a user's password hash and clear the must_change_password flag.
pub async fn update_password(db: &Db, user_id: &str, new_hash: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET password_hash = ?, must_change_password = 0 WHERE id = ?")
        .bind(new_hash)
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(())
}

/// Set the must_change_password flag for a user (admin reset).
pub async fn set_must_change_password(db: &Db, user_id: &str, value: bool) -> anyhow::Result<()> {
    let v: i64 = if value { 1 } else { 0 };
    sqlx::query("UPDATE users SET must_change_password = ? WHERE id = ?")
        .bind(v)
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn delete_user(db: &Db, user_id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM user_providers WHERE user_id = ?")
        .bind(user_id)
        .execute(db)
        .await?;
    sqlx::query("DELETE FROM provider_sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(db)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(())
}

// ── User → provider access ─────────────────────────────────────────────────

/// Returns the set of provider IDs the user may access.
/// An empty list means the user has access to *all* providers.
pub async fn get_user_providers(db: &Db, user_id: &str) -> anyhow::Result<Vec<String>> {
    let rows = sqlx::query("SELECT provider_id FROM user_providers WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(db)
        .await?;
    Ok(rows.into_iter().map(|r| r.get("provider_id")).collect())
}

/// Replace the entire provider allow-list for a user.
/// Pass an empty slice to grant access to all providers.
pub async fn set_user_providers(
    db: &Db,
    user_id: &str,
    provider_ids: &[String],
) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM user_providers WHERE user_id = ?")
        .bind(user_id)
        .execute(db)
        .await?;

    for pid in provider_ids {
        sqlx::query("INSERT INTO user_providers (user_id, provider_id) VALUES (?, ?)")
            .bind(user_id)
            .bind(pid)
            .execute(db)
            .await?;
    }
    Ok(())
}

// ── Provider sessions ──────────────────────────────────────────────────────

/// Row from the `provider_sessions` table.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ProviderSessionRow {
    pub id: String,
    pub user_id: String,
    pub provider_id: String,
    /// `stored_values` serialised as a JSON object string.
    pub stored_values: String,
    pub updated_at: String,
}

/// For `global`-scoped providers the `user_id` is the empty string `""` so
/// a single shared session is used regardless of which OTVI user made the request.
#[allow(dead_code)]
pub async fn get_provider_session(
    db: &Db,
    user_id: &str,
    provider_id: &str,
) -> anyhow::Result<Option<ProviderSessionRow>> {
    let row = sqlx::query(
        "SELECT id, user_id, provider_id, stored_values, updated_at \
         FROM provider_sessions WHERE user_id = ? AND provider_id = ?",
    )
    .bind(user_id)
    .bind(provider_id)
    .fetch_optional(db)
    .await?;

    Ok(row.map(|r| ProviderSessionRow {
        id: r.get("id"),
        user_id: r.get("user_id"),
        provider_id: r.get("provider_id"),
        stored_values: r.get("stored_values"),
        updated_at: r.get("updated_at"),
    }))
}

pub async fn upsert_provider_session(
    db: &Db,
    user_id: &str,
    provider_id: &str,
    stored_values: &HashMap<String, String>,
) -> anyhow::Result<String> {
    let now = Utc::now().to_rfc3339();
    let json = serde_json::to_string(stored_values)?;

    // Check for existing session
    let existing =
        sqlx::query("SELECT id FROM provider_sessions WHERE user_id = ? AND provider_id = ?")
            .bind(user_id)
            .bind(provider_id)
            .fetch_optional(db)
            .await?;

    if let Some(row) = existing {
        let id: String = row.get("id");
        sqlx::query("UPDATE provider_sessions SET stored_values = ?, updated_at = ? WHERE id = ?")
            .bind(&json)
            .bind(&now)
            .bind(&id)
            .execute(db)
            .await?;
        Ok(id)
    } else {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO provider_sessions \
             (id, user_id, provider_id, stored_values, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(user_id)
        .bind(provider_id)
        .bind(&json)
        .bind(&now)
        .bind(&now)
        .execute(db)
        .await?;
        Ok(id)
    }
}

pub async fn delete_provider_session(
    db: &Db,
    user_id: &str,
    provider_id: &str,
) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM provider_sessions WHERE user_id = ? AND provider_id = ?")
        .bind(user_id)
        .bind(provider_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn get_provider_session_values(
    db: &Db,
    user_id: &str,
    provider_id: &str,
) -> anyhow::Result<HashMap<String, String>> {
    let row = sqlx::query(
        "SELECT stored_values FROM provider_sessions WHERE user_id = ? AND provider_id = ?",
    )
    .bind(user_id)
    .bind(provider_id)
    .fetch_optional(db)
    .await?;

    match row {
        None => Ok(HashMap::new()),
        Some(r) => {
            let json: String = r.get("stored_values");
            Ok(serde_json::from_str(&json).unwrap_or_default())
        }
    }
}

// ── Server settings ────────────────────────────────────────────────────────

pub async fn get_setting(db: &Db, key: &str) -> anyhow::Result<Option<String>> {
    let row = sqlx::query("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(db)
        .await?;
    Ok(row.map(|r| r.get("value")))
}

pub async fn set_setting(db: &Db, key: &str, value: &str) -> anyhow::Result<()> {
    // Portable upsert: delete + insert works in SQLite, PostgreSQL, MySQL.
    sqlx::query("DELETE FROM settings WHERE key = ?")
        .bind(key)
        .execute(db)
        .await?;
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?)")
        .bind(key)
        .bind(value)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn is_signup_disabled(db: &Db) -> anyhow::Result<bool> {
    Ok(get_setting(db, "signup_disabled")
        .await?
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false))
}

pub async fn set_signup_disabled(db: &Db, disabled: bool) -> anyhow::Result<()> {
    set_setting(db, "signup_disabled", if disabled { "1" } else { "0" }).await
}
