//! Refunds (Phase 6).

use serde::Serialize;


#[derive(Debug, Clone, Copy)]
pub enum RefundPolicy {
    BeforeStart { admin_fee_minor: i64 },
    AfterStart { admin_fee_minor: i64, remaining_weight: i64, total_weight: i64 },
}

#[derive(Debug, Serialize, Clone)]
pub struct RefundResult {
    pub amount_minor: i64,
    pub admin_fee_minor: i64,
    pub gross_minor: i64,
}

/// Compute refund amount per policy. Capped by paid_minor (caller enforces).
pub fn compute_refund(policy: RefundPolicy, base_minor: i64, paid_minor: i64, prior_refunds_minor: i64) -> RefundResult {
    let (admin_fee, gross) = match policy {
        RefundPolicy::BeforeStart { admin_fee_minor } => (admin_fee_minor, base_minor),
        RefundPolicy::AfterStart { admin_fee_minor, remaining_weight, total_weight } => {
            let w = if total_weight > 0 { remaining_weight } else { 0 };
            let t = if total_weight > 0 { total_weight } else { 1 };
            let proportional = base_minor * w / t;
            (admin_fee_minor, proportional)
        }
    };
    let mut amount = gross - admin_fee - prior_refunds_minor;
    if amount < 0 {
        amount = 0;
    }
    // cap by paid
    let cap = paid_minor.saturating_sub(prior_refunds_minor);
    if amount > cap {
        amount = cap;
    }
    if amount < 0 { amount = 0; }
    RefundResult {
        amount_minor: amount,
        admin_fee_minor: admin_fee,
        gross_minor: gross,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn before_start_full_minus_fee() {
        let r = compute_refund(RefundPolicy::BeforeStart { admin_fee_minor: 500 }, 10000, 10000, 0);
        assert_eq!(r.amount_minor, 9500);
    }

    #[test]
    fn after_start_proportional() {
        // 50% remaining
        let r = compute_refund(
            RefundPolicy::AfterStart { admin_fee_minor: 0, remaining_weight: 5, total_weight: 10 },
            10000, 10000, 0,
        );
        assert_eq!(r.amount_minor, 5000);
    }

    #[test]
    fn clamped_to_paid_and_nonnegative() {
        let r = compute_refund(RefundPolicy::BeforeStart { admin_fee_minor: 0 }, 10000, 3000, 0);
        assert_eq!(r.amount_minor, 3000);
        let r2 = compute_refund(RefundPolicy::BeforeStart { admin_fee_minor: 20000 }, 10000, 10000, 0);
        assert_eq!(r2.amount_minor, 0);
    }

    #[test]
    fn prior_refunds_deducted() {
        let r = compute_refund(RefundPolicy::BeforeStart { admin_fee_minor: 0 }, 10000, 10000, 4000);
        assert_eq!(r.amount_minor, 6000);
    }
}

use sqlx::SqlitePool;
use crate::people::context::{log_audit, Actor};

/// Request a refund (engine computes proposed amount + snapshot).
pub async fn request_refund(
    pool: &SqlitePool,
    actor: &Actor,
    enrollment_id: i64,
    base_minor: i64,
    paid_minor: i64,
    prior_refunds_minor: i64,
    policy: RefundPolicy,
    cutoff_ok: bool,
    manager_exception: bool,
    exception_reason: &str,
) -> Result<i64, crate::AppError> {
    actor.require("refunds.request")?;
    if !cutoff_ok && !manager_exception {
        return Err(crate::AppError::validation("انتهى موعد الاسترداد"));
    }
    if manager_exception && exception_reason.trim().is_empty() {
        return Err(crate::AppError::validation("سبب الاستثناء مطلوب"));
    }
    let (student_id, course_id): (i64, i64) = sqlx::query_as(
        "SELECT student_id, course_id FROM enrollments WHERE id = ?",
    )
    .bind(enrollment_id)
    .fetch_one(pool)
    .await?;
    let res = compute_refund(policy, base_minor, paid_minor, prior_refunds_minor);
    let _amount = res.amount_minor;
    if manager_exception {
        // exception allows full proposed amount (already capped by paid)
    }
    let _ = _amount;
    let breakdown = serde_json::json!({
        "base_minor": base_minor,
        "paid_minor": paid_minor,
        "prior_refunds_minor": prior_refunds_minor,
        "admin_fee_minor": res.admin_fee_minor,
        "gross_minor": res.gross_minor,
        "amount_minor": res.amount_minor,
        "manager_exception": manager_exception,
    });
    let res2 = sqlx::query(
        "INSERT INTO refund_requests (enrollment_id, student_id, course_id, requested_by,
         status, policy_snapshot_json, refundable_minor, breakdown_json)
         VALUES (?, ?, ?, ?, 'submitted', '{}', ?, ?)",
    )
    .bind(enrollment_id)
    .bind(student_id)
    .bind(course_id)
    .bind(actor.user_id)
    .bind(res.amount_minor)
    .bind(breakdown.to_string())
    .execute(pool)
    .await?;
    let id = res2.last_insert_rowid();
    log_audit(pool, actor.user_id, "refunds.request", "refund_requests", Some(id), None, None, "").await;
    Ok(id)
}

