# ERD — Entity Relationship Design
## Education Center Management System (Phase 1)

All money fields are integer minor units (suffix `_minor`). All timestamps are ISO-8601 TEXT (UTC marker `Z`), business dates are local-date TEXT (`YYYY-MM-DD`). Enum-ish statuses are TEXT constrained by CHECK. FK = foreign key with `ON DELETE` policy noted. `*` marks UNIQUE.

---

## 1. Domain overview (modules → entities)

| Module      | Entities |
|-------------|----------|
| Access      | users, roles, permissions, role_permissions, user_roles, user_permission_overrides, audit_log |
| Center      | branches, center_settings |
| Students    | students, guardians, student_guardians |
| Teachers    | teachers |
| Catalog     | subjects, rooms |
| Courses     | courses, course_schedules, course_change_log |
| Scheduling  | sessions, student_attendance, teacher_attendance |
| Enrollment  | enrollments, payment_plans, installments |
| Finance     | financial_agreements, agreement_amendments, student_credit_ledger, shortfall_decisions, refund_requests, settlements |
| Ledger      | accounts, journal_entries, journal_lines |
| Vouchers    | voucher_sequences, receipt_vouchers, payment_vouchers |
| Cash        | cash_registers, cash_movements, register_closings |
| Expenses    | expense_categories, expenses |
| Platform    | attachments, print_profiles, document_print_log, saved_reports |

---

## 2. Core relationships

- branches 1—* students, rooms, courses, cash_registers, voucher_sequences
- students *—1 branches; guardians *—* students (via student_guardians, is_primary)
- teachers 1—* courses (teacher_id) — and courses may reference substitute teachers
- subjects 1—* courses
- rooms 1—* courses (default room); rooms 1—* sessions; teachers 1—* sessions (+ substitute_teacher_id → teachers)
- courses 1—* course_schedules 1—* sessions (generation source stored on each session: source_schedule_id)
- courses 1—1 financial_agreements (active version); financial_agreements 1—* agreement_amendments
- courses 1—* enrollments *—1 students; enrollments 1—1 payment_plans 1—* installments
- enrollments 1—* refund_requests 1—1 payment_vouchers (after payout)
- enrollments 1—* shortfall_decisions
- courses 1—* settlements (scope = course|teacher|center)
- ALL financial documents → journal_entries 1—* journal_lines *—1 accounts
- receipt_vouchers/payment_vouchers → journal_entries (source link), and → cash_movements for cash
- cash_registers 1—* cash_movements; cash_registers 1—* register_closings
- expenses 1—1 receipt/payment_vouchers (linked voucher_id)
- attachments *—* arbitrary entities via (entity_type, entity_id) polymorphic link
- journal_entries: reverse_of_id → self (reversals)
- sessions 1—* student_attendance, 1—1 teacher_attendance (row per session)
- document_print_log: (document_type, document_id) → any issued document + print_profiles + template_version

---

## 3. Entity tables (attributes)

### Access
**users**: id, username*, full_name, password_hash, is_active, must_change_password, last_login_at, last_login_ip, failed_attempts, locked_until, created_at, updated_at
**roles**: id, code*, name, is_system
**permissions**: id, code*, name, group
**role_permissions**: role_id, permission_id (PK pair)
**user_roles**: user_id, role_id (PK pair)
**user_permission_overrides**: id, user_id, permission_id, allowed BOOLEAN, reason, granted_by (FK users)
**audit_log**: id, logged_at, user_id (nullable for system), action, entity_type, entity_id, old_json, new_json, reason, ctx_json (window/session info)

### Center
**branches**: id, code*, name, address, phone, is_main, is_active
**center_settings**: id (=1 singleton), name, trade_name, logo_ref (branding file), address, phones, email, website, license_no, tax_no, currency_code* (ISO 4217), currency_decimals, timezone*, academic_year, header_json, footer_json, cash_rules_json, created_at, updated_at

### Students
**students**: id, student_no* (per branch: branch_id+seq; `student_code` human key), branch_id FK, full_name, national_id (unique when present), birth_date, gender, phone, address, email, photo_ref, notes, status CHECK IN(active, frozen, archived), registered_at, created_at, updated_at
**guardians**: id, full_name, relation, phone, email, national_id, address, notes
**student_guardians**: student_id, guardian_id (PK pair), is_primary

### Teachers
**teachers**: id, teacher_no*, branch_id, full_name, phone, email, specialty, qualifications, address, hire_date, status CHECK IN(active, inactive, archived), notes, created_at

