# REMAINING WORK PLAN — Education Center Management System

Status: Phases 1–10 backend complete (133 tests green). This plan closes the
Partial/Missing items identified in the audit (see final report). Execution rule
unchanged: **no phase starts before the previous phase's tests pass**, each
phase ends with a report.

## Phase 11 — High-priority backend gaps (target: 3–4 sessions)

### 11.1 Revenue recognition accrual (FR-9.4)
- **Module**: `core/src/finance/recognition.rs`
- **Work**: on enrollment approval post Dr Students Receivable / Cr Deferred Revenue (using net_fee_minor); on course start / session execution, per policy, recognize Cr Course Revenue (Dr Deferred Revenue). Configurable policy flag in agreement (`due_timing_json.recognition`).
- **Tests**: `fin019_revenue_recognition_accrual` (enrollment posts balanced entry; recognition moves deferred→revenue; ledger balanced).
- **Shell**: `recognize_revenue` command.

### 11.2 Course approval requires active agreement (FR-5.4)
- **Module**: `education::courses::set_course_status`
- **Work**: when transitioning `registration_open → active`, require an active `financial_agreements` row for the course; else `AppError::Conflict`. Maker-checker already present.
- **Tests**: `edu011_course_requires_agreement` (approve without agreement rejected; with agreement ok).
- **UI**: course detail shows agreement status.

### 11.3 Payment voucher — payout (FR-10.1)
- **Module**: `vouchers::payment_vouchers` (new) + `finance::ledger`
- **Work**: create payment voucher (teacher/student/vendor/other), status flow draft→pending_approval→approved→paid. On `paid`: post Dr Teacher Payable (or expense/refund) / Cr Cash; bounded by payable balance (over-pay rejected → `voucher.overpaid_source`).
- **Tests**: `vch009_payment_voucher_lifecycle`, `vch010_overpay_blocked`.
- **Shell**: `payment_vouchers_create/approve/pay/cancel`.

### 11.4 Payment methods (FR-10.3)
- **Work**: receipt/payment vouchers accept `method ∈ {cash, card, transfer, wallet}`; map to the correct cash/bank account; bank account selector in UI.
- **Tests**: `vch011_payment_method_account_mapping`.

### 11.5 Disable user (FR-2.6)
- **Module**: `auth::service`
- **Work**: `disable_user(token, target, reason)` sets `is_active=0`; audit `user_disabled`; login blocked. No deletion.
- **Tests**: `sec007_disable_user_blocks_login`.
- **Shell**: `auth_disable_user`. UI: users page.

### 11.6 Free-space check (FR-1.1)
- **Module**: `config::data_dir`
- **Work**: `test_write_permissions` + free-space probe (statvfs) before accepting a data dir; reject if < threshold.
- **Tests**: `cfg001_free_space_check`.

### 11.7 Data-directory change (FR-1.4)
- **Module**: `config` + `database`
- **Work**: command `change_data_dir(new_path)`: close pool, backup, copy, verify integrity, update settings, reopen. Source untouched until success.
- **Tests**: `cfg002_data_dir_change_roundtrip`.
- **UI**: settings page.

## Phase 12 — Medium-priority backend gaps

### 12.1 Payment plans & installments (FR-7.4)
- **Module**: `education::payment_plans`
- **Work**: create plan (total, count, policy), generate installments with due dates; status open/partially_paid/paid; partial payment updates installment.
- **Tests**: `edu012_installment_partial_payment`, `edu013_installment_status_transitions`.

### 12.2 Pay-on-behalf / one payment → several enrollments (FR-7.5)
- **Module**: `finance::payments`
- **Work**: `PaymentAllocation` already exists; persist each allocation as a row in a new `payment_applications` table (or use receipt_voucher.enrollment_id + a side table). Add `payment_applications` migration.
- **Tests**: `fin020_multi_enrollment_payment`, `fin021_pay_on_behalf`.