pub async fn approve_refund(
    pool: &SqlitePool,
    actor: &Actor,
    refund_id: i64,
    approve: bool,
    reason: &str,
) -> Result<(), crate::AppError> {
    if approve {
        actor.require("refunds.approve")?;
    } else {
        actor.require("refunds.approve")?;
    }
    if !approve && reason.trim().is_empty() {
        return Err(crate::AppError::validation("سبب الرفض مطلوب"));
    }
    if approve {
        sqlx::query("UPDATE refund_requests SET status='approved', approver=?, approved_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?")
            .bind(actor.user_id)
            .bind(refund_id)
            .execute(pool)
            .await?;
        log_audit(pool, actor.user_id, "refunds.approve", "refund_requests", Some(refund_id), None, None, reason).await;
    } else {
        sqlx::query("UPDATE refund_requests SET status='rejected', rejected_reason=? WHERE id=?")
            .bind(reason)
            .bind(refund_id)
            .execute(pool)
            .await?;
        log_audit(pool, actor.user_id, "refunds.approve", "refund_requests", Some(refund_id), None, None, reason).await;
    }
    Ok(())
}

use crate::vouchers::payment_vouchers::{approve_payment_voucher, create_payment_voucher};

/// Payout an approved refund: creates+approves+pays a payment voucher.
pub async fn payout_refund(
    pool: &SqlitePool,
    actor: &Actor,
    refund_id: i64,
    branch_id: i64,
    cash_account: i64,
    payable_account: i64,
    idempotency_key: &str,
) -> Result<i64, crate::AppError> {
    actor.require("refunds.approve")?;
    // load refundable amount
    let refundable: i64 = sqlx::query_scalar(
        "SELECT refundable_minor FROM refund_requests WHERE id = ? AND status = 'approved'",
    )
    .bind(refund_id)
    .fetch_one(pool)
    .await?;
    let pv = create_payment_voucher(
        pool, actor, branch_id, "student", None, "استرداد", refundable,
        "cash", cash_account, "refund", Some(refund_id), idempotency_key,
    ).await?;
    let _ = approve_payment_voucher(pool, actor, pv.id, 0).await?;
    // post Dr payable/refund account / Cr cash (no balance bound for refunds)
    let je = crate::finance::ledger::post_entry(pool, actor, &crate::finance::ledger::NewJournalEntry {
        entry_date: chrono::Utc::now().date_naive().to_string(),
        currency: "ILS".into(),
        source_type: "payment_voucher".into(),
        source_id: pv.id,
        memo: "استرداد".into(),
        lines: vec![
            crate::finance::ledger::JournalLine { account_id: payable_account, debit_minor: refundable, credit_minor: 0, memo: "استرداد".into() },
            crate::finance::ledger::JournalLine { account_id: cash_account, debit_minor: 0, credit_minor: refundable, memo: "استرداد".into() },
        ],
    }).await?;
    sqlx::query("UPDATE payment_vouchers SET status='paid', paid_by=?, paid_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'), journal_entry_id=? WHERE id=?")
        .bind(actor.user_id).bind(je.id).bind(pv.id).execute(pool).await?;
    // mark refund request paid
    sqlx::query("UPDATE refund_requests SET status='paid', voucher_id=? WHERE id=?")
        .bind(0i64).bind(refund_id).execute(pool).await?;
    log_audit(pool, actor.user_id, "refunds.payout", "refund_requests", Some(refund_id), None, None, "").await;
    Ok(pv.id)
}

/// Alias for payout_refund (compatibility).
pub async fn refund_payout(
    pool: &SqlitePool,
    actor: &Actor,
    refund_id: i64,
    branch_id: i64,
    cash_account: i64,
    payable_account: i64,
    idempotency_key: &str,
) -> Result<i64, crate::AppError> {
    payout_refund(pool, actor, refund_id, branch_id, cash_account, payable_account, idempotency_key).await
}

/// Alias.
pub async fn execute_refund_payout(
    pool: &SqlitePool,
    actor: &Actor,
    refund_id: i64,
    branch_id: i64,
    cash_account: i64,
    payable_account: i64,
    idempotency_key: &str,
) -> Result<i64, crate::AppError> {
    payout_refund(pool, actor, refund_id, branch_id, cash_account, payable_account, idempotency_key).await
}
