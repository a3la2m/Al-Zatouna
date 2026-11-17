-- Phase 22: in-app notification inbox + Telegram service requests.

CREATE TABLE IF NOT EXISTS app_notifications (
    id          INTEGER PRIMARY KEY,
    kind        TEXT NOT NULL,               -- payment|attendance|session|expense|service_request|system
    title       TEXT NOT NULL,
    body        TEXT NOT NULL,
    ref_type    TEXT NOT NULL DEFAULT '',
    ref_id      INTEGER,
    person_type TEXT NOT NULL DEFAULT '',    -- intended recipient ('' = all managers)
    person_id   INTEGER,
    seen        INTEGER NOT NULL DEFAULT 0 CHECK (seen IN (0,1)),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_app_notif_seen ON app_notifications (seen, id DESC);
CREATE INDEX IF NOT EXISTS idx_app_notif_kind ON app_notifications (kind, id DESC);

CREATE TABLE IF NOT EXISTS service_requests (
    id           INTEGER PRIMARY KEY,
    chat_id      TEXT NOT NULL,
    phone        TEXT NOT NULL DEFAULT '',
    requester    TEXT NOT NULL DEFAULT '',
    service      TEXT NOT NULL DEFAULT '',
    details      TEXT NOT NULL DEFAULT '',
    status       TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','done','rejected')),
    response     TEXT NOT NULL DEFAULT '',
    handled_by   INTEGER,
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    handled_at   TEXT
);
CREATE INDEX IF NOT EXISTS idx_service_req_status ON service_requests (status, id DESC);
