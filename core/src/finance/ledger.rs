//! Double-entry ledger (Phase 6). Posted entries immutable (triggers).

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::AppResult;
use crate::people::context::Actor;

#[derive(Debug, Serialize, Clone)]
pub struct JournalLine {
    pub account_id: i64,
    pub debit_minor: i64,
    pub credit_minor: i64,
    pub memo: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct JournalEntry {
    pub id: i64,
    pub entry_no: i64,
    pub entry_date: String,
    pub currency: String,
    pub memo: String,
    pub total_debit_minor: i64,
    pub total_credit_minor: i64,
    pub status: String,
    pub source_type: String,
    pub source_id: i64,
    pub reversal_of_id: Option<i64>,
    pub lines: Vec<JournalLine>,
}

#[derive(Debug)]
pub struct NewJournalEntry {
    pub entry_date: String,
    pub currency: String,
    pub source_type: String,
    pub source_id: i64,
    pub memo: String,
    pub lines: Vec<JournalLine>,
}

fn validate_lines(lines: &[JournalLine]) -> AppResult<(i64, i64)> {
    if lines.is_empty() {
        return Err(AppError::validation("القيد بدون سطور"));
    }
    let mut td = 0i64;
    let mut tc = 0i64;
    for l in lines {
        if l.debit_minor < 0 || l.credit_minor < 0 {
            return Err(AppError::validation("مبالغ سالبة غير مسموحة"));
        }
        if l.debit_minor > 0 && l.credit_minor > 0 {
            return Err(AppError::validation("سطر بجانبي مدين ودائن"));
        }
        if l.debit_minor == 0 && l.credit_minor == 0 {
            return Err(AppError::validation("سطر صفر"));
        }
        td += l.debit_minor;
        tc += l.credit_minor;
    }
    if td != tc {
        return Err(AppError::validation(format!(
            "القيد غير متوازن: مدين {td} ≠ دائن {tc}"
        )));
    }
    Ok((td, tc))
}

/// Post a new journal entry (directly posted) in one transaction.
pub async fn post_entry(
    pool: &SqlitePool,
    actor: &Actor,
    new: &NewJournalEntry,
) -> AppResult<JournalEntry> {
    let (td, tc) = validate_lines(&new.lines)?;
    let currency = if new.currency.is_empty() { "ILS".to_string() } else { new.currency.clone() };

    let mut tx = pool.begin().await?;
    // seed sequence (year 0)
    sqlx::query(
        "INSERT INTO voucher_sequences (branch_id, kind, fiscal_year, last_no)
         SELECT b.id, 'journal_entry', 0, 0 FROM branches b WHERE b.code='MAIN'
         ON CONFLICT (branch_id, kind, fiscal_year) DO NOTHING",
    )
    .execute(&mut *tx)
    .await?;
    // get main branch id
    let branch_id: i64 = sqlx::query_scalar("SELECT id FROM branches WHERE code='MAIN'")
        .fetch_one(&mut *tx).await?;
    let no: i64 = sqlx::query_scalar(
        "UPDATE voucher_sequences SET last_no = last_no + 1
         WHERE branch_id = ? AND kind = 'journal_entry' AND fiscal_year = 0
         RETURNING last_no",
    )
    .bind(branch_id)
    .fetch_one(&mut *tx)
    .await?;

    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let res = sqlx::query(
        "INSERT INTO journal_entries (entry_no, entry_date, memo, currency,
         total_debit_minor, total_credit_minor, status, source_type, source_id,
         posted_by, posted_at)
         VALUES (?, ?, ?, ?, ?, ?, 'posted', ?, ?, ?, ?)",
    )
    .bind(no)
    .bind(&new.entry_date)
    .bind(&new.memo)
    .bind(&currency)
    .bind(td)
    .bind(tc)
    .bind(&new.source_type)
    .bind(new.source_id)
    .bind(actor.user_id)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    let entry_id = res.last_insert_rowid();

