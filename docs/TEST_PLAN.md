# TEST PLAN — Education Center Management System (Phase 1)

Principle (project rule): **a feature is only "done" when its tests run and pass**. Each phase ships with its tests green before the next phase starts. Tests run automatically via `cargo test` (Rust) and a lightweight Vitest + component test pass (frontend), plus testdata scripts.

---

## 1. Test infrastructure

- **Rust unit tests**: pure business services (money math, engine computations, words generator).
- **Rust integration tests**: run against a temp SQLite file created fresh from migrations; helpers: `tests/common/mod.rs` (create_db(), seed_center(), login_as(role), tx helpers).
- **Property tests** (proptest) for money math and engine rounding rules.
- **API-level Tauri tests** (`tauri-driver` WebDriver) — Phase 10: login/enroll/collect/print journeys.
- **Frontend tests**: Vitest, unit tests for IPC wrappers, store reducers, RTL rendering of forms (jsdom), Playwright smoke test for the desktop shell in Phase 10.
- CI scripts (`scripts/run-tests.ps1|sh`) must run all suites and fail on warnings escalation for critical modules.

## 2. Database tests (Phases 1–3 and ongoing)

| Test | Points covered | Phase gate |
|------|----------------|------------|
| `db001_create_fresh_db` | migrations idempotent (fresh run on empty file), seeds present (accounts, roles, permissions, print profiles) | P2 |
| `db002_migration_chain` | apply all migrations in order; checksum verification; dry-run on restored fixture DB | P2+ |
| `db003_foreign_keys_enforced` | FK violation attempts rejected on each module edge table | P2 |
| `db004_wal_journal_mode` | `journal_mode` returns WAL; checkpoint does not lose data | P2 |
| `db005_transaction_atomicity` | rollback on mid-command failure leaves no partial writes; no zombie rows | P2 |
| `db006_posted_entry_immutable` | UPDATE/DELETE on posted journal entries is aborted | P6 |
| `db007_integrity_check_after_ops` | integrity_check ok after stress-run (~500 mixed ops) + after restore | P9 |
| `db008_busy_handling` | concurrent second write pool gets busy-timeout + single retry path not corrupting state | P2 |
| `db009_money_no_float` | schema scan: no REAL/FLOAT/DOUBLE columns on money-bearing columns | P2 |
| `db010_sequences_gap_free_on_rollback` | voucher minting inside failing transaction doesn't leak numbers | P7 |
| `db011_data_dir_relocation` | DB opens from new path after relocation; old path untouched | P9 |

## 3. Education domain tests (Phases 4–5)

- `edu001_single_student_course` — private course: one enrollment succeeds; second student on max=1 rejected.
- `edu002_multi_students_course` — capacity rules min/max respected on enrollment.
- `edu003_late_enrollment` — join mid-course: engine's eligibility basis computed.
- `edu004_withdraw_freeze` — statuses and downstream effects (finance untouched; controls show properly).
- `edu005_sessions_status_set` — scheduled→executed/cancelled/postponed/makeup transitions legal; invalid transitions rejected.
- `edu006_room_clash` — overlapping time, same room → rejected at command and (defense) DB check.
- `edu007_teacher_clash` — overlapping time, same teacher → rejected; substitute allowed.
- `edu008_makeup_session_weight` — original neutralized, makeup inherits weight in the engine basis.
- `edu009_attendance_record` — once per (session, student); late minutes recorded; teacher attendance once per session.
- `edu010_attendance_security` — teacher sees own sessions only via API (enforced at command layer).

## 4. Financial engine tests (Phase 6) — yardstick of the system

- `fin001_full_payment` — all allocations apply; statement balanced; posting correct.
- `fin002_partial_payment` — installment-by-installment allocation order & partial-instance rows.
- `fin003_no_payment` — due tracked; shortfall candidate at settlement.
- `fin004_discount_waiver` — reduces due amount; waiver ≠ shortfall; distinct ledger treatments.
- `fin005_entitlement_methods` — six methods each produce the documented expected amounts with exact rounding (property test asserts round-once-only).
- `fin006_method_switch_between_courses` — per-course methods independent; course A unaffected by course B.
- `fin007_agreement_snapshot_immutable` — historical accruals recompute IDENTICAL after amendment; amendments affect only future events.
- `fin008_shortfall_decision_matrix` — all 8 decision types produce documented allocations; teacher debit only per explicit decision; no auto posting when `deferred`.
- `fin009_residual_of_fixed_over_under` — fixed stays ≤ uncollected with residual ownership policy assert.
- `fin010_refund_policies` — each policy branch produces expected amount ≤ paid; admin fee honored; cutoff blocks; exception path logs.
- `fin011_refund_partial_and_full` — breakdown reconciliation against due/collected.
- `fin012_entitlement_partial_payout` — partial payout tracked; over-pay blocked (`voucher.overpaid_source` case).
- `fin013_duplicate_collect_blocked` — idempotency duplicates; double-click simulation; concurrent post rejected by unique key.
- `fin014_voucher_cancel_reversal` — cancel produces reversal entry; totals unchanged in books.
- `fin015_ledger_always_balanced` — invariant over randomized op sequences (1000 txs); ledger sums zero every step.
- `fin016_computed_student_receivable_matches_ledger` — domain-level aggregate equals ledger balance.
- `fin017_settlement_final_snapshot` — approved settlements immutable; reversal path reproducible; engine_snapshot persisted.
- `fin018_teacher_payout_no_deduction_with_no_decision` — payable without decision cannot be tapped by other flows.