### Catalog
**subjects**: id, code*, name, description, is_active
**rooms**: id, branch_id, code*, name, capacity, description, status CHECK IN(available, reserved, maintenance, archived), notes

### Courses
**courses**: id, course_no*, branch_id, name, subject_id FK, description, start_date, end_date, default_time_json, teacher_id FK, room_id FK, expected_students, min_students, max_students, fee_minor, status CHECK IN(draft, registration_open, active, completed, suspended, cancelled, archived), is_private, created_by FK users, approved_by FK users NULL, approved_at, notes
**course_schedules**: id, course_id, weekday (1-7), start_time, end_time, room_id, teacher_id, effective_from, effective_to
**course_change_log**: id, course_id, changed_by, changed_at, action, before_json, after_json, reason

### Scheduling
**sessions**: id, course_id, source_schedule_id, seq_in_course, session_date, start_time, end_time, room_id, teacher_id, substitute_teacher_id NULL FK, status CHECK IN(scheduled, executed, cancelled, postponed, makeup), session_kind CHECK IN(regular, makeup, compensating), original_session_id FK sessions NULL, reschedule_to_id FK sessions NULL, reason, created_by, executed_at, cancelled_by, created_at, updated_at
**student_attendance**: id, session_id FK, student_id FK, status CHECK IN(present, absent, late, excused), late_minutes, notes, recorded_by FK users, recorded_at (UNIQUE(session_id, student_id))
**teacher_attendance**: id (per session), session_id FK, teacher_id, status CHECK IN(present, absent, late, excused), notes, recorded_by, recorded_at

### Enrollment
**enrollments**: id, branch_id, student_id, course_id, enrolled_date, base_fee_minor, discount_minor, waiver_minor, net_fee_minor, status CHECK IN(active, completed, withdrawn, frozen, transferred, cancelled), credit_applied_minor, notes, created_by, frozen_reason, transferred_to_enrollment_id FK, UNIQUE(student_id, course_id) (a student participates in a course once)
**payment_plans**: id, enrollment_id, total_minor, installment_count, policy_json
**installments**: id, payment_plan_id, seq, due_date, amount_minor, status CHECK IN(open, paid, partially_paid) — derived fields computed by the engine; no float.

### Finance — agreements
**financial_agreements**: id, course_id, version, status CHECK IN(draft, active, superseded), method CHECK IN(percent_of_agreed, percent_of_collected, percent_of_executed_sessions, flat_course, flat_session, flat_plus_percent), rate_bp (basis points x10000), center_rate_bp, flat_minor, per_session_minor, due_timing_json (accrual policy), conditions_json (cancel effect, makeup effect, substitute effect, refund policy rules, attendance weighting), snapshot_json (full frozen copy on activation), approved_by, approved_at
**agreement_amendments**: id, agreement_id, old_version, new_version, changes_json, reason, created_by, approved_by, approved_at
**student_credit_ledger**: id, student_id, ref_type CHECK IN(receipt, refund, settlement, transfer), ref_id, credit_minor (negative = consumed), lines managed by engine, occurred_at, note
**shortfall_decisions**: id, enrollment_id, student_id, course_id, uncollected_minor, reason, decision CHECK IN(center_full, teacher_full, partial_split, fixed_center, fixed_teacher, deferred, documented_exemption, exceptional), center_share_minor, teacher_share_minor, proposed_by, proposed_at, approved_by, approved_at, notes, attachments
**refund_requests**: id, enrollment_id, student_id, course_id, requested_date, requested_by, status CHECK IN(draft, submitted, approved, rejected, paid, cancelled), policy_snapshot_json, refundable_minor, breakdown_json (remaining sessions, attendance, admin fee, prior refunds), approver, approved_at, voucher_id FK
**settlements**: id, course_id, teacher_id, kind CHECK IN(course, teacher, center, shortfall, refunds, partial, final), period_from, period_to, total_minor, engine_snapshot_json, status CHECK IN(draft, pending_approval, approved, rejected), created_by, approved_by, approved_at, reversed_by_id FK (self-settlement reversal), notes

