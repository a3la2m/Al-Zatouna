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
    let _ = crate::vouchers::payment_vouchers::approve_payment_voucher(pool, actor, pv.id, 0).await;
    // post Dr expense / Cr cash
    let je = crate::finance::ledger::post_entry(pool, actor, &crate::finance::ledger::NewJournalEntry {
        entry_date: chrono::Utc::now().date_naive().to_string(),
        currency: "ILS".into(),
        source_type: "expense".into(),
        source_id: id,
        memo: e.description.clone(),
        lines: vec![
            crate::finance::ledger::JournalLine { account_id: e.account_id, debit_minor: e.amount_minor, credit_minor: 0, memo: e.description.clone() },
            crate::finance::ledger::JournalLine { account_id: cash_account, debit_minor: 0, credit_minor: e.amount_minor, memo: e.description.clone() },
        ],
    }).await?;
    sqlx::query("UPDATE payment_vouchers SET status='paid', paid_by=?, paid_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'), journal_entry_id=? WHERE id=?")
        .bind(actor.user_id)
        .bind(je.id)
        .bind(pv.id)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE expenses SET status='paid', voucher_id=? WHERE id=?")
        .bind(pv.id)
        .bind(id)
        .execute(pool)
        .await?;
    expense_by_id(pool, id).await
}

const EXPENSE_COLS: &str = "id, branch_id, category_id, expense_date, amount_minor,
     beneficiary, description, method, account_id, status";

fn row_expense(r: &sqlx::sqlite::SqliteRow) -> Expense {
    Expense {
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
    }
}

async fn expense_by_id(pool: &SqlitePool, id: i64) -> AppResult<Expense> {
    let r = sqlx::query(&format!("SELECT {EXPENSE_COLS} FROM expenses WHERE id = ?"))
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(row_expense(&r))
}

/// List expenses (newest first) — used by the expenses screen and reports.
pub async fn list_expenses(
    pool: &SqlitePool,
    actor: &Actor,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<Expense>> {
    actor.require("expenses.view")?;
    let limit = limit.clamp(1, 500);
    let rows = if let Some(st) = status {
        sqlx::query(
            &format!("SELECT {EXPENSE_COLS} FROM expenses WHERE status = ? ORDER BY id DESC LIMIT ?"),
        )
        .bind(st)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            &format!("SELECT {EXPENSE_COLS} FROM expenses ORDER BY id DESC LIMIT ?"),
        )
        .bind(limit)
        .fetch_all(pool)
        .await?
    };
    Ok(rows.iter().map(row_expense).collect())
}

/// Expense categories (for the expense form dropdown + the by-category report).
pub async fn list_expense_categories(pool: &SqlitePool) -> AppResult<Vec<ExpenseCategory>> {
    let rows = sqlx::query(
        "SELECT id, code, name, is_active FROM expense_categories ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| ExpenseCategory {
            id: r.get("id"),
            code: r.get("code"),
            name: r.get("name"),
            is_active: r.get::<i64, _>("is_active") != 0,
        })
        .collect())
}

#[derive(Debug, Serialize, Clone)]
pub struct ExpenseCategory {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub is_active: bool,
}


// ---------- teacher-linked expense (Phase 17) ----------

/// Create an expense and immediately link its payment voucher to a teacher,
/// so the amount shows up on the teacher's file as a payable.
pub async fn create_teacher_expense(
    pool: &SqlitePool,
    actor: &Actor,
    branch_id: i64,
    category_id: i64,
    teacher_id: i64,
    expense_date: &str,
    amount_minor: i64,
    description: &str,
    method: &str,
    account_id: i64,
) -> AppResult<Expense> {
    actor.require("expenses.create")?;
    let teacher_name: String = sqlx::query_scalar(
        "SELECT full_name FROM teachers WHERE id = ?",
    )
    .bind(teacher_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("المدرس غير موجود"))?;
    let e = create_expense(pool, actor, branch_id, category_id, expense_date, amount_minor,
        &teacher_name, description, method, account_id).await?;
    // stamp the freshly created payment voucher with the teacher
    sqlx::query(
        "UPDATE payment_vouchers SET teacher_id=?, beneficiary_type='teacher', beneficiary_name=?
         WHERE source_type='expense' AND source_id=?",
    )
    .bind(teacher_id)
    .bind(&teacher_name)
    .bind(e.id)
    .execute(pool)
    .await?;
    expense_by_id(pool, e.id).await
}
