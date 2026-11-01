# API — Tauri command surface (internal contract)
## Education Center Management System (Phase 1)

- The frontend communicates exclusively through Tauri commands defined below. No direct SQL anywhere in the frontend.
- Error contract (serialized `AppError`):
  ```json
  {
    "code": "conflict.room_clash",           // stable dotted code
    "message": "…Arabic user message…",
    "field": "start_time",                   // optional, for form-level errors
    "details": { "conflicting_session_id": 42 } // optional
  }
  ```
- List queries share a `PagedQuery { page, page_size, sort?, filters?, search? }` → `Paged<T> { items, total, page, page_size }`.
- Mutating financial commands accept `idempotency_key: String` when applicable.
- Names are grouped by module: `auth_*`, `students_*`, … meaning `<module>_<action>` in snake_case; the list below is the designed surface and will be kept in sync as phases implement it.

### Session / auth
- `auth_login(username, password) -> SessionUser`
- `auth_logout()`, `auth_change_password(old, new)`, `auth_session() -> SessionUser | null`
- `auth_set_manual_unlock()` — inactivity lock re-entry.

### Users & permissions (admin)
- `users_list(PagedQuery) -> Paged<UserDto>`
- `users_create(dto) -> UserDto`, `users_update(id, dto)`, `users_set_active(id, active, reason)`
- `users_reset_password(id, new_password, reason)` (admin, audited)
- `roles_list()`, `role_set_permissions(role_id, permission_codes)`
- `user_role_assign(id, role_codes)`, `user_permission_overrides(user_id, overrides, reason)`
- `permissions_catalog() -> Group[]`

### Center & branches
- `center_get() -> CenterSettings`, `center_update(dto)` (permission settings.manage)
- `branches_list/create/update/disable`

### Students & guardians
- `students_list(PagedQuery) -> Paged<StudentDto>`
- `students_get(id) -> StudentProfile` (enrollments, credit balance, recent payments summary)
- `students_create/edit/archive(id)/freeze(id, reason)`
- `students_attach_file(id, path|file) -> AttachmentDto`; `attachments_list(entity_type, entity_id)`; `attachments_delete(id)`
- `guardians_link(student_id, guardian_id, is_primary)`; `guardians_create(dto)`

### Teachers
- `teachers_list/get/create/edit/archive`
- `teachers_schedule(id)`; `teacher_statement(id, period) -> TeacherStatementDto`

### Subjects, rooms, courses
- `subjects_list/create/edit/disable`
- `rooms_list/create/edit/status`
- `courses_list/get/create/edit/change_status(id, status, reason)`
- `courses_approve(id, agreement_dto)` — sets snapshot
- `agreements_get(course_id) -> AgreementDto (active + history)`
- `agreements_amend(course_id, new_terms, reason)` — no retroactive changes
- `classrooms_occupancy(period)` (also available as a registered report)

### Scheduling, sessions, attendance
- `schedule_create(course_id, weekly_rules[])`; `schedule_edit/delete(rule_id)`
- `sessions_generate_preview(course_id, range) -> SessionPlan[]`
- `sessions_list(course_id, range)`
- `sessions_change(room_id/teacher_id/session_date, reason)` (validates clashes)
- `sessions_add_makeup(original_session_id, when, room, teacher)`
- `sessions_set_status(id, status, reason)` (cancel/postpone/executed)
- `attendance_record(session_id, rows[])`; `attendance_get(session_id)`
- `teacher_attendance_record(session_id, dto)`
- `conflicts_check(scope) -> Clash[]` (used in wizard preview)