### Ledger
**accounts**: id, code*, name, kind CHECK IN(asset, liability, equity, revenue, expense), parent_id, is_postable, is_active
**journal_entries**: id, entry_no*, entry_date, memo, currency, total_debit_minor, total_credit_minor, status CHECK IN(draft, posted, reversed), source_type CHECK IN(receipt_voucher, payment_voucher, refund, settlement, expense, enroll_recognition, shortfall_decision, cash_closing, adjustment), source_id, posted_by, posted_at, reversal_of_id FK journal_entries
**journal_lines**: id, entry_id FK, account_id FK, debit_minor, credit_minor, line memo; CHECK (debit_minor >= 0 AND credit_minor >= 0 AND (debit_minor = 0 OR credit_minor = 0)) — a line has exactly one non-zero side
**accounting invariants**: CHECK(total_debit_minor = total_credit_minor) on entry; sum(lines.debit)=total_debit_minor asserted in Rust and by test; posted entries immutable (trigger `trg_no_edit_posted`).

### Vouchers
**voucher_sequences**: id, branch_id, kind CHECK IN(receipt_voucher, payment_voucher, journal_entry), fiscal_year, last_no — allocated inside the same transaction (gap-free under rollback).
**receipt_vouchers**: id, voucher_no*, branch_id, voucher_date, payer_name, student_id FK, enrollment_id FK, course_id FK, amount_minor, amount_words, method CHECK IN(cash, card, transfer, wallet), account_id FK (cash or bank), cash_register_id FK NULL, narrative, received_by, status CHECK IN(draft, posted, cancelled), posted_by, posted_at, cancelled_reason, journal_entry_id FK, idempotency_key*, print_count, last_printed_at
**payment_vouchers**: id, voucher_no*, branch_id, voucher_date, beneficiary_name, beneficiary_type CHECK IN(teacher, student/parent, vendor, other), teacher_id FK, student_id FK, enrollment_ref_id FK, expense_id FK, purpose, amount_minor, amount_words, method, account_id, cash_register_id, narrative, created_by, status CHECK IN(draft, pending_approval, approved, paid, cancelled), approved_by, approved_at, paid_by, journal_entry_id, idempotency_key*, source_type CHECK IN(refund, teacher_entitlement, expense, settlement, other), source_id

### Cash
**cash_registers**: id, branch_id, code*, name, status CHECK IN(active, closed), current_user_id, notes
**cash_movements**: id, register_id, kind CHECK IN(opening, receipt, payment, transfer_in, transfer_out, adjustment), direction CHECK IN(in, out), amount_minor, ref_type, ref_id (voucher id/transfer pair or closing), occurred_at, balance_after_minor, created_by
**register_closings**: id, register_id, opened_at, closed_at, opening_balance_minor, booked_balance_minor, counted_balance_minor, variance_minor, status CHECK IN(pending, approved, rejected), prepared_by, approved_by, approved_at, notes, journal_entry_id (variance posting)

### Expenses
**expense_categories**: id, code*, name, is_active
**expenses**: id, branch_id, category_id FK, expense_date, amount_minor, beneficiary, description, method CHECK IN(cash, card, transfer, wallet), account_id, status CHECK IN(draft, pending_approval, approved, paid), voucher_id FK, attachments, created_by, approved_by

### Platform
**attachments**: id, entity_type, entity_id, stored_path, display_name, mime, size_bytes, sha256, uploaded_by, created_at
**print_profiles**: id, code*, name, paper CHECK IN(a4…), orientation, margins_json, font_family, font_size_pt, show_header, show_footer, watermark_text, copies, is_default per doc_type
**document_print_log**: id, document_type, document_id, template_code, template_version, print_profile_id, copies, printed_by, printed_at, output_path, checksum, purpose CHECK IN(print, pdf, reprint)

### Reports
**saved_reports**: id, report_code (registry key), name, owner_id, config_json (columns, filters, sorting), is_shared, created_at

---

## 4. Data integrity summary

- Foreign keys enforced globally (`PRAGMA foreign_keys = ON`).
- Deletions: financial tables use `ON DELETE RESTRICT` on references; the app never hard-deletes financial records anyway (status flips + reversals only).
- CHECKs carry enum boundaries so bad states cannot be stored even from a service bug.
- SEEDED chart of accounts: 1000 Cash, 1100 Bank, 1200 Wallets Receivable, 1300 Students Receivable, 1400 Teacher Payables, 1500 Refund Liability, 2000 Deferred Revenue, 4000 Course Revenue, 4100 Center Revenue Share, 5000 Expenses (children by category), 6000 Refund Expense, 7000 Cash Variance.
- Number sequences (students/teachers/courses/vouchers/journal entries) allocated inside the performing transaction → rollback releases numbers without gaps under normal operation; cancelled vouchers keep their issued number.
