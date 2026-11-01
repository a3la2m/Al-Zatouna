# SRS — Software Requirements Specification
## Education Center Management System — v1.0 (Phase 1)

Note: technical documents are written in English for engineering precision; the product itself (UI, printed documents, templates) is Arabic-first with full RTL. See docs/DECISIONS.md (ADR-001).

---

## 1. Overview

### 1.1 Purpose
A standalone desktop application to manage a small/medium education center (or training center): students, teachers, subjects and courses, scheduling and attendance, a complete financial engine (course financial agreements, collection, teacher entitlements, shortfall decisions, refunds, settlements), a double-entry ledger, cash register and vouchers, reports and A4 printing with Arabic/RTL, plus a portable local backup system. No external server and no internet requirement for core functionality.

### 1.2 Intended users
| Role | Example | Primary usage |
|------|---------|---------------|
| System Administrator | owner / IT | app setup, users, backups, data directory |
| Center Manager | director | approve financial operations, shortfall decisions, settlements, reports |
| Accountant | finance staff | collection, vouchers, cash register, expenses, settlement drafts |
| Registration Clerk | front desk | student/course enrollment, receipt voucher drafts |
| Scheduler | coordinator | rooms, schedules, makeup/compensation sessions |
| Teacher | — | own schedule, own students' attendance, own entitlements only |
| Read-only | owner / inspector | permitted report viewing only |

### 1.3 Scope
**In scope (V1):** all modules 1–14 below; starts with a single branch with a ready multi-branch structure.
**Out of scope (V1):** guardian web portal, live multi-device sync, full payroll system, mobile app, public website, active multi-currency (structure is ready, one currency enabled).

---

## 2. Functional Requirements

### Module 1 — Data Directory management
- FR-1.1 First-run wizard offering: (a) default OS path, (b) custom folder, (c) portable mode; with read/write permission testing and free-space check before acceptance.
- FR-1.2 Creation of the standard folder tree (database, config, branding, attachments, templates, backups, exports, logs) + manifest.json.
- FR-1.3 Database initialization and versioned migrations.
- FR-1.4 Later path change via system settings, following the safe-migration procedure: close DB connections, backup, copy, verify integrity, update setting, reopen DB from the new location; the source is never deleted before the move is confirmed successful.
- FR-1.5 Portable mode: data directory next to the executable so the whole bundle runs on another machine as-is.
- FR-1.6 No important data is stored inside the installation folder.

### Module 2 — Authentication and permissions
- FR-2.1 Login / logout / change password / administrative password reset (documented, audited).
- FR-2.2 Passwords stored with Argon2id only.
- FR-2.3 Flexible RBAC: 7 built-in roles plus per-user permission overrides.
- FR-2.4 Maker–checker separation for sensitive financial operations: a user cannot approve an operation they created, unless an explicit documented override is authored with a reason recorded in the audit log.
- FR-2.5 Last-login and user-activity logging (audit log).
- FR-2.6 Users are disabled, never deleted; history preserved.
- Full permission matrix: docs/SECURITY.md.

### Module 3 — Students and guardians
- FR-3.1 Complete student record (name, optional national ID, birth date, optional gender, phone, address, email, guardian info, optional photo, notes, status, registration date).
- FR-3.2 Automatic sequential student number.
- FR-3.3 Search by name, number, phone (global quick search).
- FR-3.4 Archive / freeze without deletion.
- FR-3.5 Student profile: enrollments, attendance, account statement, payments and refunds, attachments.
- FR-3.6 Credit balance for a student, usable across that student's enrollments.
- FR-3.7 Guardian entity ready to be linked to several students; no guardian portal in V1.

### Module 4 — Teachers
- FR-4.1 Teacher record with sequential number, schedule, executed sessions, teacher attendance.
- FR-4.2 Teacher statement: accrued entitlements, payments, remaining balance.
- FR-4.3 Archiving without affecting past transactions.
- FR-4.4 Data isolation: a teacher sees only own courses/students/entitlements; enforced in Rust, never in the frontend.

### Module 5 — Subjects and courses
- FR-5.1 Subject is a separate entity from Course.
- FR-5.2 Course linked to subject, teacher (with substitute support), room, dates, times and fees; single-student (private) and multi-student courses both supported.
- FR-5.3 Course statuses: Draft, RegistrationOpen, Active, Completed, Suspended, Cancelled, Archived.
- FR-5.4 A course cannot be approved without an active financial agreement; on approval a snapshot of the financial terms is stored.
- FR-5.5 Later material changes go through Amendment (new version + approval record + reason); no retroactive effect on historical calculations.