### 12.3 Refund workflow (FR-8.4)
- **Module**: `finance::refunds` (DB workflow)
- **Work**: `refund_requests` table already exists. request (engine computes proposed amount + snapshot) → approve (maker-checker) → payout voucher (source_type=refund, bounded) → posting. Cutoff date + manager exception bypass with reason.
- **Tests**: `fin022_refund_request_to_payout`, `fin023_refund_cutoff_and_exception`.
- **Shell**: `refunds_request/approve/payout`.

### 12.4 Transfers between registers (FR-10.5)
- **Module**: `vouchers::cash`
- **Work**: `transfer_between_registers(from, to, amount)` creates paired `transfer_out`/`transfer_in` movements with matching ref_id; balances net to zero.
- **Tests**: `vch012_transfer_pair_balances`.

### 12.5 Cash receipts require open register (FR-10.6)
- **Module**: `vouchers::cash` + settings
- **Work**: center setting `cash_receipts_require_open_register`; if on, `record_payment` with method=cash requires an open register for the branch; else reject.
- **Tests**: `vch013_cash_requires_open_register`.

### 12.6 Expense → payment voucher (FR-10.7)
- **Module**: `vouchers::expenses`
- **Work**: `mark_expense_paid` creates a payment voucher (source_type=expense, source_id=expense_id) and posts Dr Expense / Cr Cash.
- **Tests**: `vch014_expense_paid_creates_voucher`.

### 12.7 Teacher data isolation (FR-4.4)
- **Module**: `people::teachers` + command layer
- **Work**: helper `scope_for_teacher(actor, pool)` filters lists (courses/sessions/enrollments/attendance) to those where teacher_id = actor's linked teacher. Add `users.teacher_id` FK (migration) or a mapping table.
- **Tests**: `sec008_teacher_scope_enforced`.

### 12.8 Guardian & credit balance (FR-3.1/3.6/3.7)
- **Module**: `people::guardians`, `people::student_credit`
- **Work**: guardian CRUD (link to many students); student credit ledger already exists; add `apply_credit_to_enrollment` and `refund_to_credit`.
- **Tests**: `edu013_guardian_link`, `fin024_credit_apply_and_refund`.

### 12.9 Room/teacher change log (FR-6.5)
- **Module**: `education::sessions`
- **Work**: on room/teacher/substitute change, insert `course_change_log` row (who/why/when/before/after).
- **Tests**: `edu014_change_log_written`.

### 12.10 Attendance policy in agreement (FR-6.7)
- **Module**: `education::attendance` + `finance::entitlement`
- **Work**: agreement `conditions_json.cancelled_effects` controls whether cancelled sessions count; makeup inherits weight (already linked).
- **Tests**: `edu015_attendance_weighting`.

## Phase 13 — Reports, Excel, charts

### 13.1 Report registry (40+ reports) (FR-11.1)
- **Module**: `reports::registry`
- **Work**: implement each report as a registered definition (code, columns, filters, SQL template via safe builder) producing paginated rows. Categories: administrative, financial, control.
- **Tests**: `rpt003_report_registry_count` (≥ 40), `rpt004_each_report_runs`.

### 13.2 Report builder filters/sort/pagination/saved (FR-11.3)
- **Module**: `reports`
- **Work**: extend saved_reports with `is_shared`, `owner_id`; saved filters; favorites; pagination + sort params.
- **Tests**: `rpt005_saved_filters_and_favorites`.

### 13.3 Excel export (FR-11.4)
- **Dep**: add `rust_xlsxwriter` to workspace.
- **Module**: `reports::export`
- **Work**: `export_excel(report_code, params) -> Vec<u8>` (streaming writer).
- **Tests**: `rpt006_excel_export_valid`.

### 13.4 PDF export via printing (FR-11.4)
- **Work**: reuse `printing::render_pdf` with report template; preview via `render_png_first_page`.
- **Tests**: `prn009_report_pdf_preview_match`.

