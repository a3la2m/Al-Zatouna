# FINANCIAL ENGINE — Design Document
## Education Center Management System (Phase 1)

The financial engine is an independent, testable Rust service (`modules::finance`). It owns all money math, entitlement accrual, shortfall allocation, refund computation and settlement rules. The frontend never computes money.

Terms:
- **Minor unit**: integer smallest currency fraction (currency code + decimals in center settings; default ILS, 2 decimals).
- **Basis point (bp)**: percent × 100 (10000 bp = 100%); all teacher/center shares stored as exact integers.
- **Uncollected / shortfall**: net fee (agreement snapshot scope) minus collected; it is NOT a loss or expense until a decision is approved.
- **Decision**: documented allocation of the approved shortfall between center and teacher; never a cash collection.

---

## 1. Financial agreement per course

One active agreement per course (versioned). On course approval, the snapshot is frozen — all later calculations reference the snapshot, never the editable source. Changes = Amendment (new version, reason, approver; never retroactive).

### 1.1 Teacher entitlement methods (per course)

| Method | Basis | Formula |
|--------|-------|---------|
| `percent_of_agreed` | agreed course fee portion credited to that enrollment (net fee after discount/waiver only if policy says; default: base fee, ignoring waiver per policy flag) | ∑ basis × teacher_bp / 10000 |
| `percent_of_collected` | amounts actually allocated to this course's enrollments (receipts applied) | ∑ allocated × teacher_bp / 10000 |
| `percent_of_executed_sessions` | each executed session has a per-session value (course fee ÷ session count by default, or an explicit per-session base in the agreement) | ∑ executed sessions × session_value × teacher_bp/10000 |
| `flat_course` | single agreed amount for the course | flat_minor (+ release schedule) |
| `flat_session` | fixed amount per executed session | count(executed sessions) × per_session_minor |
| `flat_plus_percent` | a flat base part + a percent part | flat_minor + ∑ basis × teacher_bp/10000 |

Conventions:
- Rounding: half-up at the final amount only (never during intermediate sums).
- All computed amounts are provenance-tracked: engine writes `engine_snapshot_json` containing inputs and outputs, so a report can be regenerated and compared.
- Substitute teacher: split by agreement rules (default: full share to substitute for the executed session, else split per `conditions_json.substitute_split`); recorded per-session on the entitlement lines.
- Cancelled sessions: excluded by default; per agreement, cancelled-with-school-cause vs cancelled-without-execution are configurable (`conditions_json.cancelled_effects`).
- Makeups: the makeup session inherits the original session's weight; original makeup-source is neutralized in the same period.
- Attendance weighting: optional per-course weighting factor per student presence (default: 1).
- Due timing: accrual on execution; payment scheduling (e.g. monthly) via `due_timing_json`.

### 1.2 Accrual and payment schedule

- The engine produces `entitlement accruals` (teacher payables) as ledger postings and payable records: teacher entitlements → account 1400 (Teacher Payables).
- Payment of an accrual proceeds via a `payment_voucher` with `source_type = teacher_entitlement`, bounded by the payable balance (over-payment is rejected).
- Partial payout allowed; engine tracks per-accrual paid amount.

## 2. Shortfall decisions

- Trigger: any enrollment whose net fee (agreement scope) minus collected remains open at course settlement time (or manual review by an authorized user).
- The engine only **presents** the case (student, course, uncollected, reason candidates, timeline); a qualified user records a Decision from the closed set:
  `center_full | teacher_full | partial_split (center_bp + teacher_bp = 10000) | fixed_center | fixed_teacher | deferred | documented_exemption | exceptional`
- Chosen shares are integer-exact; fixed amounts may never exceed the uncollected total; any residual not covered by fixed center/teacher amounts follows an explicit residual rule in the decision inputs (default: center absorbs the residual).
- Consequences:
  1. `documented_exemption` or `center_full` → the uncollected student receivable is written off against center revenue/expense accounts by a posted entry (a revenue write-off, not a collection).
  2. `teacher_full` or `teacher_share>0` → a teacher liability adjustment posting (teacher payable reduced/adjusted with explicit reason); it is a booking, NEVER a cash movement or a voucher.
  3. `deferred` → no posting; the case remains open and appears on control reports.
- A shortfall decision always links to its enrollment and is separate from the student's account balance state; the student's debt is cleared per case decision only.

## 3. Refund policies per course

Policy fields (inside the agreement's frozen policy or its standalone on the later Amendment):
- `before_start`: full refund allowed (optionally minus admin fee).
- `after_start`: refund = base × (remaining_sessions_weight / total_weight) − admin_fee − prior_posted_refunds; remaining weight respects the policy's session weighting and the student's attendance-based factors.
- `cutoff_date` on which refunds are no longer allowed; `manager_exception` bypass with documented reason.
- Cap: refund ≤ the paid amount allocated to the enrollment (per engine records); negative results are clamped to zero and tagged.

Refund workflow (docs/WORKFLOWS.md §R): request → proposed calculation (engine, stored snapshot) → review → approval → payout voucher (source_type=refund; bounded by the approved amount) → posting and balance update. Approval without payout remains an open liability (visible on control report).

## 4. Settlements ('التسويات')

- Kinds: course / teacher / center / shortfall / refunds / partial / final.
- A settlement is an **engineering document**: the engine produces an immutable snapshot `engine_snapshot_json` of all inputs and computed values for its scope and period.
- Approved settlements are immutable; corrections are made by a reversal settlement (linked `reversed_by_id`) or a new adjustment — the original is never edited.
- Settlement controls: status may move from `pending_approval` to `approved` or `rejected` by a permitted user (maker–checker rule applies).
- "Course unsettled" control report reads rows from enrollments/entitlements whose allotment has moved since the last approved settlement.

## 5. Ledger integration (docs/ARCHITECTURE.md §9 basis)

Journal posting happens inside the same transaction as the triggering financial action. Reference matrix (V1):

| Source document | Account flow |
|-----------------|--------------|
| Enrollment (on approval, revenue recognition per policy) | Dr Students Receivable / Cr Deferred Revenue → (on course start / session execution, per policy) Cr Course Revenue recognition |
| Receipt voucher | Dr Cash / Bank / Wallet (account_id) / Cr Students Receivable — allocations across several enrollments are captured as separate application lines referenced by the voucher |
| Payment voucher (teacher entitlement) | Dr Teacher Payable / Cr Cash |
| Payment voucher (refund) | Dr Refund Expense & Reduced Deferred Revenue (per policy snapshot) / Cr Cash |
| Expense voucher | Dr Expense (category account) / Cr Cash |
| Shortfall decision | See §2 consequences postings |
| Register closing variance | Dr/Cr Cash Variance (7000) to true-up the books |
| Voucher cancellation | Reversal entry linked to the original (status reverted on the header) |

Invariants (asserted in service code and by database triggers, and validated per-commit in tests):
- entry.total_debit_minor == entry.total_credit_minor
- validated: non-zero line debit xor credit per line
- source_type + source_id reachable and valid
- reversal entries: total equal to original entry, opposite sides, linked by reversal_of_id.
