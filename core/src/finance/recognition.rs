//! Revenue recognition (Phase 11.1): accrual on enrollment, recognition on
//! course start / session execution. Separated from cash collection (FR-9.4).

use sqlx::SqlitePool;

use crate::error::AppError;
use crate::AppResult;
use crate::finance::ledger::{post_entry, JournalLine, NewJournalEntry};
use crate::people::context::Actor;

/// Accrual at enrollment approval: Dr Students Receivable / Cr Deferred Revenue.
pub async fn accrue_enrollment(
    pool: &SqlitePool,
    actor: &Actor,
    enrollment_id: i64,
    amount_minor: i64,
    receivable_account: i64,
    deferred_revenue_account: i64,
) -> AppResult<i64> {
    if amount_minor <= 0 {
        return Err(AppError::validation("المبلغ يجب أن يكون أكبر من صفر"));
    }
    let lines = vec![
        JournalLine { account_id: receivable_account, debit_minor: amount_minor, credit_minor: 0, memo: "accrual".into() },
        JournalLine { account_id: deferred_revenue_account, debit_minor: 0, credit_minor: amount_minor, memo: "accrual".into() },
    ];
    let je = post_entry(pool, actor, &NewJournalEntry {
        entry_date: chrono::Utc::now().date_naive().to_string(),
        currency: "ILS".into(),
        source_type: "enroll_recognition".into(),
        source_id: enrollment_id,
        memo: "enrollment accrual".into(),
        lines,
    }).await?;
    Ok(je.id)
}

/// Recognition: Dr Deferred Revenue / Cr Course Revenue.
pub async fn recognize_revenue(
    pool: &SqlitePool,
    actor: &Actor,
    enrollment_id: i64,
    amount_minor: i64,
    deferred_revenue_account: i64,
    revenue_account: i64,
) -> AppResult<i64> {
    if amount_minor <= 0 {
        return Err(AppError::validation("المبلغ يجب أن يكون أكبر من صفر"));
    }
    let lines = vec![
        JournalLine { account_id: deferred_revenue_account, debit_minor: amount_minor, credit_minor: 0, memo: "recognition".into() },
        JournalLine { account_id: revenue_account, debit_minor: 0, credit_minor: amount_minor, memo: "recognition".into() },
    ];
    let je = post_entry(pool, actor, &NewJournalEntry {
        entry_date: chrono::Utc::now().date_naive().to_string(),
        currency: "ILS".into(),
        source_type: "enroll_recognition".into(),
        source_id: enrollment_id,
        memo: "revenue recognition".into(),
        lines,
    }).await?;
    Ok(je.id)
}
