//! Guardians & student credit (Phase 12.8).

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::AppResult;
use crate::people::context::{log_audit, Actor};

#[derive(Debug, Serialize, Clone)]
pub struct Guardian {
    pub id: i64,
    pub full_name: String,
    pub phone: String,
    pub email: String,
    pub national_id: String,
}

pub async fn create_guardian(
    pool: &SqlitePool,
    actor: &Actor,
    full_name: &str,
    phone: &str,
    email: &str,
    national_id: &str,
) -> AppResult<Guardian> {
    actor.require("students.edit")?;
    if full_name.trim().is_empty() {
        return Err(AppError::validation("اسم الولي مطلوب"));
    }
    let res = sqlx::query(
        "INSERT INTO guardians (full_name, phone, email, national_id) VALUES (?, ?, ?, ?)",
    )
    .bind(full_name.trim())
    .bind(phone)
    .bind(email)
    .bind(national_id)
    .execute(pool)
    .await?;
    let r = sqlx::query("SELECT id, full_name, phone, email, national_id FROM guardians WHERE id = ?")
        .bind(res.last_insert_rowid()).fetch_one(pool).await?;
    let g = Guardian {
        id: r.get("id"), full_name: r.get("full_name"), phone: r.get("phone"),
        email: r.get("email"), national_id: r.get("national_id"),
    };
    log_audit(pool, actor.user_id, "students.edit", "guardians", Some(g.id), None, None, "").await;
    Ok(g)
}

pub async fn link_guardian_to_student(
    pool: &SqlitePool,
    actor: &Actor,
    student_id: i64,
    guardian_id: i64,
    relation: &str,
    is_primary: bool,
) -> AppResult<()> {
    actor.require("students.edit")?;
    sqlx::query(
        "INSERT INTO student_guardians (student_id, guardian_id, relation, is_primary)
         VALUES (?, ?, ?, ?)
         ON CONFLICT (student_id, guardian_id) DO UPDATE SET relation=excluded.relation, is_primary=excluded.is_primary",
    )
    .bind(student_id)
    .bind(guardian_id)
    .bind(relation)
    .bind(is_primary as i64)
    .execute(pool)
    .await?;
    Ok(())
}

/// Apply student credit to an enrollment (reduces net fee via credit_applied_minor).
pub async fn apply_credit_to_enrollment(
    pool: &SqlitePool,
    actor: &Actor,
    student_id: i64,
    enrollment_id: i64,
    amount_minor: i64,
) -> AppResult<()> {
    actor.require("students.edit")?;
    if amount_minor <= 0 {
        return Err(AppError::validation("المبلغ يجب أن يكون أكبر من صفر"));
    }
    // check credit balance
    let balance: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor), 0) FROM student_credit_ledger WHERE student_id = ?",
    )
    .bind(student_id)
    .fetch_one(pool)
    .await?;
    if amount_minor > balance {
        return Err(AppError::Conflict("الرصيد غير كافٍ".into()));
    }
    // update enrollment credit_applied_minor
    sqlx::query(
        "UPDATE enrollments SET credit_applied_minor = credit_applied_minor + ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(amount_minor)
    .bind(enrollment_id)
    .execute(pool)
    .await?;
    // ledger entry: credit used
    sqlx::query(
        "INSERT INTO student_credit_ledger (student_id, enrollment_id, entry_type, amount_minor, reason, created_by)
         VALUES (?, ?, 'applied', ?, 'apply to enrollment', ?)",
    )
    .bind(student_id)
    .bind(enrollment_id)
    .bind(-amount_minor)
    .bind(actor.user_id)
    .execute(pool)
    .await?;
    log_audit(pool, actor.user_id, "students.edit", "enrollments", Some(enrollment_id), None, None, "credit applied").await;
    Ok(())
}
