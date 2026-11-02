//! Cash register operations (Phase 7).

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::AppResult;
use crate::finance::ledger::{post_entry, JournalLine, NewJournalEntry};
use crate::people::context::{log_audit, Actor};

#[derive(Debug, Serialize, Clone)]
pub struct CashMovement {
    pub id: i64,
    pub register_id: i64,
    pub kind: String,
    pub direction: String,
    pub amount_minor: i64,
    pub balance_after_minor: i64,
}

pub async fn open_register(
    pool: &SqlitePool,
    actor: &Actor,
    branch_id: i64,
    code: &str,
) -> AppResult<i64> {
    actor.require("cash.manage")?;
    let id: i64 = sqlx::query_scalar(
        "SELECT id FROM cash_registers WHERE branch_id = ? AND code = ?",
    )
    .bind(branch_id)
    .bind(code)
    .fetch_one(pool)
    .await?;
    // mark current user
    sqlx::query("UPDATE cash_registers SET current_user_id = ?, status='active', updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?")
        .bind(actor.user_id)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(id)
}

pub async fn record_cash_movement(
    pool: &SqlitePool,
    actor: &Actor,
    register_id: i64,
    kind: &str,
    direction: &str,
    amount_minor: i64,
    ref_type: &str,
    ref_id: Option<i64>,
) -> AppResult<CashMovement> {
    actor.require("cash.manage")?;
    if amount_minor < 0 {
        return Err(AppError::validation("مبلغ سالب"));
    }
    if !matches!(direction, "in" | "out") {
        return Err(AppError::validation("اتجاه غير معروف"));
    }
    // current balance
    let last: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(balance_after_minor), 0) FROM cash_movements WHERE register_id = ?",
    )
    .bind(register_id)
    .fetch_one(pool)
    .await?;
    let new_balance = if direction == "in" {
        last + amount_minor
    } else {
        if amount_minor > last {
            return Err(AppError::Conflict("رصيد غير كافٍ".into()));
        }
        last - amount_minor
    };
    let res = sqlx::query(
        "INSERT INTO cash_movements (register_id, kind, direction, amount_minor, ref_type, ref_id, balance_after_minor, created_by)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(register_id)
    .bind(kind)
    .bind(direction)
    .bind(amount_minor)
    .bind(ref_type)
    .bind(ref_id)
    .bind(new_balance)
    .bind(actor.user_id)
    .execute(pool)
    .await?;
    let cm = CashMovement {
        id: res.last_insert_rowid(),
        register_id,
        kind: kind.to_string(),
        direction: direction.to_string(),
        amount_minor,
        balance_after_minor: new_balance,
    };
    log_audit(pool, actor.user_id, "cash.manage", "cash_movements", Some(cm.id), None, None, "").await;
    Ok(cm)
}

pub async fn close_register(
    pool: &SqlitePool,
    actor: &Actor,
    register_id: i64,
    opening_balance_minor: i64,
    counted_balance_minor: i64,
    cash_variance_account: i64,
    cash_account: i64,
) -> AppResult<i64> {
    actor.require("cash.close")?;
    let booked: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(balance_after_minor), 0) FROM cash_movements WHERE register_id = ?",
    )
    .bind(register_id)
    .fetch_one(pool)
    .await?;
    let variance = counted_balance_minor - booked;
    let res = sqlx::query(
        "INSERT INTO register_closings (register_id, opened_at, opening_balance_minor,
         booked_balance_minor, counted_balance_minor, variance_minor, prepared_by, status)
         VALUES (?, strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?, ?, ?, ?, ?, 'pending')",
    )
    .bind(register_id)
    .bind(opening_balance_minor)
    .bind(booked)
    .bind(counted_balance_minor)
    .bind(variance)
    .bind(actor.user_id)
    .execute(pool)
    .await?;
    let closing_id = res.last_insert_rowid();
    // post variance if nonzero
    if variance != 0 {
        let abs = variance.unsigned_abs() as i64;
        let (d, c) = if variance > 0 {
            (cash_account, cash_variance_account)
        } else {
            (cash_variance_account, cash_account)
        };
        let lines = vec![
            JournalLine { account_id: d, debit_minor: abs, credit_minor: 0, memo: "variance".into() },
            JournalLine { account_id: c, debit_minor: 0, credit_minor: abs, memo: "variance".into() },
        ];
        let je = post_entry(pool, actor, &NewJournalEntry {
            entry_date: chrono::Utc::now().date_naive().to_string(),
            currency: "ILS".into(),
            source_type: "cash_closing".into(),
            source_id: closing_id,
            memo: "closing variance".into(),
            lines,
        }).await?;
        sqlx::query("UPDATE register_closings SET journal_entry_id = ?, status='approved', approved_by=?, approved_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?")
            .bind(je.id)
            .bind(actor.user_id)
            .bind(closing_id)
            .execute(pool)
            .await?;
    } else {
        sqlx::query("UPDATE register_closings SET status='approved', approved_by=?, approved_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?")
            .bind(actor.user_id)
            .bind(closing_id)
            .execute(pool)
            .await?;
    }
    log_audit(pool, actor.user_id, "cash.close", "register_closings", Some(closing_id), None, None, "").await;
    Ok(closing_id)
}