### Module 6 — Scheduling, rooms and attendance
- FR-6.1 Rooms entity with capacity, status and occupancy reporting.
- FR-6.2 The recurring schedule generates Sessions (independent operational records); proving a session happened never relies solely on the recurring pattern.
- FR-6.3 Room-conflict and teacher-conflict prevention over time windows, enforced via Rust validation plus DB-level checks/indexes.
- FR-6.4 Session statuses: Scheduled, Executed, Cancelled, Postponed, Makeup; makeup sessions link to their original session.
- FR-6.5 Substitute-teacher support and room changes with a change log (who, why, when).
- FR-6.6 Student attendance (present / absent / late / excused) and teacher attendance, with recorder user and timestamp.
- FR-6.7 Per-course attendance policy inside the financial agreement: which sessions count as executed, effect of absence/makeup on entitlements.

### Module 7 — Enrollment
- FR-7.1 One enrollment record per (student + course) stating base fee, discount, waiver, net fee.
- FR-7.2 Enrollment statuses: Active, Completed, Withdrawn, Frozen, Transferred, Cancelled.
- FR-7.3 Late enrollment support (participation basis computed per course policy).
- FR-7.4 Payment plan with dated installments and partial payments.
- FR-7.5 Pay on behalf of a student; one payment covering several enrollments; payment–enrollment allocation recorded.
- FR-7.6 Clear separation between: discount/waiver (reduce the due), uncollected shortfall (needs a decision), due/collected/remaining.

### Module 8 — Financial engine
- FR-8.1 Per-course financial agreement defining teacher entitlement by one of six methods (docs/FINANCIAL_ENGINE.md); selected per course, never globally.
- FR-8.2 On course approval the agreement becomes an immutable financial document; later changes only via amendment.
- FR-8.3 Uncollected shortfall is never auto-distributed; each case gets a documented decision of a defined type, separating student debt, center burden and teacher burden.
- FR-8.4 Per-course refund policy; refund may never exceed the available amount; the proposed computation is documented before approval.
- FR-8.5 Settlements (course / teacher / center / shortfall / refunds / partial / final) are immutable once approved; corrections via reversal entries or documented adjustments.
- FR-8.6 Unique idempotency key for every sensitive financial operation.

### Module 9 — Ledger
- FR-9.1 Double-entry bookkeeping with a seeded chart of accounts (cash, bank, student receivables, teacher payables, revenue, expenses, refunds).
- FR-9.2 In every entry: total debit = total credit, validated in Rust before posting and in the database.
- FR-9.3 Posted entries are immutable; corrections happen via reversal entries linked to the original.
- FR-9.4 Revenue recognition is separated from actual cash collection.
- FR-9.5 Every entry references its source document (type + id); every financial document is traceable.

### Module 10 — Vouchers, cash register, expenses
- FR-10.1 Receipt and payment vouchers with strict sequential unique numbering; cancellation is recorded with a reversal entry and the cancelled voucher keeps its number; re-printing an old voucher never issues a new voucher number.
- FR-10.2 Amounts printed in Arabic words automatically.
- FR-10.3 Payment methods: cash, card, bank transfer, e-wallet.
- FR-10.4 Vouchers cannot be deleted after issuance; print and cancellation history is kept.
- FR-10.5 Cash register: central register with optional sub-registers; opening balance, receipts, payments, transfers between registers, daily closing, booked vs counted balance variance, closing approval.
- FR-10.6 Cash receipts require an open cash register when center settings demand it.
- FR-10.7 Expenses module: categories + expense linked to a payment voucher after approval, with attachments.

### Module 11 — Reports
- FR-11.1 Administrative, financial and control report sets (40+ reports) as listed in the master project prompt, with totals.
- FR-11.2 A flexible report builder allowing permitted users to choose columns and filters without writing SQL; arbitrary SQL from the UI is forbidden.
- FR-11.3 Search, filter, sort, period/branch/teacher/course/student selection, column choice, saved filters, favorite reports.
- FR-11.4 PDF and Excel export, print preview before printing.
- FR-11.5 All reports respect user permissions (e.g., a teacher sees only own data).

