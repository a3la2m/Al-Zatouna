//! Payments / receipt collection (Phase 6).

use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::AppResult;
use crate::finance::ledger::{post_entry, JournalLine, NewJournalEntry};
use crate::people::context::{log_audit, Actor};

#[derive(Debug, Serialize, Clone)]
pub struct PaymentAllocation {
    pub enrollment_id: i64,
    pub amount_minor: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct PaymentResult {
    pub voucher_id: i64,
    pub journal_entry_id: i64,
    pub total_minor: i64,
}

/// Record a payment (receipt) with idempotency key. Creates a receipt
/// voucher row + journal entry (Dr Cash / Cr Students Receivable).
pub async fn record_payment(
    pool: &SqlitePool,
    actor: &Actor,
    branch_id: i64,
    cash_account_id: i64,
    receivable_account_id: i64,
    idempotency_key: &str,
    allocations: &[PaymentAllocation],
    memo: &str,
) -> AppResult<PaymentResult> {
    actor.require("payments.record")?;
    if idempotency_key.trim().is_empty() {
        return Err(AppError::validation("مفتاح 멱등ية مطلوب"));
    }
    // idempotency check
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM receipt_vouchers WHERE idempotency_key = ?",
    )
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await?;
    if let Some(vid) = existing {
        // return the prior result's journal entry
        let je: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM journal_entries WHERE source_type='receipt_voucher' AND source_id=?",
        )
        .bind(vid)
        .fetch_optional(pool)
        .await?;
        return Ok(PaymentResult {
            voucher_id: vid,
            journal_entry_id: je.unwrap_or(0),
            total_minor: 0,
        });
    }

    let total: i64 = allocations.iter().map(|a| a.amount_minor).sum();
    if total <= 0 {
        return Err(AppError::validation("المبلغ يجب أن يكون أكبر من صفر"));
    }
    for a in allocations {
        if a.amount_minor < 0 {
            return Err(AppError::validation("مبالغ سالبة"));
        }
    }

    let mut tx = pool.begin().await?;
    // mint voucher number (receipt_voucher, year 0 for simplicity? use 0)
    // reuse voucher_sequences
    sqlx::query(
        "INSERT INTO voucher_sequences (branch_id, kind, fiscal_year, last_no)
         VALUES (?, 'receipt_voucher', 0, 0) ON CONFLICT (branch_id, kind, fiscal_year) DO NOTHING",
    )
    .bind(branch_id)
    .execute(&mut *tx)
    .await?;
    let vno: i64 = sqlx::query_scalar(
        "UPDATE voucher_sequences SET last_no = last_no + 1
         WHERE branch_id = ? AND kind = 'receipt_voucher' AND fiscal_year = 0
         RETURNING last_no",
    )
    .bind(branch_id)
    .fetch_one(&mut *tx)
    .await?;

    // Link the receipt to the student / course behind the first allocation so
    // the voucher can be shown on the student's file and the course's file.
    // Only set the FK columns when the enrollment really exists — otherwise a
    // dangling id would violate the FOREIGN KEY (e.g. legacy/imported data).
    let (first_enr, first_student, first_course): (Option<i64>, Option<i64>, Option<i64>) =
        if let Some(e) = allocations.first().map(|a| a.enrollment_id) {
            match sqlx::query_as::<_, (i64, i64)>(
                "SELECT student_id, course_id FROM enrollments WHERE id = ?",
            )
            .bind(e)
            .fetch_optional(&mut *tx)
            .await?
            {
                Some((st, co)) => (Some(e), Some(st), Some(co)),
                None => (None, None, None),
            }
        } else {
            (None, None, None)
        };
    let res = sqlx::query(
        "INSERT INTO receipt_vouchers (branch_id, voucher_no, voucher_date, payer_name,
         student_id, enrollment_id, course_id, amount_minor, method, account_id, narrative, received_by,
         status, idempotency_key)
         VALUES (?, ?, date('now'), ?, ?, ?, ?, ?, 'cash', ?, ?, ?, 'posted', ?)",
    )
    .bind(branch_id)
    .bind(vno)
    .bind(memo)
    .bind(first_student)
    .bind(first_enr)
    .bind(first_course)
    .bind(total)
    .bind(cash_account_id)
    .bind(memo)
    .bind(actor.user_id)
    .bind(idempotency_key)
    .execute(&mut *tx)
    .await?;
    let voucher_id = res.last_insert_rowid();
    // persist per-enrollment applications (FR-7.5)
    for a in allocations {
        sqlx::query(
            "INSERT INTO payment_applications (receipt_voucher_id, enrollment_id, amount_minor)
             VALUES (?, ?, ?)",
        )
        .bind(voucher_id)
        .bind(a.enrollment_id)
        .bind(a.amount_minor)
        .execute(&mut *tx)
        .await?;
    }
    // cash register rule (FR-10.6)
    let require_open: bool = sqlx::query_scalar::<_, String>(
        "SELECT COALESCE(cash_rules_json, '{}') FROM center_settings WHERE id = 1",
    )
    .fetch_optional(&mut *tx)
    .await
    .ok()
    .flatten()
    .and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok())
    .and_then(|v| v.get("cash_receipts_require_open_register").and_then(|x| x.as_bool()))
    .unwrap_or(false);
    if require_open {
        let open: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM cash_registers WHERE branch_id = ? AND status = 'active'",
        )
        .bind(branch_id)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);
        if open == 0 {
            return Err(AppError::Conflict("يجب فتح صندوق لتسجيل قبض نقدي".into()));
        }
    }
    tx.commit().await?;

    // journal entry: Dr Cash, Cr Receivable
    let mut lines = vec![JournalLine {
        account_id: cash_account_id,
        debit_minor: total,
        credit_minor: 0,
        memo: memo.to_string(),
    }];
    // credit receivable per allocation (aggregate)
    lines.push(JournalLine {
        account_id: receivable_account_id,
        debit_minor: 0,
        credit_minor: total,
        memo: memo.to_string(),
    });
    let je = post_entry(pool, actor, &NewJournalEntry {
        entry_date: chrono::Utc::now().date_naive().to_string(),
        currency: "ILS".into(),
        source_type: "receipt_voucher".into(),
        source_id: voucher_id,
        memo: memo.to_string(),
        lines,
    })
    .await?;

    log_audit(pool, actor.user_id, "payment.record", "receipt_vouchers", Some(voucher_id),
        None, None, "").await;
    Ok(PaymentResult {
        voucher_id,
        journal_entry_id: je.id,
        total_minor: total,
    })
}

