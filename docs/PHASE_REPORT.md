# تقرير المراحل — Education Center Management System

## الحالة الحالية
- المراحل 1–15 (backend) منجزة.
- 166 اختباراً ناجحاً في `ec-core`.
- بناء `ec-app` (Tauri 2) ناجح.
- Migrations: 5.

## what was implemented
- Data directory management, first-run wizard, portable mode, change data dir.
- Auth (Argon2id), RBAC, audit log, disable user.
- People: students, teachers, subjects, rooms, guardians, credit.
- Education: courses, schedules, sessions, attendance, enrollments, agreements, payment plans.
- Finance: ledger (double-entry), entitlements (6 methods), payments, shortfalls, refunds, settlements, revenue recognition.
- Vouchers: receipts, payment vouchers, cash registers, expenses.
- Reports: registry (≥40), saved reports, filters/favorites, Excel export.
- Printing: Typst A4 PDF, RTL, fonts, preview raster.
- Backup/restore: VACUUM INTO, checksums, retention, portable, auto-schedule, destination validation.
- Dashboard: KPIs (students, courses, enrollments, collections, receivables, cash, pending refunds/shortfalls).

## Remaining (Phase 16/17)
- UI deepening (enrollment/agreement/shortfall/refund/settlement/printing/backup flows).
- E2E + packaging validation.
