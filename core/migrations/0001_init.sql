-- =============================================================
-- Education Center Management System — migration 0001 (initial)
-- All money columns: integer minor units (no REAL/FLOAT, test db009).
-- All timestamps: ISO-8601 UTC text. Business dates: YYYY-MM-DD.
-- =============================================================
PRAGMA foreign_keys = ON;

-- ---------- meta ----------
CREATE TABLE db_meta (
    id                    INTEGER PRIMARY KEY CHECK (id = 1),
    schema_version        INTEGER NOT NULL,
    applied_at            TEXT    NOT NULL,
    app_version_at_apply  TEXT    NOT NULL DEFAULT ''
);

-- ---------- access ----------
CREATE TABLE branches (
    id        INTEGER PRIMARY KEY,
    code      TEXT    NOT NULL UNIQUE,
    name      TEXT    NOT NULL,
    address   TEXT    NOT NULL DEFAULT '',
    phone     TEXT    NOT NULL DEFAULT '',
    is_main   INTEGER NOT NULL DEFAULT 0 CHECK (is_main IN (0, 1)),
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    CHECK (is_main = 0 OR is_active = 1)  -- the main branch must stay active
);

CREATE TABLE users (
    id                   INTEGER PRIMARY KEY,
    username             TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    full_name            TEXT    NOT NULL,
    password_hash        TEXT    NOT NULL,          -- Argon2id
    is_active            INTEGER NOT NULL DEFAULT 1,
    must_change_password INTEGER NOT NULL DEFAULT 1,
    failed_attempts      INTEGER NOT NULL DEFAULT 0,
    locked_until         TEXT,
    last_login_at        TEXT,
    last_login_ip        TEXT,
    created_at           TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at           TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE roles (
    id        INTEGER PRIMARY KEY,
    code      TEXT    NOT NULL UNIQUE,
    name      TEXT    NOT NULL,
    is_system INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE permissions (
    id    INTEGER PRIMARY KEY,
    code  TEXT NOT NULL UNIQUE,
    name  TEXT NOT NULL,
    grp   TEXT NOT NULL
);

CREATE TABLE role_permissions (
    role_id       INTEGER NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_id INTEGER NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);

CREATE TABLE user_roles (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id INTEGER NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, role_id)
);

CREATE TABLE user_permission_overrides (
    id            INTEGER PRIMARY KEY,
    user_id       INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    permission_id INTEGER NOT NULL REFERENCES permissions(id) ON DELETE RESTRICT,
    allowed       INTEGER NOT NULL CHECK (allowed IN (0, 1)),
    reason        TEXT    NOT NULL DEFAULT '',
    granted_by    INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (user_id, permission_id)
);

CREATE TABLE audit_log (
    id          INTEGER PRIMARY KEY,
    logged_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    user_id     INTEGER REFERENCES users(id) ON DELETE SET NULL,
    action      TEXT    NOT NULL,
    entity_type TEXT    NOT NULL DEFAULT '',
    entity_id   INTEGER,
    old_json    TEXT,
    new_json    TEXT,
    reason      TEXT    NOT NULL DEFAULT '',
    ctx_json    TEXT
);
CREATE INDEX idx_audit_logged_at ON audit_log (logged_at);
CREATE INDEX idx_audit_user      ON audit_log (user_id);
CREATE INDEX idx_audit_entity    ON audit_log (entity_type, entity_id);

CREATE TRIGGER trg_audit_no_update BEFORE UPDATE ON audit_log
BEGIN SELECT RAISE(ABORT, 'audit log is append-only'); END;
CREATE TRIGGER trg_audit_no_delete BEFORE DELETE ON audit_log
BEGIN SELECT RAISE(ABORT, 'audit log is append-only'); END;

-- ---------- center ----------
CREATE TABLE center_settings (
    id                INTEGER PRIMARY KEY CHECK (id = 1),
    name              TEXT    NOT NULL,
    trade_name        TEXT    NOT NULL DEFAULT '',
    logo_ref          TEXT    NOT NULL DEFAULT '',
    address           TEXT    NOT NULL DEFAULT '',
    phones            TEXT    NOT NULL DEFAULT '',
    email             TEXT    NOT NULL DEFAULT '',
    website           TEXT    NOT NULL DEFAULT '',
    license_no        TEXT    NOT NULL DEFAULT '',
    tax_no            TEXT    NOT NULL DEFAULT '',
    currency_code     TEXT    NOT NULL DEFAULT 'ILS' CHECK (length(currency_code) = 3),
    currency_decimals INTEGER NOT NULL DEFAULT 2 CHECK (currency_decimals BETWEEN 0 AND 8),
    timezone          TEXT    NOT NULL DEFAULT 'Asia/Hebron',
    academic_year     TEXT    NOT NULL DEFAULT '',
    header_json       TEXT    NOT NULL DEFAULT '{}',
    footer_json       TEXT    NOT NULL DEFAULT '{}',
    cash_rules_json   TEXT    NOT NULL DEFAULT '{}',
    created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- ---------- students & guardians ----------
CREATE TABLE students (
    id            INTEGER PRIMARY KEY,
    branch_id     INTEGER NOT NULL REFERENCES branches(id) ON DELETE RESTRICT,
    student_no    INTEGER NOT NULL,
    full_name     TEXT    NOT NULL,
    national_id   TEXT    NOT NULL DEFAULT '',
    birth_date    TEXT    NOT NULL DEFAULT '',
    gender        TEXT    CHECK (gender IN ('M','F')),
    phone         TEXT    NOT NULL DEFAULT '',
    address       TEXT    NOT NULL DEFAULT '',
    email         TEXT    NOT NULL DEFAULT '',
    photo_ref     TEXT    NOT NULL DEFAULT '',
    notes         TEXT    NOT NULL DEFAULT '',
    status        TEXT    NOT NULL DEFAULT 'active'
                  CHECK (status IN ('active','frozen','archived')),
    registered_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    created_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (branch_id, student_no)
);
CREATE UNIQUE INDEX uniq_students_national
    ON students (national_id) WHERE national_id <> '';
CREATE INDEX idx_students_branch_status ON students (branch_id, status);
CREATE INDEX idx_students_name  ON students (full_name COLLATE NOCASE);
CREATE INDEX idx_students_phone ON students (phone);

CREATE TABLE guardians (
    id          INTEGER PRIMARY KEY,
    full_name   TEXT NOT NULL,
    relation    TEXT NOT NULL DEFAULT '',
    phone       TEXT NOT NULL DEFAULT '',
    email       TEXT NOT NULL DEFAULT '',
    national_id TEXT NOT NULL DEFAULT '',
    address     TEXT NOT NULL DEFAULT '',
    notes       TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE student_guardians (
    student_id  INTEGER NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    guardian_id INTEGER NOT NULL REFERENCES guardians(id) ON DELETE RESTRICT,
    is_primary  INTEGER NOT NULL DEFAULT 0 CHECK (is_primary IN (0, 1)),
    PRIMARY KEY (student_id, guardian_id)
);

-- ---------- teachers ----------
CREATE TABLE teachers (
    id             INTEGER PRIMARY KEY,
    branch_id      INTEGER NOT NULL REFERENCES branches(id) ON DELETE RESTRICT,
    teacher_no     INTEGER NOT NULL,
    full_name      TEXT    NOT NULL,
    phone          TEXT    NOT NULL DEFAULT '',
    email          TEXT    NOT NULL DEFAULT '',
    specialty      TEXT    NOT NULL DEFAULT '',
    qualifications TEXT    NOT NULL DEFAULT '',
    address        TEXT    NOT NULL DEFAULT '',
    hire_date      TEXT    NOT NULL DEFAULT '',
    status         TEXT    NOT NULL DEFAULT 'active'
                   CHECK (status IN ('active','inactive','archived')),
    notes          TEXT    NOT NULL DEFAULT '',
    created_at     TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at     TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (branch_id, teacher_no)
);
CREATE INDEX idx_teachers_name ON teachers (full_name COLLATE NOCASE);

-- ---------- catalog ----------
CREATE TABLE subjects (
    id          INTEGER PRIMARY KEY,
    code        TEXT    NOT NULL UNIQUE,
    name        TEXT    NOT NULL,
    description TEXT    NOT NULL DEFAULT '',
    is_active   INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE rooms (
    id          INTEGER PRIMARY KEY,
    branch_id   INTEGER NOT NULL REFERENCES branches(id) ON DELETE RESTRICT,
    code        TEXT    NOT NULL,
    name        TEXT    NOT NULL,
    capacity    INTEGER NOT NULL DEFAULT 0,
    description TEXT    NOT NULL DEFAULT '',
    status      TEXT    NOT NULL DEFAULT 'available'
                CHECK (status IN ('available','reserved','maintenance','archived')),
    notes       TEXT    NOT NULL DEFAULT '',
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (branch_id, code)
);

-- ---------- courses ----------
CREATE TABLE courses (
    id                INTEGER PRIMARY KEY,
    branch_id         INTEGER NOT NULL REFERENCES branches(id) ON DELETE RESTRICT,
    course_no         INTEGER NOT NULL,
    name              TEXT    NOT NULL,
    subject_id        INTEGER NOT NULL REFERENCES subjects(id) ON DELETE RESTRICT,
    description       TEXT    NOT NULL DEFAULT '',
    start_date        TEXT    NOT NULL DEFAULT '',
    end_date          TEXT    NOT NULL DEFAULT '',
    default_time_json TEXT    NOT NULL DEFAULT '{}',
    teacher_id        INTEGER NOT NULL REFERENCES teachers(id) ON DELETE RESTRICT,
    room_id           INTEGER NOT NULL REFERENCES rooms(id) ON DELETE RESTRICT,
    expected_students INTEGER NOT NULL DEFAULT 0,
    min_students      INTEGER NOT NULL DEFAULT 0,
    max_students      INTEGER NOT NULL DEFAULT 0,
    fee_minor         INTEGER NOT NULL DEFAULT 0 CHECK (fee_minor >= 0),
    status            TEXT    NOT NULL DEFAULT 'draft'
                      CHECK (status IN ('draft','registration_open','active','completed','suspended','cancelled','archived')),
    is_private        INTEGER NOT NULL DEFAULT 0 CHECK (is_private IN (0, 1)),
    created_by        INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    approved_by       INTEGER REFERENCES users(id) ON DELETE SET NULL,
    approved_at       TEXT,
    notes             TEXT    NOT NULL DEFAULT '',
    created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (branch_id, course_no)
);
CREATE INDEX idx_courses_subject ON courses (subject_id);
CREATE INDEX idx_courses_teacher ON courses (teacher_id);
CREATE INDEX idx_courses_room    ON courses (room_id);
CREATE INDEX idx_courses_status  ON courses (branch_id, status);

CREATE TABLE course_schedules (
    id             INTEGER PRIMARY KEY,
    course_id      INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    weekday        INTEGER NOT NULL CHECK (weekday BETWEEN 1 AND 7),
    start_time     TEXT    NOT NULL,
    end_time       TEXT    NOT NULL,
    room_id        INTEGER NOT NULL REFERENCES rooms(id) ON DELETE RESTRICT,
    teacher_id     INTEGER NOT NULL REFERENCES teachers(id) ON DELETE RESTRICT,
    effective_from TEXT    NOT NULL DEFAULT '',
    effective_to   TEXT    NOT NULL DEFAULT '',
    CHECK (start_time < end_time),
    CHECK (effective_to = '' OR effective_to >= effective_from)
);
CREATE INDEX idx_schedules_course ON course_schedules (course_id);

CREATE TABLE course_change_log (
    id          INTEGER PRIMARY KEY,
    course_id   INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    changed_by  INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    changed_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    action      TEXT    NOT NULL,
    before_json TEXT    NOT NULL DEFAULT '{}',
    after_json  TEXT    NOT NULL DEFAULT '{}',
    reason      TEXT    NOT NULL DEFAULT ''
);
CREATE INDEX idx_course_changes ON course_change_log (course_id, changed_at);

-- ---------- scheduling ----------
CREATE TABLE sessions (
    id                    INTEGER PRIMARY KEY,
    course_id             INTEGER NOT NULL REFERENCES courses(id) ON DELETE RESTRICT,
    source_schedule_id    INTEGER REFERENCES course_schedules(id) ON DELETE SET NULL,
    seq_in_course         INTEGER NOT NULL,
    session_date          TEXT    NOT NULL,
    start_time            TEXT    NOT NULL,
    end_time              TEXT    NOT NULL,
    room_id               INTEGER NOT NULL REFERENCES rooms(id) ON DELETE RESTRICT,
    teacher_id            INTEGER NOT NULL REFERENCES teachers(id) ON DELETE RESTRICT,
    substitute_teacher_id INTEGER REFERENCES teachers(id) ON DELETE SET NULL,
    status                TEXT    NOT NULL DEFAULT 'scheduled'
                          CHECK (status IN ('scheduled','executed','cancelled','postponed','makeup')),
    session_kind          TEXT    NOT NULL DEFAULT 'regular'
                          CHECK (session_kind IN ('regular','makeup','compensating')),
    original_session_id   INTEGER REFERENCES sessions(id) ON DELETE SET NULL,
    reschedule_to_id      INTEGER REFERENCES sessions(id) ON DELETE SET NULL,
    reason                TEXT    NOT NULL DEFAULT '',
    created_by            INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    executed_at           TEXT,
    cancelled_by          INTEGER REFERENCES users(id) ON DELETE SET NULL,
    created_at            TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at            TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (course_id, seq_in_course),
    CHECK (end_time > start_time)
);
CREATE INDEX idx_sessions_course_date ON sessions (course_id, session_date, status);
CREATE INDEX idx_sessions_room        ON sessions (room_id, session_date, start_time);
CREATE INDEX idx_sessions_teacher     ON sessions (teacher_id, session_date, start_time);

CREATE TABLE student_attendance (
    id           INTEGER PRIMARY KEY,
    session_id   INTEGER NOT NULL REFERENCES sessions(id) ON DELETE RESTRICT,
    student_id   INTEGER NOT NULL REFERENCES students(id) ON DELETE RESTRICT,
    status       TEXT    NOT NULL CHECK (status IN ('present','absent','late','excused')),
    late_minutes INTEGER NOT NULL DEFAULT 0 CHECK (late_minutes >= 0),
    notes        TEXT    NOT NULL DEFAULT '',
    recorded_by  INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    recorded_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (session_id, student_id)
);
CREATE INDEX idx_attendance_student ON student_attendance (student_id, session_id);

CREATE TABLE teacher_attendance (
    id          INTEGER PRIMARY KEY,
    session_id  INTEGER NOT NULL REFERENCES sessions(id) ON DELETE RESTRICT,
    teacher_id  INTEGER NOT NULL REFERENCES teachers(id) ON DELETE RESTRICT,
    status      TEXT    NOT NULL CHECK (status IN ('present','absent','late','excused')),
    notes       TEXT    NOT NULL DEFAULT '',
    recorded_by INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    recorded_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (session_id)
);

-- ---------- enrollment ----------
CREATE TABLE enrollments (
    id                   INTEGER PRIMARY KEY,
    branch_id            INTEGER NOT NULL REFERENCES branches(id) ON DELETE RESTRICT,
    student_id           INTEGER NOT NULL REFERENCES students(id) ON DELETE RESTRICT,
    course_id            INTEGER NOT NULL REFERENCES courses(id) ON DELETE RESTRICT,
    enrolled_date        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d','now')),
    base_fee_minor       INTEGER NOT NULL DEFAULT 0 CHECK (base_fee_minor >= 0),
    discount_minor       INTEGER NOT NULL DEFAULT 0 CHECK (discount_minor >= 0),
    waiver_minor         INTEGER NOT NULL DEFAULT 0 CHECK (waiver_minor >= 0),
    credit_applied_minor INTEGER NOT NULL DEFAULT 0 CHECK (credit_applied_minor >= 0),
    net_fee_minor        INTEGER NOT NULL DEFAULT 0 CHECK (net_fee_minor >= 0),
    status               TEXT    NOT NULL DEFAULT 'active'
                         CHECK (status IN ('active','completed','withdrawn','frozen','transferred','cancelled')),
    notes                TEXT    NOT NULL DEFAULT '',
    frozen_reason        TEXT    NOT NULL DEFAULT '',
    transferred_to_enrollment_id INTEGER REFERENCES enrollments(id) ON DELETE SET NULL,
    created_by           INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at           TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at           TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (student_id, course_id),
    CHECK (net_fee_minor = base_fee_minor - discount_minor - waiver_minor)
);
CREATE INDEX idx_enrollments_student ON enrollments (student_id, status);
CREATE INDEX idx_enrollments_course  ON enrollments (course_id, status);

CREATE TABLE payment_plans (
    id                INTEGER PRIMARY KEY,
    enrollment_id     INTEGER NOT NULL UNIQUE REFERENCES enrollments(id) ON DELETE CASCADE,
    total_minor       INTEGER NOT NULL CHECK (total_minor >= 0),
    installment_count INTEGER NOT NULL CHECK (installment_count >= 1),
    policy_json       TEXT    NOT NULL DEFAULT '{}'
);

CREATE TABLE installments (
    id              INTEGER PRIMARY KEY,
    payment_plan_id INTEGER NOT NULL REFERENCES payment_plans(id) ON DELETE CASCADE,
    seq             INTEGER NOT NULL,
    due_date        TEXT    NOT NULL,
    amount_minor    INTEGER NOT NULL CHECK (amount_minor >= 0),
    status          TEXT    NOT NULL DEFAULT 'open'
                    CHECK (status IN ('open','partially_paid','paid')),
    UNIQUE (payment_plan_id, seq)
);
CREATE INDEX idx_installments_due ON installments (due_date, status);

-- ---------- financial agreements ----------
CREATE TABLE financial_agreements (
    id                INTEGER PRIMARY KEY,
    course_id         INTEGER NOT NULL REFERENCES courses(id) ON DELETE RESTRICT,
    version           INTEGER NOT NULL,
    status            TEXT    NOT NULL DEFAULT 'draft'
                      CHECK (status IN ('draft','active','superseded')),
    method            TEXT    NOT NULL
                      CHECK (method IN ('percent_of_agreed','percent_of_collected','percent_of_executed_sessions','flat_course','flat_session','flat_plus_percent')),
    rate_bp           INTEGER CHECK (rate_bp IS NULL OR (rate_bp >= 0 AND rate_bp <= 10000)),
    center_rate_bp    INTEGER CHECK (center_rate_bp IS NULL OR (center_rate_bp >= 0 AND center_rate_bp <= 10000)),
    flat_minor        INTEGER CHECK (flat_minor IS NULL OR flat_minor >= 0),
    per_session_minor INTEGER CHECK (per_session_minor IS NULL OR per_session_minor >= 0),
    due_timing_json   TEXT    NOT NULL DEFAULT '{}',
    conditions_json   TEXT    NOT NULL DEFAULT '{}',
    snapshot_json     TEXT    NOT NULL DEFAULT '{}',
    approved_by       INTEGER REFERENCES users(id) ON DELETE SET NULL,
    approved_at       TEXT,
    created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (course_id, version)
);

-- the frozen snapshot of an active/superseded agreement never changes (FR-5.5)
CREATE TRIGGER trg_agreement_snapshot_frozen
BEFORE UPDATE OF snapshot_json, method, rate_bp, center_rate_bp,
                 flat_minor, per_session_minor, conditions_json, due_timing_json
ON financial_agreements
WHEN OLD.status IN ('active','superseded')
BEGIN SELECT RAISE(ABORT, 'approved agreement terms are immutable; create an amendment'); END;

CREATE TABLE agreement_amendments (
    id           INTEGER PRIMARY KEY,
    agreement_id INTEGER NOT NULL REFERENCES financial_agreements(id) ON DELETE RESTRICT,
    old_version  INTEGER NOT NULL,
    new_version  INTEGER NOT NULL,
    changes_json TEXT    NOT NULL DEFAULT '{}',
    reason       TEXT    NOT NULL,
    created_by   INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    approved_by  INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    approved_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    created_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_amendments_agreement ON agreement_amendments (agreement_id);

CREATE TABLE student_credit_ledger (
    id           INTEGER PRIMARY KEY,
    student_id   INTEGER NOT NULL REFERENCES students(id) ON DELETE RESTRICT,
    ref_type     TEXT    NOT NULL CHECK (ref_type IN ('receipt','refund','settlement','transfer','adjustment')),
    ref_id       INTEGER NOT NULL,
    credit_minor INTEGER NOT NULL,
    occurred_at  TEXT    NOT NULL,
    note         TEXT    NOT NULL DEFAULT '',
    created_by   INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT
);
CREATE INDEX idx_student_credit ON student_credit_ledger (student_id, occurred_at);

CREATE TABLE shortfall_decisions (
    id                  INTEGER PRIMARY KEY,
    enrollment_id       INTEGER NOT NULL REFERENCES enrollments(id) ON DELETE RESTRICT,
    student_id          INTEGER NOT NULL REFERENCES students(id) ON DELETE RESTRICT,
    course_id           INTEGER NOT NULL REFERENCES courses(id) ON DELETE RESTRICT,
    uncollected_minor   INTEGER NOT NULL CHECK (uncollected_minor > 0),
    reason              TEXT    NOT NULL DEFAULT '',
    decision            TEXT    NOT NULL
                        CHECK (decision IN ('center_full','teacher_full','partial_split','fixed_center','fixed_teacher','deferred','documented_exemption','exceptional')),
    center_share_minor  INTEGER NOT NULL DEFAULT 0 CHECK (center_share_minor >= 0),
    teacher_share_minor INTEGER NOT NULL DEFAULT 0 CHECK (teacher_share_minor >= 0),
    proposed_by         INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    proposed_at         TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    approved_by         INTEGER REFERENCES users(id) ON DELETE SET NULL,
    approved_at         TEXT,
    status              TEXT    NOT NULL DEFAULT 'pending'
                        CHECK (status IN ('pending','decided','deferred','rejected')),
    notes               TEXT    NOT NULL DEFAULT '',
    created_at          TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at          TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_shortfall_course  ON shortfall_decisions (course_id, status);
CREATE INDEX idx_shortfall_student ON shortfall_decisions (student_id);

CREATE TABLE refund_requests (
    id                   INTEGER PRIMARY KEY,
    enrollment_id        INTEGER NOT NULL REFERENCES enrollments(id) ON DELETE RESTRICT,
    student_id           INTEGER NOT NULL REFERENCES students(id) ON DELETE RESTRICT,
    course_id            INTEGER NOT NULL REFERENCES courses(id) ON DELETE RESTRICT,
    requested_date       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d','now')),
    requested_by         INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    status               TEXT    NOT NULL DEFAULT 'submitted'
                         CHECK (status IN ('draft','submitted','approved','rejected','paid','cancelled')),
    policy_snapshot_json TEXT    NOT NULL DEFAULT '{}',
    refundable_minor     INTEGER NOT NULL DEFAULT 0 CHECK (refundable_minor >= 0),
    breakdown_json       TEXT    NOT NULL DEFAULT '{}',
    approver             INTEGER REFERENCES users(id) ON DELETE SET NULL,
    approved_at          TEXT,
    voucher_id           INTEGER,
    rejected_reason      TEXT    NOT NULL DEFAULT '',
    created_at           TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at           TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_refunds_status     ON refund_requests (status);
CREATE INDEX idx_refunds_enrollment ON refund_requests (enrollment_id);

CREATE TABLE settlements (
    id                   INTEGER PRIMARY KEY,
    course_id            INTEGER REFERENCES courses(id) ON DELETE RESTRICT,
    teacher_id           INTEGER REFERENCES teachers(id) ON DELETE RESTRICT,
    kind                 TEXT NOT NULL
                         CHECK (kind IN ('course','teacher','center','shortfall','refunds','partial','final')),
    period_from          TEXT NOT NULL,
    period_to            TEXT NOT NULL,
    total_minor          INTEGER NOT NULL DEFAULT 0 CHECK (total_minor >= 0),
    engine_snapshot_json TEXT NOT NULL DEFAULT '{}',
    status               TEXT NOT NULL DEFAULT 'pending_approval'
                         CHECK (status IN ('draft','pending_approval','approved','rejected','reversed')),
    created_by           INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    approved_by          INTEGER REFERENCES users(id) ON DELETE SET NULL,
    approved_at          TEXT,
    reversed_by_id       INTEGER REFERENCES settlements(id) ON DELETE SET NULL,
    notes                TEXT NOT NULL DEFAULT '',
    created_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_settlements_scope ON settlements (course_id, teacher_id, status);

-- ---------- ledger ----------
CREATE TABLE accounts (
    id          INTEGER PRIMARY KEY,
    code        TEXT    NOT NULL UNIQUE,
    name        TEXT    NOT NULL,
    kind        TEXT    NOT NULL CHECK (kind IN ('asset','liability','equity','revenue','expense')),
    parent_id   INTEGER REFERENCES accounts(id) ON DELETE RESTRICT,
    is_postable INTEGER NOT NULL DEFAULT 1 CHECK (is_postable IN (0, 1)),
    is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1))
);

CREATE TABLE journal_entries (
    id                 INTEGER PRIMARY KEY,
    entry_no           INTEGER NOT NULL UNIQUE,
    entry_date         TEXT    NOT NULL,
    memo               TEXT    NOT NULL DEFAULT '',
    currency           TEXT    NOT NULL,
    total_debit_minor  INTEGER NOT NULL CHECK (total_debit_minor >= 0),
    total_credit_minor INTEGER NOT NULL CHECK (total_credit_minor >= 0),
    status             TEXT    NOT NULL DEFAULT 'draft'
                       CHECK (status IN ('draft','posted','reversed')),
    source_type        TEXT    NOT NULL
                       CHECK (source_type IN ('receipt_voucher','payment_voucher','refund','settlement','expense','enroll_recognition','shortfall_decision','cash_closing','adjustment')),
    source_id          INTEGER NOT NULL,
    posted_by          INTEGER REFERENCES users(id) ON DELETE SET NULL,
    posted_at          TEXT,
    reversal_of_id     INTEGER REFERENCES journal_entries(id) ON DELETE SET NULL,
    created_at         TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    CHECK (total_debit_minor = total_credit_minor)
);
CREATE INDEX idx_journal_date     ON journal_entries (entry_date);
CREATE INDEX idx_journal_source   ON journal_entries (source_type, source_id);
CREATE INDEX idx_journal_reversal ON journal_entries (reversal_of_id);

CREATE TABLE journal_lines (
    id           INTEGER PRIMARY KEY,
    entry_id     INTEGER NOT NULL REFERENCES journal_entries(id) ON DELETE RESTRICT,
    account_id   INTEGER NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    debit_minor  INTEGER NOT NULL DEFAULT 0 CHECK (debit_minor >= 0),
    credit_minor INTEGER NOT NULL DEFAULT 0 CHECK (credit_minor >= 0),
    line_memo    TEXT    NOT NULL DEFAULT '',
    -- exactly one non-zero side per line
    CHECK ((debit_minor <> 0 AND credit_minor = 0) OR (credit_minor <> 0 AND debit_minor = 0))
);
CREATE INDEX idx_lines_entry   ON journal_lines (entry_id);
CREATE INDEX idx_lines_account ON journal_lines (account_id);

-- Posted financial records are immutable (ADR-012). Corrections happen via
-- reversal entries only; the only allowed transition is posted -> reversed.
CREATE TRIGGER trg_je_no_delete BEFORE DELETE ON journal_entries
WHEN OLD.status IN ('posted', 'reversed')
BEGIN SELECT RAISE(ABORT, 'posted journal entries are immutable'); END;

CREATE TRIGGER trg_je_draft_status_change BEFORE UPDATE ON journal_entries
WHEN OLD.status = 'draft' AND NEW.status NOT IN ('draft', 'posted')
BEGIN SELECT RAISE(ABORT, 'a draft entry can only move to posted or stay draft'); END;

-- any update of a posted entry is rejected unless it is exactly the
-- posted->reversed transition (memo/amounts/dates stay immutable)
CREATE TRIGGER trg_je_posted_status_change BEFORE UPDATE ON journal_entries
WHEN OLD.status = 'posted'
   AND NOT (NEW.status = 'reversed')
BEGIN SELECT RAISE(ABORT, 'posted entry: only posted->reversed is allowed'); END;

CREATE TRIGGER trg_je_reversed_frozen BEFORE UPDATE ON journal_entries
WHEN OLD.status = 'reversed' AND NEW.status <> 'reversed'
BEGIN SELECT RAISE(ABORT, 'reversed entries stay reversed'); END;

CREATE TRIGGER trg_je_reversal_target BEFORE INSERT ON journal_entries
WHEN NEW.status = 'reversed' AND NEW.reversal_of_id IS NULL
BEGIN SELECT RAISE(ABORT, 'a reversal entry must point to the original entry'); END;

CREATE TRIGGER trg_lines_update_frozen BEFORE UPDATE ON journal_lines
WHEN (SELECT status FROM journal_entries WHERE id = OLD.entry_id) IN ('posted','reversed')
BEGIN SELECT RAISE(ABORT, 'lines of posted/reversed entries are immutable'); END;

CREATE TRIGGER trg_lines_delete_frozen BEFORE DELETE ON journal_lines
WHEN (SELECT status FROM journal_entries WHERE id = OLD.entry_id) IN ('posted','reversed')
BEGIN SELECT RAISE(ABORT, 'lines of posted/reversed entries are immutable'); END;

-- ---------- cash ----------
CREATE TABLE cash_registers (
    id              INTEGER PRIMARY KEY,
    branch_id       INTEGER NOT NULL REFERENCES branches(id) ON DELETE RESTRICT,
    code            TEXT    NOT NULL,
    name            TEXT    NOT NULL,
    status          TEXT    NOT NULL DEFAULT 'active'
                    CHECK (status IN ('active','closed')),
    current_user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    notes           TEXT    NOT NULL DEFAULT '',
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (branch_id, code)
);

CREATE TABLE cash_movements (
    id                  INTEGER PRIMARY KEY,
    register_id         INTEGER NOT NULL REFERENCES cash_registers(id) ON DELETE RESTRICT,
    kind                TEXT    NOT NULL
                        CHECK (kind IN ('opening','receipt','payment','transfer_in','transfer_out','adjustment')),
    direction           TEXT    NOT NULL CHECK (direction IN ('in','out')),
    amount_minor        INTEGER NOT NULL CHECK (amount_minor >= 0),
    ref_type            TEXT    NOT NULL DEFAULT '',
    ref_id              INTEGER,
    occurred_at         TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    balance_after_minor INTEGER NOT NULL CHECK (balance_after_minor >= 0),
    created_by          INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT
);
CREATE INDEX idx_cash_moves_register ON cash_movements (register_id, occurred_at);
CREATE INDEX idx_cash_moves_ref      ON cash_movements (ref_type, ref_id);

CREATE TABLE register_closings (
    id                    INTEGER PRIMARY KEY,
    register_id           INTEGER NOT NULL REFERENCES cash_registers(id) ON DELETE RESTRICT,
    opened_at             TEXT    NOT NULL,
    closed_at             TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    opening_balance_minor INTEGER NOT NULL CHECK (opening_balance_minor >= 0),
    booked_balance_minor  INTEGER NOT NULL CHECK (booked_balance_minor >= 0),
    counted_balance_minor INTEGER NOT NULL CHECK (counted_balance_minor >= 0),
    variance_minor        INTEGER NOT NULL,
    status                TEXT    NOT NULL DEFAULT 'pending'
                          CHECK (status IN ('pending','approved','rejected')),
    prepared_by           INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    approved_by           INTEGER REFERENCES users(id) ON DELETE SET NULL,
    approved_at           TEXT,
    notes                 TEXT    NOT NULL DEFAULT '',
    journal_entry_id      INTEGER REFERENCES journal_entries(id) ON DELETE RESTRICT
);
CREATE INDEX idx_closings_register ON register_closings (register_id, closed_at);

-- ---------- vouchers ----------
CREATE TABLE voucher_sequences (
    id          INTEGER PRIMARY KEY,
    branch_id   INTEGER NOT NULL REFERENCES branches(id) ON DELETE RESTRICT,
    kind        TEXT    NOT NULL
                CHECK (kind IN ('receipt_voucher','payment_voucher','journal_entry','student_no','teacher_no','course_no')),
    fiscal_year INTEGER NOT NULL DEFAULT 0,
    last_no     INTEGER NOT NULL DEFAULT 0 CHECK (last_no >= 0),
    UNIQUE (branch_id, kind, fiscal_year)
);

CREATE TABLE receipt_vouchers (
    id               INTEGER PRIMARY KEY,
    voucher_no       INTEGER NOT NULL UNIQUE,
    branch_id        INTEGER NOT NULL REFERENCES branches(id) ON DELETE RESTRICT,
    voucher_date     TEXT    NOT NULL,
    payer_name       TEXT    NOT NULL,
    student_id       INTEGER REFERENCES students(id) ON DELETE RESTRICT,
    enrollment_id    INTEGER REFERENCES enrollments(id) ON DELETE RESTRICT,
    course_id        INTEGER REFERENCES courses(id) ON DELETE RESTRICT,
    amount_minor     INTEGER NOT NULL CHECK (amount_minor > 0),
    amount_words     TEXT    NOT NULL DEFAULT '',
    method           TEXT    NOT NULL CHECK (method IN ('cash','card','transfer','wallet')),
    account_id       INTEGER NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    cash_register_id INTEGER REFERENCES cash_registers(id) ON DELETE RESTRICT,
    narrative        TEXT    NOT NULL DEFAULT '',
    received_by      INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    status           TEXT    NOT NULL DEFAULT 'draft'
                     CHECK (status IN ('draft','posted','cancelled')),
    posted_by        INTEGER REFERENCES users(id) ON DELETE SET NULL,
    posted_at        TEXT,
    cancel_reason    TEXT    NOT NULL DEFAULT '',
    journal_entry_id INTEGER REFERENCES journal_entries(id) ON DELETE RESTRICT,
    idempotency_key  TEXT    NOT NULL UNIQUE,
    print_count      INTEGER NOT NULL DEFAULT 0 CHECK (print_count >= 0),
    last_printed_at  TEXT,
    created_at       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_receipts_date      ON receipt_vouchers (voucher_date);
CREATE INDEX idx_receipts_course    ON receipt_vouchers (course_id);
CREATE INDEX idx_receipts_enrolment ON receipt_vouchers (enrollment_id);

CREATE TABLE payment_vouchers (
    id                INTEGER PRIMARY KEY,
    voucher_no        INTEGER NOT NULL UNIQUE,
    branch_id         INTEGER NOT NULL REFERENCES branches(id) ON DELETE RESTRICT,
    voucher_date      TEXT    NOT NULL,
    beneficiary_name  TEXT    NOT NULL,
    beneficiary_type  TEXT    NOT NULL
                      CHECK (beneficiary_type IN ('teacher','student','vendor','other')),
    teacher_id        INTEGER REFERENCES teachers(id) ON DELETE RESTRICT,
    student_id        INTEGER REFERENCES students(id) ON DELETE RESTRICT,
    enrollment_ref_id INTEGER REFERENCES enrollments(id) ON DELETE RESTRICT,
    expense_id        INTEGER,
    purpose           TEXT    NOT NULL DEFAULT '',
    amount_minor      INTEGER NOT NULL CHECK (amount_minor > 0),
    amount_words      TEXT    NOT NULL DEFAULT '',
    method            TEXT    NOT NULL CHECK (method IN ('cash','card','transfer','wallet')),
    account_id        INTEGER NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    cash_register_id  INTEGER REFERENCES cash_registers(id) ON DELETE RESTRICT,
    narrative         TEXT    NOT NULL DEFAULT '',
    created_by        INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    status            TEXT    NOT NULL DEFAULT 'draft'
                      CHECK (status IN ('draft','pending_approval','approved','paid','cancelled')),
    approved_by       INTEGER REFERENCES users(id) ON DELETE SET NULL,
    approved_at       TEXT,
    paid_by           INTEGER REFERENCES users(id) ON DELETE SET NULL,
    paid_at           TEXT,
    journal_entry_id  INTEGER REFERENCES journal_entries(id) ON DELETE RESTRICT,
    source_type       TEXT    NOT NULL DEFAULT 'other'
                      CHECK (source_type IN ('refund','teacher_entitlement','expense','settlement','other')),
    source_id         INTEGER,
    idempotency_key   TEXT    NOT NULL UNIQUE,
    print_count       INTEGER NOT NULL DEFAULT 0 CHECK (print_count >= 0),
    created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_payments_date      ON payment_vouchers (voucher_date);
CREATE INDEX idx_payments_teacher   ON payment_vouchers (teacher_id);

-- ---------- expenses ----------
CREATE TABLE expense_categories (
    id        INTEGER PRIMARY KEY,
    code      TEXT    NOT NULL UNIQUE,
    name      TEXT    NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE expenses (
    id            INTEGER PRIMARY KEY,
    branch_id     INTEGER NOT NULL REFERENCES branches(id) ON DELETE RESTRICT,
    category_id   INTEGER NOT NULL REFERENCES expense_categories(id) ON DELETE RESTRICT,
    expense_date  TEXT    NOT NULL,
    amount_minor  INTEGER NOT NULL CHECK (amount_minor > 0),
    beneficiary   TEXT    NOT NULL DEFAULT '',
    description   TEXT    NOT NULL DEFAULT '',
    method        TEXT    NOT NULL CHECK (method IN ('cash','card','transfer','wallet')),
    account_id    INTEGER NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    status        TEXT    NOT NULL DEFAULT 'draft'
                  CHECK (status IN ('draft','pending_approval','approved','paid')),
    voucher_id    INTEGER REFERENCES payment_vouchers(id) ON DELETE RESTRICT,
    created_by    INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    approved_by   INTEGER REFERENCES users(id) ON DELETE SET NULL,
    approved_at   TEXT,
    created_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_expenses_category ON expenses (category_id);
CREATE INDEX idx_expenses_date     ON expenses (expense_date);

-- ---------- platform ----------
CREATE TABLE attachments (
    id           INTEGER PRIMARY KEY,
    entity_type  TEXT    NOT NULL,
    entity_id    INTEGER NOT NULL,
    stored_path  TEXT    NOT NULL,             -- relative to <DATA>
    display_name TEXT    NOT NULL DEFAULT '',
    mime         TEXT    NOT NULL DEFAULT '',
    size_bytes   INTEGER NOT NULL DEFAULT 0 CHECK (size_bytes >= 0),
    sha256       TEXT    NOT NULL DEFAULT '',
    uploaded_by  INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_attachments_entity ON attachments (entity_type, entity_id);

CREATE TABLE print_profiles (
    id            INTEGER PRIMARY KEY,
    code          TEXT    NOT NULL UNIQUE,
    name          TEXT    NOT NULL,
    doc_type      TEXT    NOT NULL,
    paper         TEXT    NOT NULL DEFAULT 'a4',
    orientation   TEXT    NOT NULL DEFAULT 'portrait' CHECK (orientation IN ('portrait','landscape')),
    margins_json  TEXT    NOT NULL DEFAULT '{"mm":{"top":15,"bottom":15,"left":15,"right":15}}',
    font_family   TEXT    NOT NULL DEFAULT 'Cairo',
    font_size_pt  INTEGER NOT NULL DEFAULT 10 CHECK (font_size_pt BETWEEN 7 AND 20),
    show_header   INTEGER NOT NULL DEFAULT 1 CHECK (show_header IN (0, 1)),
    show_footer   INTEGER NOT NULL DEFAULT 1 CHECK (show_footer IN (0, 1)),
    watermark     TEXT    NOT NULL DEFAULT '',
    copies        INTEGER NOT NULL DEFAULT 1 CHECK (copies >= 1),
    is_default    INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1))
);

CREATE TABLE document_print_log (
    id                INTEGER PRIMARY KEY,
    document_type     TEXT    NOT NULL,
    document_id       INTEGER NOT NULL,
    template_code     TEXT    NOT NULL,
    template_version  TEXT    NOT NULL,
    print_profile_id  INTEGER REFERENCES print_profiles(id) ON DELETE SET NULL,
    copies            INTEGER NOT NULL DEFAULT 1 CHECK (copies >= 1),
    printed_by        INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    printed_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    output_path       TEXT    NOT NULL DEFAULT '',
    checksum          TEXT    NOT NULL DEFAULT '',
    purpose           TEXT    NOT NULL CHECK (purpose IN ('print','pdf','reprint'))
);
CREATE INDEX idx_print_log_doc ON document_print_log (document_type, document_id);

CREATE TABLE saved_reports (
    id            INTEGER PRIMARY KEY,
    report_code   TEXT    NOT NULL,
    name          TEXT    NOT NULL,
    owner_id      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    config_json   TEXT    NOT NULL DEFAULT '{}',
    is_shared     INTEGER NOT NULL DEFAULT 0 CHECK (is_shared IN (0, 1)),
    created_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_saved_reports_owner ON saved_reports (owner_id);