## 5. Voucher, cash & expense tests (Phase 7)

- `vch001_seq_unique_minting` per branch and kind; `vch002_reprint_no_new_number`; `vch003_cancel_keeps_number`; `vch004_cash_requires_open_register` (and off via a setting); `vch005_transfer_pair_balances`; `vch006_closing_variance_posted`; `vch007_no_expense_without_approval`; `vch008_amount_words_arabic` (property-test on thousands/millions/compound forms).

## 6. Printing & PDF tests (Phase 8)

- `prn001_pdf_a4_geometry` — produced pages are exactly 595x842 pt (±0.5).
- `prn002_rtl_and_shaping` — sample Arabic text renders with correct direction (visual compare via fixture hash of engine output).
- `prn003_font_embedded` — PDF contains an embedded Arabic font (no missing-glyph fallback found in text draw ops).
- `prn004_table_header_repeat` — multipage report contains repeated table headers with repeat flag by engine sanity-check and visual fixture.
- `prn005_no_split_important_rows` — row heights/pages guard; row can't cross.
- `prn006_preview_matches_pdf` — preview raster hash matches PDF-render raster from the SAME file.
- `prn007_reprint_exact_history` — reprinting an old document produces identical template-v-version output; payload snapshot preserved.
- `prn008_page_numbers_and_totals` — page X/Y, print date, title present on every page.

## 7. Backup / restore / migration tests (Phase 9)

- `bak001_snapshot_under_transactions` — a copy taken mid-stress is always a consistent snapshot; integrity check ok.
- `bak002_checksums_match_manifest` — all SHA-256 values verified at package creation; per-file list equality checked on restore.
- `bak003_restore_roundtrip` — restore to clean environment; row-count spot checks on seeded fixtures equal pre-backup.
- `bak004_restore_rejects_corruption` — manipulated/truncated files fail early with clear report.
- `bak005_version_gates` — downgrade/unsupported schema versions blocked with clear message.
- `bak006_safety_copy_before_restore` — a `pre_restore` package is created automatically.
- `bak007_cloud_sync_folder_destination` — package-based destination in a Google Drive/OneDrive-like folder works (no live DB inside).
- `bak008_retention_prune` — per-type retention prunes correctly; pre_upgrade never pruned.
- `bak009_portable_mode_transfer` — moving bundle to a second location works; remote path expectations validated.
- `bak010_disk_full_mid_run` — space-exhaustion leaves a valid partial state (no corrupted main DB); friendly error, retry possible.

## 8. Security tests

- `sec001_no_incorrect_permissions` — negative controls: every sensitive command attempt by an unauthorized role is rejected and audited; matrix assertion test per role.
- `sec002_maker_checker_gates` — self-approval blocked (incl. override rules); audit row written.
- `sec003_password_hashing` — stored hashes verified to be Argon2id and we can never reveal passwords.
- `sec004_path_traversal` — attachment path manipulation strings rejected; canonical paths inside `<DATA>`.
- `sec005_mime_and_size_validation` — executable upload attempts rejected.
- `sec006_audit_coverage` — event list from SECURITY §6 produces entries; audit chain immutable via triggers.

## 9. Dashboard & report integrity tests

- `dash001_kpis_correct` — dashboard numbers reconcile to domain aggregates on a seeded demo dataset.
- `rpt001_permissions_respected` — teacher/read-only scopes.
- `rpt002_saved_report_builder` — column/filter selections persisted; invalid SQL injection attempts impossible (no raw SQL accepted by design; registry rejects unknown codes).

## 10. Phase → gate matrix

| Phase | Tests required green (minimum) |
|-------|-------------------------------|
| 2 | db001–db010 (+ wizard helper tests) |
| 3 | sec001–006 + auth tests |
| 4 | edu001–010 partial (domain CRUD), plus fixture-driven reports |
| 5 | edu001–010 full |
| 6 | fin001–018 + relevant db tests |
| 7 | vch001–008 |
| 8 | prn001–008 |
| 9 | bak001–010 + db011 |
| 10 | dashboard, full stage suite, smoke E2E, packaging/docker-less install validation |

A reported phase-end gate includes a test-run summary (counts passed/failed/skipped and any known risks).