    for l in &new.lines {
        sqlx::query(
            "INSERT INTO journal_lines (entry_id, account_id, debit_minor, credit_minor, line_memo)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(entry_id)
        .bind(l.account_id)
        .bind(l.debit_minor)
        .bind(l.credit_minor)
        .bind(&l.memo)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    load_entry(pool, entry_id).await
}

pub async fn reverse_entry(
    pool: &SqlitePool,
    actor: &Actor,
    original_id: i64,
    reason: &str,
) -> AppResult<JournalEntry> {
    if reason.trim().is_empty() {
        return Err(AppError::validation("سبب الإلغاء مطلوب"));
    }
    let orig = load_entry(pool, original_id).await?;
    if orig.status != "posted" {
        return Err(AppError::Conflict("القيد ليس مرحّلاً".into()));
    }
    let mut lines = Vec::new();
    for l in &orig.lines {
        lines.push(JournalLine {
            account_id: l.account_id,
            debit_minor: l.credit_minor,
            credit_minor: l.debit_minor,
            memo: format!("إلغاء: {}", l.memo),
        });
    }
    let new = NewJournalEntry {
        entry_date: orig.entry_date.clone(),
        currency: orig.currency.clone(),
        source_type: orig.source_type.clone(),
        source_id: orig.source_id,
        memo: format!("إلغاء قيد {} — {}", orig.entry_no, reason),
        lines,
    };
    let (td, _) = validate_lines(&new.lines)?;

    let mut tx = pool.begin().await?;
    let branch_id: i64 = sqlx::query_scalar("SELECT id FROM branches WHERE code='MAIN'")
        .fetch_one(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO voucher_sequences (branch_id, kind, fiscal_year, last_no)
         SELECT id, 'journal_entry', 0, 0 FROM branches WHERE code='MAIN'
         ON CONFLICT (branch_id, kind, fiscal_year) DO NOTHING",
    )
    .execute(&mut *tx).await?;
    let no: i64 = sqlx::query_scalar(
        "UPDATE voucher_sequences SET last_no = last_no + 1
         WHERE branch_id = ? AND kind = 'journal_entry' AND fiscal_year = 0
         RETURNING last_no",
    )
    .bind(branch_id)
    .fetch_one(&mut *tx)
    .await?;

    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let res = sqlx::query(
        "INSERT INTO journal_entries (entry_no, entry_date, memo, currency,
         total_debit_minor, total_credit_minor, status, source_type, source_id,
         posted_by, posted_at, reversal_of_id)
         VALUES (?, ?, ?, ?, ?, ?, 'posted', ?, ?, ?, ?, ?)",
    )
    .bind(no)
    .bind(&new.entry_date)
    .bind(&new.memo)
    .bind(&new.currency)
    .bind(td)
    .bind(td)
    .bind(&new.source_type)
    .bind(new.source_id)
    .bind(actor.user_id)
    .bind(&now)
    .bind(original_id)
    .execute(&mut *tx)
    .await?;
    let new_id = res.last_insert_rowid();
    for l in &new.lines {
        sqlx::query(
            "INSERT INTO journal_lines (entry_id, account_id, debit_minor, credit_minor, line_memo)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(new_id)
        .bind(l.account_id)
        .bind(l.debit_minor)
        .bind(l.credit_minor)
        .bind(&l.memo)
        .execute(&mut *tx)
        .await?;
    }
    // mark original as reversed
    sqlx::query("UPDATE journal_entries SET status='reversed' WHERE id = ?")
        .bind(original_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    load_entry(pool, new_id).await
}

async fn load_entry(pool: &SqlitePool, id: i64) -> AppResult<JournalEntry> {
    let r = sqlx::query(
        "SELECT id, entry_no, entry_date, currency, memo, total_debit_minor,
                total_credit_minor, status, source_type, source_id, reversal_of_id
         FROM journal_entries WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    let lines_r = sqlx::query(
        "SELECT account_id, debit_minor, credit_minor, line_memo
         FROM journal_lines WHERE entry_id = ?",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    let lines = lines_r
        .iter()
        .map(|l| JournalLine {
            account_id: l.get("account_id"),
            debit_minor: l.get("debit_minor"),
            credit_minor: l.get("credit_minor"),
            memo: l.get("line_memo"),
        })
        .collect();
    Ok(JournalEntry {
        id: r.get("id"),
        entry_no: r.get("entry_no"),
        entry_date: r.get("entry_date"),
        currency: r.get("currency"),
        memo: r.get("memo"),
        total_debit_minor: r.get("total_debit_minor"),
        total_credit_minor: r.get("total_credit_minor"),
        status: r.get("status"),
        source_type: r.get("source_type"),
        source_id: r.get("source_id"),
        reversal_of_id: r.get("reversal_of_id"),
        lines,
    })
}

pub async fn account_balance(
    pool: &SqlitePool,
    account_id: i64,
) -> AppResult<i64> {
    let bal: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(debit_minor - credit_minor), 0)
         FROM journal_lines jl JOIN journal_entries je ON je.id = jl.entry_id
         WHERE jl.account_id = ? AND je.status = 'posted'",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await?;
    Ok(bal)
}

pub async fn ledger_balanced(pool: &SqlitePool) -> AppResult<bool> {
    let (td, tc): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(total_debit_minor),0), COALESCE(SUM(total_credit_minor),0)
         FROM journal_entries WHERE status='posted'",
    )
    .fetch_one(pool)
    .await?;
    Ok(td == tc)
}

/// Postable accounts (id + code + name + kind) — dropdowns for vouchers,
/// closings and the chart of accounts screen.
#[derive(Debug, Serialize, Clone)]
pub struct AccountOption {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub kind: String,
}

pub async fn list_accounts(pool: &SqlitePool) -> Result<Vec<AccountOption>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, code, name, kind FROM accounts
         WHERE is_postable = 1 AND is_active = 1 ORDER BY code",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| AccountOption {
            id: r.get("id"),
            code: r.get("code"),
            name: r.get("name"),
            kind: r.get("kind"),
        })
        .collect())
}