### 13.5 Charts (FR-14.1)
- **Module**: frontend `web/src/lib/charts` (ECharts) + backend KPI endpoints.
- **Work**: dashboard charts: revenue/collection/enrollment/attendance; ECharts wired in `+page.svelte` (dashboard).

### 13.6 Dashboard KPIs full (FR-14.1)
- **Module**: `dashboard`
- **Work**: extend `compute_kpis` with today's sessions, today's attendance, due fees, teacher payables, cash balance, courses needing settlement, pending refunds, pending shortfall cases.
- **Tests**: `dash002_kpis_full`, `dash003_kpis_reconcile_to_ledger`.

## Phase 14 — Printing deepening

### 14.1 Template versioning (FR-12.5)
- **Module**: `printing`
- **Work**: `document_print_log` already exists; store `template_version` + payload snapshot; reprint uses stored snapshot (identical output).
- **Tests**: `prn010_reprint_exact_history`.

### 14.2 Customizable header/footer (FR-12.6)
- **Module**: `printing` + `print_profiles`
- **Work**: template params (center data, logo, custom texts, signatures, page x/y, print date) injected into Typst template.
- **Tests**: `prn011_header_footer_params`.

### 14.3 True preview (FR-12.4)
- **Work**: already `render_png_first_page` from same PDF; add test verifying raster hash equality.
- **Tests**: `prn012_preview_matches_pdf_raster`.

## Phase 15 — Backup deepening + automation

### 15.1 Auto backups (FR-13.1)
- **Module**: `backup::scheduler`
- **Work**: daily/weekly/monthly + pre-upgrade + pre-migration triggers; runs in background task.
- **Tests**: `bak011_auto_schedule`.

### 15.2 Destinations (FR-13.2)
- **Work**: support external/USB/network/synced folders as destination path (already path-based; add validation + free-space check).
- **Tests**: `bak012_destination_validation`.

### 15.3 Restore wizard UI (FR-13.4)
- **Module**: frontend + shell
- **Work**: multi-step wizard (choose file, verify, contents, backup current, restore, integrity check, restart).

## Phase 16 — UI deepening (heavy)

### 16.1 Enrollment flow UI
- New enrollment form (student/course selection, fee/discount/waiver), list, status transitions.

### 16.2 Agreement approval UI
- Draft → approve (maker-checker), amendment with reason, version history.

### 16.3 Shortfall decision UI
- Propose case, decision type selector, share inputs, reason; postings audit.

### 16.4 Refund workflow UI
- Request → proposed computation → approve → payout.

### 16.5 Settlement UI
- Prepare (scope/period/snapshot), approve/reject, reverse.

### 16.6 Printing UI
- Template management, preview dialog, reprint, cancel-with-reversal.

### 16.7 Backup/restore wizard UI
- As in 15.3.

### 16.8 Global quick search (FR-3.3)
- Cross-entity search by name/number/phone.

## Phase 17 — E2E + packaging (Phase 10 completion)

### 17.1 E2E smoke (tauri-driver / Playwright)
- Login → enroll → collect → print → backup → restore on clean machine.

### 17.2 Packaging validation
- Build .deb/.appimage/.msi/.dmg; install on clean VM; run full suite.

### 17.3 Final audit
- Re-run all tests; verify 0 warnings on critical modules; update docs.

---

## Suggested execution order
11 → 12 → 13 → 14 → 15 → 16 → 17.

Each phase: implement → tests → report → commit. No phase starts before previous tests green.

## Estimated test growth
- Phase 11: +6 tests (139)
- Phase 12: +9 tests (148)
- Phase 13: +6 tests (154)
- Phase 14: +3 tests (157)
- Phase 15: +2 tests (159)
- Phase 16: 0 new (UI)
- Phase 17: E2E (separate suite)

## Open decisions (need your input)
1. Should I start Phase 11 now, or adjust priorities?
2. Excel export (rust_xlsxwriter) — add dependency now or later?
3. E2E framework preference: tauri-driver (WebDriver) vs Playwright?
4. Should guardian/credit (12.8) be deferred to a "V2" scope?
EOF
echo "written"