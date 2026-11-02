//! Payment vouchers (Phase 11.3): payout to teachers/vendors/refunds.

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::AppResult;
use crate::finance::ledger::{post_entry, JournalLine, NewJournalEntry};
use crate::people::context::{log_audit, Actor};

#[derive(Debug, Serialize, Clone)]
pub struct PaymentVoucher {
    pub id: i64,
    pub voucher_no: i64,
    pub branch_id: i64,
    pub amount_minor: i64,
    pub beneficiary_type: String,
    pub teacher_id: Option<i64>,
    pub purpose: String,
    pub status: String,
    pub source_type: String,
    pub source_id: Option<i64>,
}

pub async fn create_payment_voucher(
    pool: &SqlitePool,
    actor: &Actor,
    branch_id: i64,
    beneficiary_type: &str,
    teacher_id: Option<i64>,
    purpose: &str,
    amount_minor: i64,
    method: &str,
    account_id: i64,
    source_type: &str,
    source_id: Option<i64>,
    idempotency_key: &str,
) -> AppResult<PaymentVoucher> {
    actor.require("vouchers.payment.create")?;
    if amount_minor <= 0 {
        return Err(AppError::validation("المبلغ يجب أن يكون أكبر من صفر"));
    }
    // idempotency
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM payment_vouchers WHERE idempotency_key = ?",
    )
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await?;
    if let Some(vid) = existing {
        return get_payment_voucher(pool, vid).await;
    }
    // mint number
    sqlx::query(
        "INSERT INTO voucher_sequences (branch_id, kind, fiscal_year, last_no) VALUES (?, 'payment_voucher', 0, 0) ON CONFLICT (branch_id, kind, fiscal_year) DO NOTHING",
    )
    .bind(branch_id)
    .execute(pool)
    .await?;
    let vno: i64 = sqlx::query_scalar(
        "UPDATE voucher_sequences SET last_no = last_no + 1 WHERE branch_id = ? AND kind = 'payment_voucher' AND fiscal_year = 0 RETURNING last_no",
    )
    .bind(branch_id)
    .fetch_one(pool)
    .await?;

    let res = sqlx::query(
        "INSERT INTO payment_vouchers (voucher_no, branch_id, voucher_date, beneficiary_name,
         beneficiary_type, teacher_id, purpose, amount_minor, method, account_id,
         created_by, status, source_type, source_id, idempotency_key)
         VALUES (?, ?, date('now'), ?, ?, ?, ?, ?, ?, ?, ?, 'draft', ?, ?, ?)",
    )
    .bind(vno)
    .bind(branch_id)
    .bind(purpose)
    .bind(beneficiary_type)
    .bind(teacher_id)
    .bind(purpose)
    .bind(amount_minor)
    .bind(method)
    .bind(account_id)
    .bind(actor.user_id)
    .bind(source_type)
    .bind(source_id)
    .bind(idempotency_key)
    .execute(pool)
    .await?;
    let pv = get_payment_voucher(pool, res.last_insert_rowid()).await?;
    log_audit(pool, actor.user_id, "vouchers.payment.create", "payment_vouchers", Some(pv.id), None, None, "").await;
    Ok(pv)
}

pub async fn approve_payment_voucher(
    pool: &SqlitePool,
    actor: &Actor,
    id: i64,
    maker_user_id: i64,
) -> AppResult<PaymentVoucher> {
    actor.require("vouchers.payment.approve")?;
    let pv = get_payment_voucher(pool, id).await?;
    if pv.status != "draft" && pv.status != "pending_approval" {
        return Err(AppError::Conflict("السند ليس بانتظار اعتماد".into()));
    }
    // maker-checker
    let creator: i64 = sqlx::query_scalar("SELECT created_by FROM payment_vouchers WHERE id = ?")
        .bind(id).fetch_one(pool).await?;
    if creator == actor.user_id && creator == maker_user_id {
        return Err(AppError::permission("الاعتماد الذاتي غير مسموح"));
    }
    sqlx::query("UPDATE payment_vouchers SET status='approved', approved_by=?, approved_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?")
        .bind(actor.user_id)
        .bind(id)
        .execute(pool)
        .await?;
    get_payment_voucher(pool, id).await
}

pub async fn pay_payment_voucher(
    pool: &SqlitePool,
    actor: &Actor,
    id: i64,
    payable_account: i64,
    cash_account: i64,
) -> AppResult<PaymentVoucher> {
    actor.require("vouchers.payment.approve")?;
    let pv = get_payment_voucher(pool, id).await?;
    if pv.status != "approved" {
        return Err(AppError::Conflict("السند غير معتمد".into()));
    }
    // bound by payable balance
    let payable_balance: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(debit_minor - credit_minor), 0) FROM journal_lines jl JOIN journal_entries je ON je.id = jl.entry_id WHERE jl.account_id = ? AND je.status = 'posted'",
    )
    .bind(payable_account)
    .fetch_one(pool)
    .await?;
    if pv.amount_minor > payable_balance {
        return Err(AppError::Conflict("voucher.overpaid_source: المبلغ يتجاوز الرصيد المستحق".into()));
    }
    let lines = vec![
        JournalLine { account_id: payable_account, debit_minor: pv.amount_minor, credit_minor: 0, memo: "payout".into() },
        JournalLine { account_id: cash_account, debit_minor: 0, credit_minor: pv.amount_minor, memo: "payout".into() },
    ];
    let je = post_entry(pool, actor, &NewJournalEntry {
        entry_date: chrono::Utc::now().date_naive().to_string(),
        currency: "ILS".into(),
        source_type: "payment_voucher".into(),
        source_id: pv.id,
        memo: pv.purpose.clone(),
        lines,
    }).await?;
    sqlx::query("UPDATE payment_vouchers SET status='paid', paid_by=?, paid_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'), journal_entry_id=? WHERE id=?")
        .bind(actor.user_id)
        .bind(je.id)
        .bind(id)
        .execute(pool)
        .await?;
    get_payment_voucher(pool, id).await
}

pub async fn cancel_payment_voucher(
    pool: &SqlitePool,
    actor: &Actor,
    id: i64,
    reason: &str,
) -> AppResult<PaymentVoucher> {
    actor.require("vouchers.payment.approve")?;
    if reason.trim().is_empty() {
        return Err(AppError::validation("السبب مطلوب"));
    }
    let pv = get_payment_voucher(pool, id).await?;
    if pv.status == "paid" {
        return Err(AppError::Conflict("لا يمكن إلغاء سند مدفوع".into()));
    }
    sqlx::query("UPDATE payment_vouchers SET status='cancelled' WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?;
    get_payment_voucher(pool, id).await
}

async fn get_payment_voucher(pool: &SqlitePool, id: i64) -> AppResult<PaymentVoucher> {
    let r = sqlx::query(
        "SELECT id, voucher_no, branch_id, amount_minor, beneficiary_type, teacher_id, purpose, status, source_type, source_id FROM payment_vouchers WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(PaymentVoucher {
        id: r.get("id"),
        voucher_no: r.get("voucher_no"),
        branch_id: r.get("branch_id"),
        amount_minor: r.get("amount_minor"),
        beneficiary_type: r.get("beneficiary_type"),
        teacher_id: r.get("teacher_id"),
        purpose: r.get("purpose"),
        status: r.get("status"),
        source_type: r.get("source_type"),
        source_id: r.get("source_id"),
    })
}
