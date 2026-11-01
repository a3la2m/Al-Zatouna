# WORKFLOWS — Business process flows (Phase 1)
## Education Center Management System

Each flow lists: actors/permissions, states, engine involvement, ledger postings, and audit events. Status ladders follow docs/ERD.md enums.

---

## W1 — First run & data directory (Phases 2–3)
1. Wizard: choose default/custom/portable → permission+space test → create tree + manifests → DB init + migrations + seeds → create admin user (no default password) → summary.
- Audits: `data_directory.init` (system), `users.create` (admin bootstrap).
- Re-runnable: changing directory from Settings → System triggers the DATA migration flow (docs/BACKUP_RESTORE.md §3).

## W2 — Course lifecycle
Draft → RegistrationOpen → Active → Completed → Archived; branches to Suspended / Cancelled.
1. Create (perm `courses.create`) — saved as Draft; schedule drafted as weekly rules.
2. Attach **Financial Agreement** (draft) — method, rates, conditions (incl. refund policy).
3. Preview entitlements/fees computation (engine `what-if`).
4. Submit for approval → Approve (perm `courses.approve`, different user) — freezes agreement snapshot; approval creates default sessions for the schedule (status Scheduled).
5. RegistrationOpen → seats fill (see W3).
6. Active gate: `Effective start date` reached or manual activation; sessions become operable.
7. Completion: manual transition or auto when end_date passed + all sessions resolved; per course settlement checklist (unsettled enrollments flagged).
8. Amendments: `agreements_amend` never changes the frozen snapshot; the new version is the baseline for FUTURE events (accruals, refunds, settlements). Material changes record an audit reason.

