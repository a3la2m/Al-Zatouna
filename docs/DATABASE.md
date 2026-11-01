# DATABASE — SQLite Design & Data Management
## Education Center Management System (Phase 1)

Companion to docs/ERD.md (entity model) and docs/BACKUP_RESTORE.md (files and movement).

---

## 1. File and connect configuration

- Single file: `<data>/database/education_center.db` (plus `-wal` / `-shm`).
- Opened via sqlx `SqlitePoolOptions` with `max_connections=1` for writes and a small read pool; write-serialize by design.
- Pragmas set on every connection at open:
  - `PRAGMA foreign_keys = ON;`
  - `PRAGMA journal_mode = WAL;`
  - `PRAGMA busy_timeout = 5000;`
  - `PRAGMA synchronous = NORMAL;` (WAL-appropriate)
- Recommendations applied at startup after connect: `PRAGMA wal_checkpoint(TRUNCATE)` on graceful close, and `PRAGMA integrity_check` monthly + before/after every restore (with report).
- **Transactions**: every multi-row logical write is wrapped in `BEGIN IMMEDIATE ... COMMIT/ROLLBACK` in the service layer; repos never open their own transactions.
- All statement execution uses prepared statements via sqlx query APIs (no string-concatenated SQL anywhere).

## 2. Data types policy

| Business concept | Storage | Reason |
|------------------|---------|--------|
| Money | INTEGER minor units (`*_minor`) + currency code and decimals in `center_settings` | No float money; conversion/presentation unify in `common::money` |
| Rates/percents | INTEGER basis-points (`*_bp`, 10000 = 100%) | Exactness for teacher splits |
| Instant/timestamp | TEXT ISO-8601 UTC (`YYYY-MM-DDTHH:MM:SS.sssZ`) | Stable, lexicographically sortable |
| Business date (day) | TEXT `YYYY-MM-DD` (center-local date passed by Rust) | Date separated from time cleanly where an operation needs it |
| Local timestamp display | Computed by the app using the center timezone setting, never stored twice | No duplicated truth for timestamps |
| Boolean | INTEGER 0/1 |
| Enums / status | TEXT + CHECK from ERD |
| Structured metadata (policy, pricing rules, snapshots) | TEXT JSON (serde validated) — only inside documents that are themselves immutable (agreement snapshots, print snapshots) or engine-produced provenance records |
| File references | RELATIVE paths under the data directory only; never absolute paths |

## 3. Versioning and migrations

- Migration format: sqlx **numbered SQL files** in `src-tauri/migrations/` (`0001_init.sql`, ...), tracked in `_sqlx_migrations` with checksums — the single source of truth.
- Rules:
  - Migrations are forward-only, append-only, and never edited after release; corrections = new migration.
  - Each migration is a transaction implicitly (multiple `--` statements grouped by sqlx); destructive migrations are explicitly validated with dry-run tests in CI.
  - DB schema version is stamped inside `db_meta` table (id, schema_version, applied_at, app_version_at_apply) for human-readable visibility and for backup/restore checks.
  - Pre-migration automatic backup is taken (Phase 9 integration, always effective in phase 2 for release-level upgrades).
- Seed migration (code M0002 style or stored in a `seed_up` step): chart of accounts, system roles/permissions, default print profiles, default center settings row, default cash register, admin bootstrap workflow (admin password set in first-run wizard — never a default password in the file).

## 4. Immutability & financial guards

Enforced both in Rust (service layer) and by DB triggers as a backstop:

1. `journal_entries`/`journal_lines` of status `posted` cannot be UPDATEd or DELETEd (BEFORE UPDATE/DELETE triggers return abort); reversal entries set a toggle on the header only (`status='reversed'`).
2. `journal_entries.status` may only go draft→posted→reversed; no regression permitted.
3. Rows in `receipt_vouchers`/`payment_vouchers`: status admits only defined  transitions and only by the service; triggers deny transitions raising entry imbalance.
4. `financial_agreements` snapshots: the `snapshot_json` column and any approved agreement header are UPDATE-proof (`source_type='system'` flag is honored on updates).
5. Audit log: normal users have no write path to its tables besides the audit service; triggers prevent aliases WHERE the `app` connection is not the logging service.

## 5. Index set (baseline)

- FK indexes for every FK column that appears in lookups (students.branch, courses.subject/course/teacher/room, attendance.session/student, etc.).
- Status/date composite indexes for list screens: e.g. `sessions(course_id, session_date, status)`, `enrollments(student_id, status)`, `journal_entries(entry_date, source_type)`.
- Search-oriented: `students(full_name COLLATE NOCASE)`, `students(student_code)`, `students(phone)`, `teachers(full_name COLLATE NOCASE)`.
- Uniqueness per ERD: student_code, voucher_no, journal entry_no, idempotency_key, (student_id, course_id), (session_id, student_id), role/permission codes.
- Indexes are defined inside migrations and tested for plan improvement on representative seed data; every index change is covered by an integration test.
- Note: the final index list is finalized against a seeded benchmark dataset in Phase 2; this Section defines the baseline.

## 6. Balances at query time

- Journal balances and student debtor balances are computed with indexed aggregate queries (`SELECT ... GROUP BY account_id`) rather than denormalized running balances; "vouchers per cash register balance" is derived from movements.
- Regression tests assert the invariant: sum(debits)=sum(credits) for all entries at any point in history; student receivable from ledger == sum(due-collected) from domain tables.

## 7. Failure handling and availability

- On open: run a fast check (`PRAGMA quick_check`); if it fails, a safe (read-only) warning screen guides the user toward restoring from the most recent verified backup. Silent data loss is never accepted — always surface the issue via a user dialog.
- App close: preparations for `WAL checkpoint` and closing DB connections; handlers stop jobs first (closing job state is persisted in `job_state` cache table seeded by phase 2).
- Power-loss safety is delegated to WAL + transaction atomicity; the app never writes financial information outside a transaction.
- Database-critical errors are logged with `tracing` into `<data>/logs` and surfaced via an error code (not stack traces) to the UI.

## 8. Space and hygiene

- `VACUUM` offered as a maintenance action (Settings → Maintenance), scheduled monthly suggestion with approval (never automatic, never under UI load).
- WAL file checked at startup if bigger than 64 MB (generally over-storm indicator) — run checkpoint first.
- A manual `PRAGMA wal_checkpoint(TRUNCATE)` runs once before any backup snapshotting, keeping the snapshot procedure cheap and deterministic.

## 9. Reference to migration dependency

The mapping between phases and migration batches is defined in docs/TEST_PLAN.md §7 so each Phase's migrations ship with their matching tests run against a clean temp DB.
