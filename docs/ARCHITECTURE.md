# Architecture — Education Center Management System
## Phase 1 design document

See docs/DECISIONS.md for the decision log behind everything below.

---

## 1. Architectural style

**Layered Modular Monolith** inside a single Tauri 2 desktop application. No external server, no microservices. The Rust side is organized as independent feature modules with a shared core; the SvelteKit frontend is a presentation layer only.

### Layers (Rust side)
1. **Presentation (frontend)** — SvelteKit + TypeScript. Renders data, collects input. Contains **zero** business/financial logic. Talks to Rust exclusively through Tauri IPC (commands/events).
2. **API / Commands layer** — Tauri command handlers. Responsibilities: deserialize args, resolve the caller's user/session, enforce permissions, delegate to services, map errors to a stable error code contract, emit progress events.
3. **Business logic (services)** — the backbone: the financial engine, scheduling validation, agreement/amendment rules, voucher numbering, settlement workflows. All money math lives here (`common::money`), all policy decisions live here.
4. **Data access (repositories)** — SQLx-based repositories per module; prepared statements; transactions; no business rules allowed here beyond mapping rows.
5. **Cross-cutting core** — `database` (pool, pragmas, migrations), `config` (data directory, app settings), `security` (auth, permissions, crypto), `printing`, `backup`, `audit`, `common` (Money, DateRange, errors, seq).

### Dependency rule
`commands -> services -> repositories -> database`, and everything may use `common`. Modules may not reach into another module's internals; cross-module interaction goes through the depended-upon module's public service API (e.g., the finance engine consumes sessions/enrollments via the scheduling/enrollment services, never raw SQL on their tables from a foreign module except through repositories exports used by the engine).

---

## 2. Technology stack

| Concern | Choice | Notes |
|---------|--------|-------|
| Desktop core | Rust + **Tauri 2** | commands, events, fs scope policies, single instance |
| Database | **SQLite** (WAL mode) via **SQLx 0.8**, runtime queries | pool=1 writer + readers; pragmas at open |
| Async runtime | Tokio (used by Tauri and background jobs) | heavy tasks spawned off the UI |
| Serialization | serde / serde_json | stable DTOs per command |
| Password hashing | **Argon2id** (`argon2` crate) | params documented in SECURITY.md |
| Money | **i64 minor units** + currency code + decimals in center settings | custom `Money` type in Rust; no float anywhere |
| PDF engine | **Typst** (as a Rust crate, template-driven) | native Arabic/RTL shaping, font embedding, A4, page headers/footers |
| Excel export | `rust_xlsxwriter` | streaming writer for large reports |
| Frontend | **SvelteKit (Svelte 5)** + TS, static-adapter SPA | no SSR server; runs inside webview |
| Styling | **Tailwind CSS** with RTL variants (`rtl:`/`ltr:`, logical properties) | navy/blue/teal palette per spec |
| Components | **shadcn-svelte** (copied-in components) | design tokens; dark/light themes |
| Charts | **Apache ECharts** via `svelte-echarts` wrapper | strong RTL/Arabic label support |
| Logging | `tracing` + tracing-subscriber (file + rolling) | logs under `<data>/logs` |
| IDs/UUID | uuid v7 for external identifiers of files/packages | DB PKs remain INTEGER |
| Checksums | sha2 (SHA-256) | backup manifest, attachments |
| Time | chrono + current-tz from center settings | ISO-8601 storage |

---

## 3. Rust project layout

```
src-tauri/
  tauri.conf.json
  Cargo.toml
  migrations/                 # sqlx .sql migrations, numbered
    0001_init.sql ...
  src/
    main.rs                   # entry
    lib.rs                    # app builder, state, command registration
    state.rs                  # AppState (DbPool, settings cache, session mgr)
    error.rs                  # AppError + stable error codes
    commands/                 # thin IPC handlers grouped by module
      auth.rs students.rs courses.rs finance.rs vouchers.rs ...
    common/
      money.rs                # Money struct: minor units + currency
      money_words.rs          # Arabic amount-in-words
      dates.rs seq.rs results.rs validation.rs
    config/                   # data directory, portable mode, settings files
      data_dir.rs settings.rs wizard.rs
    database/
      pool.rs pragmas.rs migrations.rs integrity.rs
    security/                 # argon2, sessions, permission evaluation
      auth.rs session.rs permissions.rs crypto.rs
    audit/                    # audit writer + queries
    backup/                   # snapshot, package, restore, retention
    printing/                 # typst engine, templates, print profiles, jobs
    reporting/                # report registry, builder config validator
    modules/                  # feature modules (repos + services + DTOs)
      users/ branches/ students/ teachers/ subjects/ rooms/
      courses/ scheduling/ attendance/ enrollment/
      finance/                # agreements, entitlements, shortfall, refunds,
                              # settlements, ledger
      payments/ vouchers/ cash_register/ expenses/ reports/ dashboard/
    tests/                    # integration tests (see TEST_PLAN)
```