## W3 — Enrollment (student + course)
1. Quick student lookup or create (perm students.create).
2. `enrollments_create` → fee terms (base fee default = course fee, editable by permission; discount/waiver with reason), net computed → **Active** (at later statuses, transfer/freeze/withdraw flows).
3. Payment plan (optional): N installments with dates (Open state).
4. Late enrollment: engine derives `join_offset` sessions executed before join (excluded from student's basis per course policy).
5. Credit usage: a student with positive credit balance may apply credit (`credit_apply`); allocation row tracks usage.
- Enforcing invariants: one enrollment per (student, course); freeze/withdraw/cancelled statuses keep records; transferring moves with engine-recomputed basis.

## W4 — Money collection (receipt voucher)
Actors: clerk creates, permitted approver posts (maker–checker).
1. Preferred from student profile: `payment_record(enrollment_ids[], amount, method, date, payer)` → engine allocates by policy order (installments due → oldest due → forfeit/cross-enrollment (recorded lines)) and drafts a **Receipt Voucher**.
2. Cash payment ⇒ open-register check (setting default ON): posting rejects when the register is closed/unselected by required config.
3. Post (`voucher_receipt_post`) — in ONE transaction: voucher number minted by sequence, allocation rows saved, ledger entry posted (Dr Cash/Bank/Wallet Cr Students Receivable), audit events (`vouchers.issue`).
4. Printing (A4 template, print log). Reprinting reuses the same voucher; no new number.
5. Cancel (perm) ⇒ reversal entry (documented reason), cash movement subtracted, status `cancelled`; keeping original number; audit `vouchers.cancel`.
- Idempotency: same `idempotency_key` in retried calls returns the same voucher; no duplicate.

## W5 — Sessions & attendance
1. Weekly schedule → generated sessions (Scheduled). Conflict detection at any room/teacher change (command refusals with `conflict.room_clash`).
2. On session day: mark attendance for students (present/absent/late/excused) and teacher — recorded with `recorded_by/recorded_at`.
3. `sessions_set_status(id, executed, ...)` — turning scheduling into "executed" creates the accrual basis for teacher entitlement math (per method).
4. Change flows: change room/teacher with reason → change log row; substitute teacher is captured per-session in entitlement calculations; makeup: create a **Makeup** session linked to the original `postponed/cancelled`, inheriting weight via engine rules.
- Audits: `attendance.record`, `sessions.change`, `sessions.cancel/makeup`.

## W6 — Teacher entitlements accrual & payout
1. Engine accrual run per course/period (`entitlements_run`): loads executed sessions + enrollments + frozen agreement → posting Dr Teacher Payable accrual / Cr (per method), with `engine_snapshot_json` provenance.
2. `entitlements_list` exposes per-teacher payable balances.
3. Payout: `create payment voucher (source_type=teacher_entitlement)` — bound to payable amount; approved → paid → posted (Dr Teacher Payable Cr Cash). Partial payouts allowed; over-payout rejected (conflict code `voucher.overpaid_source`).
- Policy rule: no auto-deduction from entitlements without a documented decision (W7).

## W7 — Shortfall decision
1. Trigger: course settlement prep / finances screen shows uncollected enrollments (uncollected = net − allocated, from engine).
2. `shortfall_propose(enrollment_id)` — engine snapshot stored; status `pending`.
3. Decision by permitted user (`shortfall.decide`): choose one of the 8 decision types; required inputs vary:
   - `partial_split` → center_bp+teacher_bp = 10000
   - `fixed_center` / `fixed_teacher` → amounts
   - `documented_exemption`/`exceptional` → reason + attachment optional
   - `deferred` → stays open on control report.
4. Posting: recorded in journal per engine maps (see docs/FINANCIAL_ENGINE.md §2); **NOT** a cash collection; teacher share adjusts payable only via ledger booking with its own audit event.
- Maker–checker: proposer ≠ approver.

## W8 — Refund
1. `refund_request(enrollment_id)` — status `submitted` (or DRAFT).
2. `refund_compute` — engine proposes amount with breakdown (weight basis, remaining sessions, attendance factor, admin fee, prior refunds); capped at paid-to-enrollment; zero cap results in rejection path automatically.
3. Approve (different user) by policy rules (cutoff dates, exception with reason by manager).
4. Payout: payment voucher (source_type=refund) bounds by approved amount; posts ledger (per docs/FINANCIAL_ENGINE.md §5), updates balances, `paid` status, audit `refunds.paid`.
- Reject path: `rejected` with reason; remains visible for audit trail.

## W9 — Settlements
1. `settlements_prepare(scope)` — course|teacher|center|shortfall|refunds|final within a period bound; engine snapshot produced, status `pending_approval`.
2. Approve (different user with permission) → immutable snapshot; ledger log; relevant control list updated.
3. Reverse (perm settlements.approve): creates a **reversal settlement** linked to the original (with snapshot), audit `settlements.reverse` — original is never edited.

## W10 — Cash register daily cycle
1. Open register (`register_open` with opening count) — audit `register.open`.
2. Operating: cash movements from vouchers/transfers (both sides paired for transfers).
3. Close (`register_close` with counted amount): draft with booked vs counted + variance.
4. Approver approves/updates the closing; variance > 0 posts to variance account per settings; closing approved ⇒ register locked until next day (cash ops route through closed/other registers).
- Rule: no cash acceptance without an open register (per setting), no register-state bypass; enforced in the voucher post flow.

## W11 — Expenses
1. Create draft expense (category, date, amount, method, account/designated cash, beneficiary, description, attachments).
2. Submit → Approved (different user) → generates a linked **payment voucher** (source_type=expense) → paid path mirrors W4 standards with ledger entry.
- Cancel path: reversal entry before cash release; after release: recorded reversal + voucher cancellation rules apply.

## W12 — Backup, restore, data move
- backup_run_now / schedule (per retention policy) → verify checksum → list/prune (docs/BACKUP_RESTORE.md).
- Restore wizard steps with safety copy and audit entries; a restore refuses an unlocked running financial state (jobs cancelled gracefully first).

## W13 — Reports & printing control
1. All report runs (admin/finance roles) use the registry; teacher-scope data limited by W-isolation rules.
2. Export paths: PDF via the print module; Excel via the xlsx writer; permission-specific view of the export list; files under `<DATA>/exports/...`.