// ---------- enrollment vouchers (Phase 17) ----------

/// Create the two draft documents that accompany a new enrollment:
///   * a receipt voucher for the student's (net) fee — printable, not posted,
///   * a payment voucher for the teacher's entitlement from the active
///     financial agreement (skipped when the computed share is zero).
///
/// Both are inserted inside the caller's transaction so enrollment + vouchers
/// are atomic. Returns `(receipt_voucher_id, payment_voucher_id)`.
pub async fn create_enrollment_vouchers_in_tx(
    conn: &mut sqlx::SqliteConnection,
    actor: &crate::people::context::Actor,
    branch_id: i64,
    enrollment_id: i64,
    student_id: i64,
    course_id: i64,
    net_fee_minor: i64,
) -> AppResult<(i64, Option<i64>)> {
    let (student_name, teacher_id, course_name): (String, Option<i64>, String) =
        sqlx::query_as(
            "SELECT st.full_name, c.teacher_id, c.name
             FROM enrollments e JOIN students st ON st.id=e.student_id
             JOIN courses c ON c.id=e.course_id WHERE e.id=?",
        )
        .bind(enrollment_id)
        .fetch_one(&mut *conn)
        .await?;
    let _ = student_id;
    let cash_account: i64 =
        sqlx::query_scalar("SELECT id FROM accounts WHERE code='1000' LIMIT 1")
            .fetch_one(&mut *conn)
            .await?;
    let liability_account: i64 =
        sqlx::query_scalar("SELECT id FROM accounts WHERE code='1400' LIMIT 1")
            .fetch_one(&mut *conn)
            .await?;

    // mint receipt number
    sqlx::query(
        "INSERT INTO voucher_sequences (branch_id, kind, fiscal_year, last_no)
         VALUES (?, 'receipt_voucher', 0, 0) ON CONFLICT (branch_id, kind, fiscal_year) DO NOTHING",
    )
    .bind(branch_id)
    .execute(&mut *conn)
    .await?;
    let rvno: i64 = sqlx::query_scalar(
        "UPDATE voucher_sequences SET last_no=last_no+1
         WHERE branch_id=? AND kind='receipt_voucher' AND fiscal_year=0 RETURNING last_no",
    )
    .bind(branch_id)
    .fetch_one(&mut *conn)
    .await?;
    let rkey = format!("enr-{}-receipt", enrollment_id);
    let rv = sqlx::query(
        "INSERT INTO receipt_vouchers (branch_id, voucher_no, voucher_date, payer_name,
             student_id, enrollment_id, course_id, amount_minor, method, account_id,
             narrative, received_by, status, idempotency_key)
         VALUES (?, ?, date('now'), ?, ?, ?, ?, ?, 'cash', ?, ?, ?, 'draft', ?)",
    )
    .bind(branch_id)
    .bind(rvno)
    .bind(&student_name)
    .bind(student_id)
    .bind(enrollment_id)
    .bind(course_id)
    .bind(net_fee_minor)
    .bind(cash_account)
    .bind(format!("رسوم تسجيل: {}", course_name))
    .bind(actor.user_id)
    .bind(&rkey)
    .execute(&mut *conn)
    .await?;
    let receipt_id = rv.last_insert_rowid();

    // teacher entitlement from the active agreement
    let mut payment_id: Option<i64> = None;
    if let Some(tid) = teacher_id {
        let agr: Option<(String, Option<i64>, Option<i64>, Option<i64>, Option<i64>)> =
            sqlx::query_as(
                "SELECT method, rate_bp, center_rate_bp, flat_minor, per_session_minor
                 FROM financial_agreements WHERE course_id=? AND status='active' LIMIT 1",
            )
            .bind(course_id)
            .fetch_optional(&mut *conn)
            .await?;
        let share = if let Some((method, rate_bp, _center, flat, per_session)) = agr {
            let rate = rate_bp.unwrap_or(0);
            let m = crate::finance::entitlement::EntitlementMethod::parse(&method).unwrap_or(
                crate::finance::entitlement::EntitlementMethod::PercentOfAgreed,
            );
            let sessions: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sessions WHERE course_id=? AND status='executed'",
            )
            .bind(course_id)
            .fetch_one(&mut *conn)
            .await
            .unwrap_or(0);
            let session_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sessions WHERE course_id=?",
            )
            .bind(course_id)
            .fetch_one(&mut *conn)
            .await
            .unwrap_or(1);
            let session_value = if session_count > 0 { net_fee_minor / session_count } else { 0 };
            crate::finance::entitlement::compute_entitlement(
                &crate::finance::entitlement::EntitlementParams {
                    method: m,
                    rate_bp: rate,
                    agreed_basis_minor: net_fee_minor,
                    collected_minor: 0,
                    executed_sessions: sessions,
                    session_value_minor: session_value,
                    per_session_minor: per_session.unwrap_or(0),
                    flat_minor: flat.unwrap_or(0),
                },
            )
        } else {
            0
        };
        if share > 0 {
            let teacher_name: String =
                sqlx::query_scalar("SELECT full_name FROM teachers WHERE id=?")
                    .bind(tid)
                    .fetch_one(&mut *conn)
                    .await?;
            sqlx::query(
                "INSERT INTO voucher_sequences (branch_id, kind, fiscal_year, last_no)
                 VALUES (?, 'payment_voucher', 0, 0) ON CONFLICT (branch_id, kind, fiscal_year) DO NOTHING",
            )
            .bind(branch_id)
            .execute(&mut *conn)
            .await?;
            let pvno: i64 = sqlx::query_scalar(
                "UPDATE voucher_sequences SET last_no=last_no+1
                 WHERE branch_id=? AND kind='payment_voucher' AND fiscal_year=0 RETURNING last_no",
            )
            .bind(branch_id)
            .fetch_one(&mut *conn)
            .await?;
            let pkey = format!("enr-{}-payment", enrollment_id);
            let pv = sqlx::query(
                "INSERT INTO payment_vouchers (branch_id, voucher_no, voucher_date,
                     beneficiary_name, beneficiary_type, teacher_id, purpose, amount_minor,
                     method, account_id, narrative, created_by, status, source_type,
                     source_id, idempotency_key)
                 VALUES (?, ?, date('now'), ?, 'teacher', ?, ?, ?, 'cash', ?, ?, ?, 'draft',
                         'teacher_entitlement', ?, ?)",
            )
            .bind(branch_id)
            .bind(pvno)
            .bind(&teacher_name)
            .bind(tid)
            .bind(format!("حصة المدرس من تسجيل: {}", student_name))
            .bind(share)
            .bind(liability_account)
            .bind(format!("دورة: {}", course_name))
            .bind(actor.user_id)
            .bind(enrollment_id)
            .bind(&pkey)
            .execute(&mut *conn)
            .await?;
            payment_id = Some(pv.last_insert_rowid());
        }
    }
    Ok((receipt_id, payment_id))
}

