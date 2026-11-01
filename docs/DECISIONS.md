# DECISIONS — Engineering Decision Log (ADR-style)
## Education Center Management System

Each decision: context → options → choice → consequences. New material changes during later phases are appended (never deleted).

| ID | Date | Decision | Rationale |
|----|------|----------|-----------|
| ADR-001 | Phase 1 | Technical documents are written in English; the product UI and printed documents are Arabic-first RTL | Documentation clarity and low-maintenance risk; product language requirements (Arabic UI, Arabic templates) remain fully in scope |
| ADR-002 | Phase 1 | Money is stored as i64 minor units with currency code + decimals in center settings (default ILS/2). Rust `Money` type central. No float anywhere | Exactness of financial math, easy testing, tiny storage; a single conversion surface prevents display drift |
| ADR-003 | Phase 1 | Percentages/ratios stored as integer basis points (10000=100%) | Avoids float precision problems in teacher/center splits |
| ADR-004 | Phase 1 | SQLite via SQLx runtime queries; single-writer connection pool with WAL | Modular monolith + desktop local DB; runtime queries keep the build free of compile-time DB resolution; one writer exploits SQLite semantics |
| ADR-005 | Phase 1 | Backup snapshot via `VACUUM INTO` + pre-checkpoint + integrity_check (Backup API equivalent) | SQLx lacks a direct backup-API hook; `VACUUM INTO` is a documented, single-statement, consistent snapshot creation under WAL; verified by integrity check |
| ADR-006 | Phase 1 | PDF generation via embedded Typst engine; fonts (Cairo and/or IBM Plex Sans Arabic) bundled | Native Arabic shaping/RTL, precise A4, page header/footer logic, table header repetition, deterministic reproducibility; preview == rendered PDF by construction via pdfium render |
| ADR-007 | Phase 1 | UI: SvelteKit static-adapter SPA + TypeScript + Tailwind + shadcn-svelte + ECharts | Modern, RTL-friendly, installable inside a Tauri webview, no SSR server to maintain |
| ADR-008 | Phase 1 | Permissions enforced in Rust at the command layer; frontend only hides UI elements | Security requirement: UI hiding is not security |
| ADR-009 | Phase 1 | Maker–checker is mandatory for sensitive financial operations (receipt/payment post, settle, shortfall decide, refund approve); no self-approval without a documented override reason | Robust requirement of the master project prompt; auditor-friendly |
| ADR-010 | Phase 1 | Voucher/document numbering minted centrally inside the service transaction (allocator in the same transaction) | Gap-free sequential numbers that disappear on rollback; a cancelled voucher keeps its number |
| ADR-011 | Phase 1 | Idempotency: unique `idempotency_key` on sensitive financial mutations provided by the UI (generated once per logical action) | Protects against double click, retry after timeout, or duplicate submission |
| ADR-012 | Phase 1 | Reversal entries are the only correction mechanism for posted data; postings forbidden to be updated/or deleted | A standard accounting requirement; keeps books auditable |
| ADR-013 | Phase 1 | `deferred` is a first-class shortfall decision type and is a no-posting state | Explicit implementation of "avoid auto-distribution"; keeps open items visible on control reports |
| ADR-014 | Phase 1 | A shortfall decision's teacher share adjusts the teacher payable account only via a ledger booking — never as an automatic cash movement or a persisted voucher | A documented decision is a booking, not a cash event; protects teachers from uncontracted deductions |
| ADR-015 | Phase 1 | Revenue recognition separated from cash collection: enrollments accrue deferred revenue; recognition policy configurable per course | Requirement to separate recognition from collection per prompt's financial specifications |
| ADR-016 | Phase 1 | Data directory resolution: OS-config path → portable marker → wizard; relative paths only inside `<DATA>` | Immunity to moving data between machines (canonical snapshot + relativepath integrity) |
| ADR-017 | Phase 1 | Local auth (Argon2id) only; no external identity provider | Local-first desktop product; keeps data and access fully inside the machine |
| ADR-018 | Phase 1 | Report builder operates on the registered report definitions (JSON config), never on raw SQL strings | Security requirement "no arbitrary SQL from UI"; validation via a safe registry |
| ADR-019 | Phase 1 | Reports' Excel export via `rust_xlsxwriter`; the PDF is Typst's | Windows-friendly, local-only, full streaming support for large reports |
| ADR-020 | Phase 1 | Timezone: stored UTC; display times per center timezone; business day = date passed from engine using center tz | Prompts normalize for local work and reporting |
| ADR-021 | Phase 1 | Session statuses include `makeup` linked by `original_session_id` and `reschedule_to_id` for postponement | Clear neutralization of the base session and weight inheritance in the engine |
| ADR-022 | Phase 1 | Attachments stored under `<DATA>/attachments/<domain>/` with UUID names; original names preserved as metadata only | Path traversal and filename injection safety |
| ADR-023 | Phase 1 | Charts: ECharts | Strong RTL/Arabic label support and consistent rendering inside webview |
| ADR-024 | Phase 1 | Inactivity lock default 20 min; configurable | Practical desktop security default |
| ADR-025 | Phase 1 | Backup encryption designed as a hook but disabled in V1 | Complexity tradeoff: keeps values opt-in with a dated scope note |
| ADR-026 | Phase 2 | Cargo workspace split: core crate `ec-core` (all business logic) + thin Tauri shell `ec-app` in `src-tauri`; migrations live in `core/migrations`; frontend in `web/` | Lets every phase gate `cargo test -p ec-core` without a GUI; shell stays dumb per ADR-008 |
| ADR-027 | Phase 3 | Auth sessions live in a Rust-side in-memory store (opaque 32-byte token); lockout counters, `last_login_at` and audit rows stay durable in SQLite | Desktop semantics (sessions die at app exit), no new schema table needed; `must_change_password`/`locked_until` columns already exist from migration 0001 |
| ADR-028 | Phase 3 | Inactivity auto-lock is enforced server-side at session verification (`SessionStore::verify`), not by UI timers | UI-timer enforcement is spoofable; verification refreshes activity on each successful call |
