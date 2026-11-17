-- Phase 23: self-service Telegram linking from the «حسابي» screen.
-- 1) Users can store a mobile number on their own profile.
ALTER TABLE users ADD COLUMN phone TEXT NOT NULL DEFAULT '';

-- 2) One-time codes typed into the bot (/link CODE) to attach a chat to a person
--    without relying on a phone match against students/teachers.
CREATE TABLE IF NOT EXISTS telegram_link_codes (
    code        TEXT PRIMARY KEY,
    person_type TEXT NOT NULL CHECK (person_type IN ('student','teacher','manager')),
    person_id   INTEGER NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    expires_at  TEXT NOT NULL,
    used        INTEGER NOT NULL DEFAULT 0 CHECK (used IN (0,1))
);

CREATE INDEX IF NOT EXISTS idx_tg_link_codes_person
    ON telegram_link_codes (person_type, person_id);