// ---------- service sales (Phase 21) ----------

/// Record an "other service" receipt: e.g. a subject summary, printing a
/// specific sheet, or selling a book. Unlike `record_payment` this is NOT tied
/// to an enrollment — it posts Dr Cash / Cr Service Revenue directly.
pub async fn record_service_sale(
    pool: &SqlitePool,
    actor: &Actor,
    branch_id: i64,
    cash_account_id: i64,
    revenue_account_id: i64,
    payer_name: &str,
    description: &str,
    amount_minor: i64,
    method: &str,
    idempotency_key: &str,
) -> AppResult<i64> {
    actor.require("payments.record")?;
    if amount_minor <= 0 { return Err(AppError::validation("المبلغ يجب أن يكون أكبر من صفر")); }
    if payer_name.trim().is_empty() { return Err(AppError::validation("اسم الدافع مطلوب")); }
    if idempotency_key.trim().is_empty() { return Err(AppError::validation("مفتاح 멱등ية مطلوب")); }

    // idempotency
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM receipt_vouchers WHERE idempotency_key=?",
    ).bind(idempotency_key).fetch_optional(pool).await?;
    if let Some(id) = existing { return Ok(id); }

    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO voucher_sequences (branch_id,kind,fiscal_year,last_no)
         VALUES (?,'receipt_voucher',0,0) ON CONFLICT(branch_id,kind,fiscal_year) DO NOTHING",
    ).bind(branch_id).execute(&mut *tx).await?;
    let vno: i64 = sqlx::query_scalar(
        "UPDATE voucher_sequences SET last_no=last_no+1
         WHERE branch_id=? AND kind='receipt_voucher' AND fiscal_year=0 RETURNING last_no",
    ).bind(branch_id).fetch_one(&mut *tx).await?;
    let res = sqlx::query(
        "INSERT INTO receipt_vouchers (branch_id,voucher_no,voucher_date,payer_name,amount_minor,
             method,account_id,narrative,received_by,status,idempotency_key)
         VALUES (?,?,date('now'),?,?,?,?,?,?,'posted',?)
         ON CONFLICT(idempotency_key) DO NOTHING",
    )
    .bind(branch_id).bind(vno).bind(payer_name.trim()).bind(amount_minor)
    .bind(method).bind(cash_account_id).bind(&format!("[خدمة] {}", description)).bind(actor.user_id)
    .bind(idempotency_key)
    .execute(&mut *tx).await?;
    let vid = res.last_insert_rowid();
    if vid == 0 {
        // idempotent race: fetch existing
        let id: i64 = sqlx::query_scalar("SELECT id FROM receipt_vouchers WHERE idempotency_key=?")
            .bind(idempotency_key).fetch_one(&mut *tx).await?;
        return Ok(id);
    }
    tx.commit().await?;

    let posted = crate::finance::ledger::post_entry(pool, actor, &crate::finance::ledger::NewJournalEntry {
        entry_date: chrono::Utc::now().date_naive().to_string(),
        currency: "ILS".into(),
        source_type: "receipt_voucher".into(),
        source_id: vid,
        memo: description.to_string(),
        lines: vec![
            crate::finance::ledger::JournalLine { account_id: cash_account_id, debit_minor: amount_minor, credit_minor: 0, memo: description.to_string() },
            crate::finance::ledger::JournalLine { account_id: revenue_account_id, debit_minor: 0, credit_minor: amount_minor, memo: description.to_string() },
        ],
    }).await;
    if let Err(e) = posted {
        // never leave a posted voucher without its ledger entry behind
        sqlx::query("DELETE FROM receipt_vouchers WHERE id=? AND idempotency_key=?")
            .bind(vid)
            .bind(idempotency_key)
            .execute(pool)
            .await
            .ok();
        return Err(e);
    }
    crate::people::context::log_audit(pool, actor.user_id, "service_sale", "receipt_vouchers", Some(vid), None, None, description).await;
    Ok(vid)
}