### Module 12 — Printing
- FR-12.1 A4 default (210x297 mm), portrait, customizable margins, RTL direction, page numbering, print date, report title.
- FR-12.2 Arabic fonts (Cairo or IBM Plex Sans Arabic) embedded in generated PDFs, independent of user-installed fonts.
- FR-12.3 Repeating table headers on every page; protection of important rows from splitting; multi-page documents.
- FR-12.4 True print preview matching print output and PDF (not a screenshot).
- FR-12.5 Template versioning (template snapshot) at document issuance so old documents never change appearance.
- FR-12.6 Customizable header/footer: center data, logo, custom texts, signatures, page x/y, print date.
- Full details: docs/PRINTING.md.

### Module 13 — Backup and restore
- FR-13.1 Manual and automatic backups (daily, weekly, monthly), pre-upgrade backup, pre-migration backup.
- FR-13.2 Destinations: local folder, external/USB disk, network folder, synced folders (Google Drive / OneDrive) — as complete standalone backup packages only, never a live sync of the DB file.
- FR-13.3 Backup package: database + key settings + branding + templates + attachments + manifest with versions, timestamp and SHA-256 checksums per file.
- FR-13.4 Restore wizard with verification steps (choose file, verify integrity, check versions, show contents, back up current data, restore, run integrity check, restart).
- FR-13.5 Safe snapshot mechanism; copying the live SQLite file with plain file copy is forbidden. Primary mechanism: `VACUUM INTO` (documented equivalent of the Backup API) followed by `integrity_check`.
- FR-13.6 Retention policy per backup type with automatic pruning of the oldest copies.

### Module 14 — Dashboard and audit log
- FR-14.1 Dashboard with real DB-backed KPIs: students, teachers, active courses, enrollments, today's sessions, today's attendance, due fees, collections, student receivables, teacher payables, payments, expenses, cash balance, courses needing settlement, pending refunds, pending shortfall cases; plus revenue/collection/enrollment/attendance charts.
- FR-14.2 Mandatory audit log for all defined events (docs/SECURITY.md), not editable through the regular UI, with old/new values and reasons for sensitive changes.

---

## 3. Non-functional requirements

| ID | Requirement |
|----|-------------|
| NFR-1 | Full autonomy: no external server or internet for core features; local SQLite only |
| NFR-2 | Money integrity: integer minor units everywhere; FLOAT/REAL for money is forbidden |
| NFR-3 | Data integrity: foreign keys, CHECK, UNIQUE, WAL mode, transactions, versioned migrations |
| NFR-4 | Performance: paginated lists everywhere; heavy work in background tasks with progress feedback |
| NFR-5 | Language: Arabic-first fully RTL UI; printed documents Arabic-first |
| NFR-6 | Portability: the data directory can be moved to another machine or disk intact |
| NFR-7 | Security: Argon2id, permission checks on every Tauri command, path traversal prevention, no internal details leaked to end users |
| NFR-8 | Reliability: DB busy/locking handled gracefully; idempotency for sensitive operations; all financial writes inside transactions |
| NFR-9 | Extensibility: layered modular monolith, ready for multiple branches |
| NFR-10 | Every architectural decision and material design change is recorded in docs/DECISIONS.md |

---

## 4. Constraints and acceptance criteria

### 4.1 Technical constraints
Rust + Tauri 2 + SQLite + SQLx + SvelteKit + TypeScript + Tailwind CSS; layered modular monolith with clear separation of presentation, business logic and data access; all business/financial logic in Rust; no microservices, no external DB servers.

### 4.2 Release-1 acceptance criteria
1. Runs as a standalone desktop application with a configurable data directory.
2. Ledger always balanced (debits = credits) in 100% of tests.
3. No duplicated financial operations under double-click, crash or restart (idempotency proven by tests).
4. Historical agreements never change retroactively; amendments only.
5. Vouchers are re-printable without issuing a new number; cancellation via reversal entry.
6. Restoring a backup on a clean machine succeeds 100% in documented tests.
7. A4 Arabic RTL PDFs with preview matching actual print.
8. Configurable header/footer, actionable RBAC, immutable audit log.
9. Safe data-directory migration with zero data loss.
10. Cross-module behaviors are covered by unit and integration tests that were actually run; nothing is claimed done without a passing test.

### 4.3 Phased execution rule
Phases 1–10 (see README.md) are executed in order; no phase starts before the previous phase's tests pass, with an end-of-phase report.