Mirrored SvelteKit layout in `src/` (routes per module, `lib/api` typed IPC wrappers, stores, components, printing preview).

---

## 4. Tauri IPC design

- **Commands** run through a single dispatcher wrapper that:
  1. loads the session from the webview-managed token and rejects unauthenticated calls,
  2. evaluates the permission for the operation (`permission` attribute on the command definition),
  3. validates input (serde types + explicit validators; never trust the UI),
  4. on writes, opens a transaction in the service layer and commits/rolls back atomically,
  5. returns `Result<Dto, AppError>` where `AppError` serializes as `{ code, message_ar, field?, details? }`.
- **Events**: progress events for long jobs (backup, export, report generation), data-refresh events after mutations so other windows update.
- **Idempotency**: every mutating financial command accepts an `idempotency_key` (uuid generated once per logical action from the UI); unique indexes make retries safe.
- Token storage: only a session handle is kept; the password never leaves the Rust side after unlock.
- The webview gets a minimal, explicitly enumerated `fs.scope` and no arbitrary shell access; file dialogs go through Tauri dialog APIs only.

---

## 5. State and concurrency

- `AppState` holds: DB pool (sqlx `SqlitePool`), read/write concentrate on a single writer connection to exploit SQLite single-writer semantics; settings cache (hot-reloaded from JSON files); active user session; running background jobs registry.
- Long operations (backup, restore, report/PDF generation, migration of the data directory) run on background Tokio tasks and emit progress events; the UI never blocks.
- Financial mutations: one service-level SQL transaction; the same transaction allocates voucher numbers, writes ledger lines, updates balances-dependent rows and writes audit rows — commit or rollback together.
- Busy handling: `busy_timeout` (5s) + WAL; on `SQLITE_BUSY` errors commands retry once inside the service; the UI surfaces a friendly retry message.
- Single app instance enforced in Rust to avoid two writers racing on the same data directory.

---

## 6. Runtime configuration and data directory

- Resolution order for the data directory: (1) the path recorded in the app config file stored in the OS standard location (e.g., `~/.config/education-center/app.ini`), (2) portable marker file next to the executable, (3) first-run wizard result.
- All module code receives paths via a `DataStore` service; modules must never build their own absolute paths from guesses (path traversal rules in SECURITY.md).
- The settings JSON files (app-settings, print-settings, backup-settings) live in `<data>/config`; DB-stored config is only center/business settings.
- Full structure and migration procedure: docs/BACKUP_RESTORE.md.

---

## 7. Error handling model

- Rust: `AppError` ladder — `Validation`, `PermissionDenied`, `NotFound`, `Conflict` (invariants, e.g. room clash, over-refund), `Concurrency` (busy/retry), `Storage` (io/backup), `Internal`.
- Every user-visible error carries an Arabic message and a stable code; internals are logged with `tracing`, never shown raw.
- UI: a toast/dialog per error class; loading, empty, and error states are first-class page states.
- Panics in commands are caught by the handler wrapper and converted to `Internal` errors so the app cannot crash from a business path.

---

## 8. Testing strategy (summary; details in TEST_PLAN.md)

- Unit tests on pure services (money math, entitlement methods, refund policy, words-in-Arabic).
- Integration tests run against a temporary SQLite database created from migrations, exercising transactions, conflicts and the ledger balance invariant.
- End-to-end tests via the Tauri driver (tauri-driver / WebDriver) for critical journeys: login, enroll, collect, print.
- Backup/restore tests cover snapshotting under load, checksum verification, cross-machine restore rehearsal.

---

## 9. Extensibility notes

- Multi-branch: every branch-scopeable entity already carries `branch_id`; the branch context is a request-scoped parameter; a branch picker arrives in a later release without schema surgery.
- Encryption-at-rest for the DB/backup files is designed as an interface in `backup` and `database` (a key-provision step in the wizard) but ships disabled by default in V1.
- Report definitions are data (a registry), so new report types are additions, not code branches.
