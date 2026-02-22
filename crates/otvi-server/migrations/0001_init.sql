-- Initial schema compatible with SQLite, PostgreSQL, and MySQL.
-- All PKs are UUID strings stored as TEXT to avoid dialect differences
-- with SERIAL / AUTOINCREMENT / AUTO_INCREMENT.
--
-- Booleans are stored as INTEGER (0 = false, 1 = true) because SQLite
-- has no native BOOLEAN type; PostgreSQL and MySQL accept INTEGER fine.
--
-- Timestamps are stored as TEXT (ISO-8601) for the same reason.

-- ── OTVI application users ────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS users (
    id           TEXT NOT NULL PRIMARY KEY,
    username     TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role         TEXT NOT NULL DEFAULT 'user',   -- 'admin' | 'user'
    created_at   TEXT NOT NULL,
    CONSTRAINT uq_users_username UNIQUE (username)
);

-- ── Provider access list per user ─────────────────────────────────────────
-- Empty = access to ALL providers; rows = explicit allow-list.
CREATE TABLE IF NOT EXISTS user_providers (
    user_id     TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    PRIMARY KEY (user_id, provider_id)
);

-- ── Server-wide settings (key / value) ───────────────────────────────────
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);

-- Seed: signup is enabled by default.
INSERT INTO settings (key, value)
    SELECT 'signup_disabled', '0'
    WHERE NOT EXISTS (SELECT 1 FROM settings WHERE key = 'signup_disabled');

-- ── Provider sessions (replaces the old sessions.json) ───────────────────
-- For global-scoped providers user_id = '' (empty) so the row is shared.
CREATE TABLE IF NOT EXISTS provider_sessions (
    id             TEXT NOT NULL PRIMARY KEY,
    user_id        TEXT NOT NULL,
    provider_id    TEXT NOT NULL,
    stored_values  TEXT NOT NULL DEFAULT '{}',   -- JSON object
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    CONSTRAINT uq_prov_session UNIQUE (user_id, provider_id)
);