### Enrollment & payments
- `enrollments_create(course_id, student_id, fee_terms)`; `enrollments_get(id)`
- `enrollments_change_status(id, status, reason)` (withdraw/freeze/transfer/cancel)
- `enrollments_list(PagedQuery)`
- `payment_plans_get/set(enrollment_id, installments[])`
- `payment_record(enrollment_ids[], amount, method, date, payer, notes, idempotency_key) -> AllocationDto[]` — auto allocation engine; supports on-behalf payments and cross-enrollment amounts.
- `payments_list(enrollment_id|student_id|course_id)`; refund flows are listed under Finance engine endpoints.
- `student_statement(student_id, range) -> StatementDto`
- `student_credit_get(student_id)`; `student_credit_apply(enrollment_id, amount)`

### Finance engine endpoints
- `finance_agreement_preview(course_id, dto) -> ComputedTerms` (what-if)
- `entitlements_run(course_id, period)` (admin accrual trigger, background task, permission agreements.manage)
- `entitlements_list(teacher_id|course_id, period)`
- `shortfall_cases_list(status filter)`; `shortfall_propose(enrollment_id)`; `shortfall_decide(case_id, decision_dto)`; `shortfall_defer(case_id, reason)`
- `refund_request(enrollment_id)`; `refund_compute(request_id)`; `refund_approve(request_id, idempotency_key)`; `refund_payout(request_id, method, account_id)`
- `settlements_prepare(scope_dto)` -> draft snapshot; `settlements_approve(id)`; `settlements_reverse(id, reason)`; `settlements_list(...)`

### Vouchers
- `voucher_receipt_create(dto, idempotency_key) -> ReceiptVoucherDto (draft)`
- `voucher_receipt_post(id)` — validates open register (if required), allocates payment, posts ledger
- `voucher_receipt_cancel(id, reason)` (reversal entry)
- `voucher_receipt_reprint(id, profile)` — same number; increments print_count
- `voucher_receipt_get(id)`, `voucher_receipts_list(PagedQuery)`
- `voucher_payment_*` mirror set with `approve` + `pay` + `cancel`

### Cash register
- `register_list()/create/edit(status)`; `register_open(id, opening_amount)`; `register_close(id, counted_amount) -> ClosingDraft`
- `register_approve_close(closing_id)`; `register_movements(register_id, range)`
- `register_transfer(from, to, amount, idempotency_key)`
- `register_balance(id)`; `register_closings_list(...)`

### Expenses
- `expenses_list(PagedQuery)`; `expenses_create(dto)`; `expenses_edit(draft)`; `expenses_submit(id) (pending)`
- `expenses_approve(id)` → creates payment voucher; `expenses_link_voucher(expense_id, voucher_id)`; `expense_categories_manage(...)`

### Reports & builder
- `reports_catalog() -> ReportDef[]` (allow-list of defined reports, params schema)
- `reports_run(code, params, columns) -> ReportData (paged/streamed)` — NOT raw SQL by design; validated against the registry only
- `reports_export(code, params, format pdf|xlsx)` (Background + progress event; returns stored path under `<DATA>/exports`)
- `reports_save_config(cfg)` per user; `reports_list_saved()`; `reports_apply(id)`
- `dashboards_kpis() -> KpiSet`; `dashboards_series(range) -> SeriesDto`

### Printing
- `print_preview(document_type, id, profile_code) -> pdf bytes` (cached versioned output)
- `print_execute(document_type, id, profile_code, copies)` (spooling path + log)
- `print_profiles_list/get/upsert` (permission print.profiles.manage)
- `document_print_log_list(doc_type, id)`

### Backup & data directory
- `backup_run_now(run_type, destination)` (Background + progress)
- `backup_settings_get/set`; `backup_list(destination)`; `backup_verify(package_path)`
- `restore_begin(package_path) -> VerificationReport`; `restore_execute()` (safety-copy + steps, progress)
- `datadir_status()`; `datadir_change(new_path)` (Background via §3 safe-migration)

### Maintenance & audit
- `audit_list(PagedQuery, event_filter)` (permission audit.view)
- `db_integrity_check()`; `db_wal_checkpoint()`; `db_vacuum_setting()`; `app_settings_get/set`
- `jobs_cancel(job_id)`; `jobs_active()`
