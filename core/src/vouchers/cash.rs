//! Cash register operations (Phase 7).

use serde::Serialize;
use sqlx::SqlitePool;

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
    // current balance — the LATEST movement, not the peak (MAX would be wrong
    // after any withdrawal because a later `in` would start from the peak).
    let last = current_balance(pool, register_id).await?;
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

pub async fn transfer_between_registers(
    pool: &SqlitePool,
    actor: &Actor,
    from_register: i64,
    to_register: i64,
    amount_minor: i64,
) -> AppResult<(i64, i64)> {
    actor.require("cash.manage")?;
    if amount_minor <= 0 {
        return Err(AppError::validation("المبلغ يجب أن يكون أكبر من صفر"));
    }
    if from_register == to_register {
        return Err(AppError::validation("لا يمكن التحويل إلى نفس الصندوق"));
    }
    // out from source
    let out = record_cash_movement(pool, actor, from_register, "transfer_out", "out", amount_minor, "transfer", Some(to_register)).await?;
    // in to destination
    let inn = record_cash_movement(pool, actor, to_register, "transfer_in", "in", amount_minor, "transfer", Some(from_register)).await?;
    Ok((out.id, inn.id))
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
    let booked = current_balance(pool, register_id).await?;
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

// ---------- balance helpers, summaries & listings (auto-calculation) -------

/// Latest booked balance of a register (0 when it has no movement yet).
///
/// Deliberately reads the LAST movement by id instead of `MAX(balance_after)`:
/// a withdrawal lowers the balance, so the peak would silently over-state the
/// cash on hand and every later calculation would start from the wrong base.
pub async fn current_balance(pool: &SqlitePool, register_id: i64) -> Result<i64, sqlx::Error> {
    let last: Option<i64> = sqlx::query_scalar(
        "SELECT balance_after_minor FROM cash_movements
         WHERE register_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(register_id)
    .fetch_optional(pool)
    .await?;
    Ok(last.unwrap_or(0))
}

#[derive(Debug, Serialize, Clone)]
pub struct CashRegisterRow {
    pub id: i64,
    pub branch_id: i64,
    pub code: String,
    pub name: String,
    pub status: String,
    pub current_user_id: Option<i64>,
    pub balance_minor: i64,
    pub movement_count: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct CashMovementRow {
    pub id: i64,
    pub register_id: i64,
    pub kind: String,
    pub direction: String,
    pub amount_minor: i64,
    pub ref_type: String,
    pub ref_id: Option<i64>,
    pub occurred_at: String,
    pub balance_after_minor: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct CashKindTotal {
    pub kind: String,
    pub direction: String,
    pub total_minor: i64,
    pub count: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct CashClosingRow {
    pub id: i64,
    pub register_id: i64,
    pub closed_at: String,
    pub opening_balance_minor: i64,
    pub booked_balance_minor: i64,
    pub counted_balance_minor: i64,
    pub variance_minor: i64,
    pub status: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct CashRegisterSummary {
    pub register: CashRegisterRow,
    pub opening_balance_minor: i64,
    pub total_in_minor: i64,
    pub total_out_minor: i64,
    pub balance_minor: i64,
    pub movement_count: i64,
    pub today_in_minor: i64,
    pub today_out_minor: i64,
    pub kind_totals: Vec<CashKindTotal>,
    pub last_closing: Option<CashClosingRow>,
}

/// All registers of the system with their live (auto-calculated) balances.
pub async fn list_registers(pool: &SqlitePool) -> AppResult<Vec<CashRegisterRow>> {
    let rows: Vec<(i64, i64, String, String, String, Option<i64>)> = sqlx::query_as(
        "SELECT id, branch_id, code, name, status, current_user_id
         FROM cash_registers ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for (id, branch_id, code, name, status, current_user_id) in rows {
        let balance_minor = current_balance(pool, id).await?;
        let movement_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM cash_movements WHERE register_id = ?")
                .bind(id)
                .fetch_one(pool)
                .await?;
        out.push(CashRegisterRow {
            id,
            branch_id,
            code,
            name,
            status,
            current_user_id,
            balance_minor,
            movement_count,
        });
    }
    Ok(out)
}

/// Movement ledger of one register, newest first.
pub async fn list_movements(
    pool: &SqlitePool,
    register_id: i64,
    limit: i64,
) -> AppResult<Vec<CashMovementRow>> {
    let limit = limit.clamp(1, 1000);
    let rows: Vec<(
        i64,
        i64,
        String,
        String,
        i64,
        String,
        Option<i64>,
        String,
        i64,
    )> = sqlx::query_as(
        "SELECT id, register_id, kind, direction, amount_minor, ref_type, ref_id,
                occurred_at, balance_after_minor
         FROM cash_movements WHERE register_id = ?
         ORDER BY id DESC LIMIT ?",
    )
    .bind(register_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                register_id,
                kind,
                direction,
                amount_minor,
                ref_type,
                ref_id,
                occurred_at,
                balance_after_minor,
            )| CashMovementRow {
                id,
                register_id,
                kind,
                direction,
                amount_minor,
                ref_type,
                ref_id,
                occurred_at,
                balance_after_minor,
            },
        )
        .collect())
}

/// Full auto-calculated picture of one register: totals, live balance,
/// today's movement and the most recent closing (variance included).
pub async fn register_summary(
    pool: &SqlitePool,
    register_id: i64,
) -> AppResult<CashRegisterSummary> {
    let reg: Option<(i64, i64, String, String, String, Option<i64>)> = sqlx::query_as(
        "SELECT id, branch_id, code, name, status, current_user_id
         FROM cash_registers WHERE id = ?",
    )
    .bind(register_id)
    .fetch_optional(pool)
    .await?;
    let Some((id, branch_id, code, name, status, current_user_id)) = reg else {
        return Err(AppError::NotFound("الصندوق غير موجود".into()));
    };

    let totals: (Option<i64>, Option<i64>, i64) = sqlx::query_as(
        "SELECT SUM(CASE WHEN direction='in'  THEN amount_minor ELSE 0 END),
                SUM(CASE WHEN direction='out' THEN amount_minor ELSE 0 END),
                COUNT(*)
         FROM cash_movements WHERE register_id = ?",
    )
    .bind(register_id)
    .fetch_one(pool)
    .await?;
    let total_in_minor = totals.0.unwrap_or(0);
    let total_out_minor = totals.1.unwrap_or(0);
    let movement_count = totals.2;

    let today: (Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT SUM(CASE WHEN direction='in'  THEN amount_minor ELSE 0 END),
                SUM(CASE WHEN direction='out' THEN amount_minor ELSE 0 END)
         FROM cash_movements
         WHERE register_id = ? AND date(occurred_at) = date('now')",
    )
    .bind(register_id)
    .fetch_one(pool)
    .await?;

    let kind_rows: Vec<(String, String, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT kind, direction, SUM(amount_minor), COUNT(*)
         FROM cash_movements WHERE register_id = ?
         GROUP BY kind, direction ORDER BY kind, direction",
    )
    .bind(register_id)
    .fetch_all(pool)
    .await?;
    let kind_totals = kind_rows
        .into_iter()
        .map(|(kind, direction, total, count)| CashKindTotal {
            kind,
            direction,
            total_minor: total.unwrap_or(0),
            count: count.unwrap_or(0),
        })
        .collect();

    let closing: Option<(i64, String, i64, i64, i64, i64, String)> = sqlx::query_as(
        "SELECT id, closed_at, opening_balance_minor, booked_balance_minor,
                counted_balance_minor, variance_minor, status
         FROM register_closings WHERE register_id = ?
         ORDER BY id DESC LIMIT 1",
    )
    .bind(register_id)
    .fetch_optional(pool)
    .await?;

    let opening_balance_minor: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor),0) FROM cash_movements
         WHERE register_id = ? AND kind = 'opening'",
    )
    .bind(register_id)
    .fetch_one(pool)
    .await?;

    let balance_minor = current_balance(pool, register_id).await?;

    Ok(CashRegisterSummary {
        register: CashRegisterRow {
            id,
            branch_id,
            code,
            name,
            status,
            current_user_id,
            balance_minor,
            movement_count,
        },
        opening_balance_minor,
        total_in_minor,
        total_out_minor,
        balance_minor,
        movement_count,
        today_in_minor: today.0.unwrap_or(0),
        today_out_minor: today.1.unwrap_or(0),
        kind_totals,
        last_closing: closing.map(
            |(id, closed_at, opening, booked, counted, variance, status)| CashClosingRow {
                id,
                register_id,
                closed_at,
                opening_balance_minor: opening,
                booked_balance_minor: booked,
                counted_balance_minor: counted,
                variance_minor: variance,
                status,
            },
        ),
    })
}
