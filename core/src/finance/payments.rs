//! Payments / receipt collection (Phase 6).

use serde::Serialize;
use sqlx::{Row, SqlitePool};

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

    let first_enr: Option<i64> = None;
    let res = sqlx::query(
        "INSERT INTO receipt_vouchers (branch_id, voucher_no, voucher_date, payer_name,
         enrollment_id, amount_minor, method, account_id, narrative, received_by,
         status, idempotency_key)
         VALUES (?, ?, date('now'), ?, ?, ?, 'cash', ?, ?, ?, 'posted', ?)",
    )
    .bind(branch_id)
    .bind(vno)
    .bind(memo)
    .bind(first_enr)
    .bind(total)
    .bind(cash_account_id)
    .bind(memo)
    .bind(actor.user_id)
    .bind(idempotency_key)
    .execute(&mut *tx)
    .await?;
    let voucher_id = res.last_insert_rowid();
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
