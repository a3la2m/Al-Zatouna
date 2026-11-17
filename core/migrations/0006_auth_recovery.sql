-- Phase 19: persistent ("remember me") sessions + offline password recovery codes.

-- Persistent session tokens survive an app restart and have a fixed, long
-- expiry (unlike the in-memory 20-minute sliding sessions).
CREATE TABLE IF NOT EXISTS auth_sessions (
    token          TEXT PRIMARY KEY,
    user_id        INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    last_activity  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    expires_at     TEXT NOT NULL,
    remember       INTEGER NOT NULL DEFAULT 1 CHECK (remember IN (0,1))
);
CREATE INDEX IF NOT EXISTS idx_auth_sessions_user ON auth_sessions (user_id);

-- One-time recovery codes. Only SHA-256 hashes are stored (never the plain
-- code). A code is consumed by setting used_at.
CREATE TABLE IF NOT EXISTS password_recovery_codes (
    id          INTEGER PRIMARY KEY,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash   TEXT NOT NULL,
    label       TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    used_at     TEXT
);
CREATE INDEX IF NOT EXISTS idx_recovery_codes_user ON password_recovery_codes (user_id);
