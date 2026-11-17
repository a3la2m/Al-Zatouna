-- Phase 20: Telegram notification + query bot.
-- Generic key/value settings for bot config (token never leaves the DB).
CREATE TABLE IF NOT EXISTS telegram_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL DEFAULT ''
);

-- A Telegram chat linked to a person (student / teacher / manager).
CREATE TABLE IF NOT EXISTS telegram_chats (
    id          INTEGER PRIMARY KEY,
    chat_id     TEXT NOT NULL UNIQUE,
    person_type TEXT NOT NULL CHECK (person_type IN ('student','teacher','manager')),
    person_id   INTEGER NOT NULL,
    phone       TEXT NOT NULL DEFAULT '',
    linked_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    active      INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0,1)),
    UNIQUE (person_type, person_id)
);
CREATE INDEX IF NOT EXISTS idx_tg_chats_phone ON telegram_chats (phone);

-- Full message history (in/out) — mirrors the other project's chat log.
CREATE TABLE IF NOT EXISTS telegram_messages (
    id          INTEGER PRIMARY KEY,
    chat_id     TEXT NOT NULL,
    person_type TEXT NOT NULL DEFAULT '',
    person_id   INTEGER,
    direction   TEXT NOT NULL CHECK (direction IN ('in','out')),
    body        TEXT NOT NULL,
    kind        TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_tg_msg_chat ON telegram_messages (chat_id, id DESC);
CREATE INDEX IF NOT EXISTS idx_tg_msg_kind ON telegram_messages (kind, id DESC);

-- Durable log of notifications sent (or attempted), with delivery status.
CREATE TABLE IF NOT EXISTS telegram_notifications (
    id          INTEGER PRIMARY KEY,
    chat_id     TEXT NOT NULL,
    person_type TEXT NOT NULL DEFAULT '',
    person_id   INTEGER,
    kind        TEXT NOT NULL,
    title       TEXT NOT NULL DEFAULT '',
    body        TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'sent' CHECK (status IN ('sent','failed','skipped')),
    error       TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_tg_notif_kind ON telegram_notifications (kind, id DESC);

-- Multi-step interactive flows (e.g. /recover, /register) per chat.
CREATE TABLE IF NOT EXISTS telegram_flow_state (
    chat_id     TEXT PRIMARY KEY,
    state_json  TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Seed a notifications.manage permission and grant to admins/managers.
INSERT OR IGNORE INTO permissions (code, name, grp) VALUES ('notifications.manage','إدارة الإشعارات','admin');
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
    SELECT r.id, p.id FROM roles r, permissions p
    WHERE r.code IN ('system_admin','center_manager') AND p.code='notifications.manage';
