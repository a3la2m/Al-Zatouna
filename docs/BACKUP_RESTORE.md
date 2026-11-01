# BACKUP & RESTORE + Data Directory management
## Education Center Management System (Phase 1)

## 1. Data directory layout

```
<DATA>/
  database/education_center.db (+ -wal/-shm)
  config/
    app-settings.json        # preferences, window state, defaults
    print-settings.json      # global print defaults
    backup-settings.json     # schedule, destination, retention
    branding/                # logo files and visual identity
  attachments/
    students/  teachers/  receipts/  documents/
  templates/
    receipts/  reports/  certificates/
  backups/
    automatic/  manual/
  exports/
    pdf/  excel/
  logs/
  manifest.json
```

- `<DATA>` default per OS: Windows `%APPDATA%/EducationCenter/`; Linux `~/.local/share/education-center/`; macOS `~/Library/Application Support/EducationCenter/`.
- Alternative modes: custom folder chosen in the wizard; **portable mode** — a `portable.marker` file next to the executable makes `<DATA>` the executable's folder (Self-contained bundle machine-to-machine).
- The pointer to the current `<DATA>` is kept at an OS-standard app-config path, never inside the install folder.
- `manifest.json` holds: app version, schema version, data-directory schema version, currency defaults, created date, last integrity check time.

## 2. First-run wizard (Phases 2–3 scope)

1. Choose path: default / custom / portable (option list to browse directory with memory of navigation).
2. Permission test: write+read check with a temp file, plus free-space check (< threshold rejected).
3. Folder skeleton creation with retry-safe steps (resumable wizard).
4. Database initialization: create + migrations + seeds; admin user creation via wizard input.
5. Summary with path, DB size, versions; "change later" note pointing to Settings → System.

## 3. Data directory relocation (Settings → System)

Safe-migration procedure (blocked-while-running background jobs):
1. Stop current financial ops (writer connection closed; background jobs drained).
2. Auto backup of the current data directory state (as a manual backup package, §4).
3. Pre-check destination: permissions, free-space, destination-not-inside-source, not inside install dir.
4. Copy: database via safe snapshot (§5), all attachments/templates/branding/config with checksum verification per file.
5. Verify: run `integrity_check` on the copied DB + compare file counts/SHA-256 manifest.
6. Switchover: switch DATA pointer, reopen pool from new location, run application self-test, only after user confirms the app passes its self-test does the wizard finish.
7. Cleanup policy: the OLD source data directory is left untouched by default (user may delete it later manually), unless the user explicitly selects "delete original after successful move". The wizard never deletes the source while the destination lacks checksum-verified files.

## 4. Backup packages

- Package = directory (zipped with `.ecbackup` suffix) built at a destination, containing:
  - `db/education_center.db` (safe snapshot)
  - `settings/` copies of config JSON files + branding
  - `attachments/` full copy + `templates/` full copy
  - `manifest.json`: package version, app version, schema version, timestamp, creator machine/user, file list with per-file SHA-256
- Naming: `EC-backup_<run-type>_YYYYMMDD-HHMMSS` where run-type ∈ manual | auto_daily | auto_weekly | auto_monthly | pre_upgrade | pre_migration.
- Checksum: SHA-256 per file recorded in manifest; `manifest.sha256` holds the manifest's own checksum.

## 5. Safe SQLite snapshot

- NEVER plain file-copy of a live DB.
- Procedure (deterministic and atomic):
  1. `PRAGMA wal_checkpoint(TRUNCATE);` (best effort; busy is tolerated)
  2. `VACUUM INTO 'backups/.staging/db_snapshot.db';` — single-statement file creation of a self-contained copy under the current database's connection; creates a consistent snapshot with all WAL contents checked in, appropriate for a running DB.
  3. `PRAGMA integrity_check;` on the snapshot file (fresh connection, `readonly`).
  4. Move the verified snapshot file into the package (staging dir ensures a failed run leaves no half package; retry naming is timestamped).

## 6. Scheduler and retention

- backup-settings.json: enable auto, daily at HH:MM, weekly day-of-week, monthly day-of-month; keep_last (per type), destination(s) (primary + optional mirror list).
- Destinations: any local or network path (also typical cloud-sync folders like Google Drive/OneDrive); multiple destinations strongly recommended.
- Retention: after a successful backup, prune oldest packages per type down to keep_last; pre_upgrade/pre_migration backups never auto-pruned.
- All run history logged to `<data>/logs/backup.log` and surfaced on the Settings → Backups screen.
- Run when app active: skip-and-reschedule if a financial transaction is mid-flight (never overlaps a user workflow), or alternatively pause tasks (configurable sentinel).

## 7. Restore wizard

1. Select `.ecbackup` package (dialog).
2. Verify manifest + per-file SHA-256 checksums; report failures per file and abort on mismatch.
3. Version gates: schema_version ≤ current supported; app_version noted; user confirms proceeding (downgrade path blocked with an explanatory message).
4. Preview contents: DB metadata, counts per module derived from manifest, attachments count, timestamp, size.
5. Safety backup of CURRENT data directory first (pre_restore package, never skipped).
6. Perform restore: stop jobs, close DB, replace DB + settings + attachments + templates atomically from a staging directory, then reopen DB.
7. `integrity_check`, quick smoke-test queries, audit-log entry for the restore action.
8. Restart request. All steps emit progress events.

## 8. Cloud-sync positioning (explicit)

Folders inside Google Drive/OneDrive are supported as **destinations of complete backup packages**. No live syncing of the working database file is ever supported — using active DB inside a sync-folder risks corruption and is actively prevented by warnings + a blocked path check. Backup packages are self-contained and movable to another machine through the restore wizard.

## 9. Integrity validation matrix (test hooks, see TEST_PLAN.md §7)

- snapshot correctness under active transactions (concurrent source of truth)
- checksum mismatch rejection, truncated file rejection
- restore on clean machine: full wizard path + cross-machine file transfer rehearsal
- retention pruning correctness and pre_upgrade mutation protection
- failure at any step: staging rollback, inconsistent half-package rejection, and audit logging.
