//! Expenses (Phase 7).

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::AppResult;
use crate::people::context::{log_audit, Actor};
use crate::vouchers::payment_vouchers::create_payment_voucher;

#[derive(Debug, Serialize, Clone)]
pub struct Expense {
    pub id: i64,
    pub branch_id: i64,
    pub category_id: i64,
    pub expense_date: String,
    pub amount_minor: i64,
    pub beneficiary: String,
    pub description: String,
    pub method: String,
    pub account_id: i64,
    pub status: String,
}

pub async fn create_expense(
    pool: &SqlitePool,
    actor: &Actor,
    branch_id: i64,
    category_id: i64,
    expense_date: &str,
    amount_minor: i64,
    beneficiary: &str,
    description: &str,
    method: &str,
    account_id: i64,
) -> AppResult<Expense> {
    actor.require("expenses.create")?;
    if amount_minor <= 0 {
        return Err(AppError::validation("المبلغ يجب أن يكون أكبر من صفر"));
    }
    let res = sqlx::query(
        "INSERT INTO expenses (branch_id, category_id, expense_date, amount_minor,
         beneficiary, description, method, account_id, created_by, status)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'draft')",
    )
    .bind(branch_id)
    .bind(category_id)
    .bind(expense_date)
    .bind(amount_minor)
    .bind(beneficiary)
    .bind(description)
    .bind(method)
    .bind(account_id)
    .bind(actor.user_id)
    .execute(pool)
    .await?;
    let e = expense_by_id(pool, res.last_insert_rowid()).await?;
    log_audit(pool, actor.user_id, "expenses.create", "expenses", Some(e.id), None, None, "").await;
    Ok(e)
}

pub async fn approve_expense(
    pool: &SqlitePool,
    actor: &Actor,
    id: i64,
) -> AppResult<Expense> {
    actor.require("expenses.approve")?;
    sqlx::query("UPDATE expenses SET status='approved', approved_by=?, approved_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=? AND status IN ('draft','pending_approval')")
        .bind(actor.user_id)
        .bind(id)
        .execute(pool)
        .await?;
    let e = expense_by_id(pool, id).await?;
    log_audit(pool, actor.user_id, "expenses.approve", "expenses", Some(id), None, None, "").await;
    Ok(e)
}

pub async fn mark_expense_paid(
    pool: &SqlitePool,
    actor: &Actor,
    id: i64,
    cash_account: i64,
) -> AppResult<Expense> {
    actor.require("expenses.approve")?;
    let e = expense_by_id(pool, id).await?;
    if e.status != "approved" {
        return Err(AppError::Conflict("المصروف غير معتمد".into()));
    }
    // create payment voucher for the expense
    let pv = create_payment_voucher(
        pool, actor, e.branch_id, "vendor", None, &e.description, e.amount_minor,
        "cash", cash_account, "expense", Some(id), &format!("exp-{}", id),
    ).await?;
    // approve + pay
    let _ = crate::vouchers::payment_vouchers::approve_payment_voucher(pool, actor, pv.id, 0).await;
    sqlx::query("UPDATE expenses SET status='paid', voucher_id=? WHERE id=?")
        .bind(pv.id)
        .bind(id)
        .execute(pool)
        .await?;
    expense_by_id(pool, id).await
}

async fn expense_by_id(pool: &SqlitePool, id: i64) -> AppResult<Expense> {
    let r = sqlx::query(
        "SELECT id, branch_id, category_id, expense_date, amount_minor, beneficiary,
                description, method, account_id, status FROM expenses WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(Expense {
        id: r.get("id"),
        branch_id: r.get("branch_id"),
        category_id: r.get("category_id"),
        expense_date: r.get("expense_date"),
        amount_minor: r.get("amount_minor"),
        beneficiary: r.get("beneficiary"),
        description: r.get("description"),
        method: r.get("method"),
        account_id: r.get("account_id"),
        status: r.get("status"),
    })
}
