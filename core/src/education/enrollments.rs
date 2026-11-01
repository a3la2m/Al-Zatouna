//! Enrollments (Phase 5): capacity rules, late-join eligibility, status
//! transitions, payment plan linkage.

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::AppResult;
use crate::people::context::{log_audit, snapshot, Actor};

#[derive(Debug, Serialize, Clone)]
pub struct Enrollment {
    pub id: i64,
    pub branch_id: i64,
    pub student_id: i64,
    pub course_id: i64,
    pub enrolled_date: String,
    pub base_fee_minor: i64,
    pub discount_minor: i64,
    pub waiver_minor: i64,
    pub credit_applied_minor: i64,
    pub net_fee_minor: i64,
    pub status: String,
    pub notes: String,
    pub frozen_reason: String,
}

fn row_enrollment(r: &sqlx::sqlite::SqliteRow) -> Enrollment {
    Enrollment {
        id: r.get("id"),
        branch_id: r.get("branch_id"),
        student_id: r.get("student_id"),
        course_id: r.get("course_id"),
        enrolled_date: r.get("enrolled_date"),
        base_fee_minor: r.get("base_fee_minor"),
        discount_minor: r.get("discount_minor"),
        waiver_minor: r.get("waiver_minor"),
        credit_applied_minor: r.get("credit_applied_minor"),
        net_fee_minor: r.get("net_fee_minor"),
        status: r.get("status"),
        notes: r.get("notes"),
        frozen_reason: r.get("frozen_reason"),
    }
}

const ENR_COLS: &str =
    "id, branch_id, student_id, course_id, enrolled_date, base_fee_minor,
     discount_minor, waiver_minor, credit_applied_minor, net_fee_minor,
     status, notes, frozen_reason";

async fn enrollment_by_id(pool: &SqlitePool, id: i64) -> AppResult<Enrollment> {
    let r = sqlx::query(&format!("SELECT {ENR_COLS} FROM enrollments WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("التسجيل غير موجود"))?;
    Ok(row_enrollment(&r))
}

/// Enroll a student in a course: enforces capacity rules + private-course
/// single-student limit (edu001/edu002).
pub async fn enroll(
    pool: &SqlitePool,
    actor: &Actor,
    branch_id: i64,
    student_id: i64,
    course_id: i64,
    base_fee_minor: i64,
    discount_minor: i64,
    waiver_minor: i64,
    enrolled_date: &str,
    notes: &str,
) -> AppResult<Enrollment> {
    actor.require("courses.edit")?; // proxy for enrollment permission
    // course + capacity
    let (is_private, max_students, min_students): (i64, i64, i64) =
        sqlx::query_as("SELECT is_private, max_students, min_students FROM courses WHERE id = ?")
            .bind(course_id)
            .fetch_one(pool)
            .await?;
    let active_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM enrollments WHERE course_id = ? AND status IN ('active','frozen')",
    )
    .bind(course_id)
    .fetch_one(pool)
    .await?;
    if is_private == 1 && active_count >= 1 {
        return Err(AppError::Conflict(
            "دورة خاصة: طالب واحد كحد أقصى".into(),
        ));
    }
    if max_students > 0 && active_count >= max_students {
        return Err(AppError::Conflict(format!(
            "الدورة ممتلئة ({active_count}/{max_students})"
        )));
    }
    // duplicate enrollment
    let dup: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM enrollments WHERE student_id = ? AND course_id = ?",
    )
    .bind(student_id)
    .bind(course_id)
    .fetch_one(pool)
    .await?;
    if dup > 0 {
        return Err(AppError::Conflict("الطالب مسجّل بالفعل في هذه الدورة".into()));
    }

    let net = base_fee_minor - discount_minor - waiver_minor;
    if net < 0 {
        return Err(AppError::validation("صافي الرسوم لا يمكن أن يكون سالباً"));
    }
    let res = sqlx::query(
        "INSERT INTO enrollments (branch_id, student_id, course_id, enrolled_date,
         base_fee_minor, discount_minor, waiver_minor, net_fee_minor, notes, created_by)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(branch_id)
    .bind(student_id)
    .bind(course_id)
    .bind(enrolled_date)
    .bind(base_fee_minor)
    .bind(discount_minor)
    .bind(waiver_minor)
    .bind(net)
    .bind(notes)
    .bind(actor.user_id)
    .execute(pool)
    .await?;
    let e = enrollment_by_id(pool, res.last_insert_rowid()).await?;
    log_audit(pool, actor.user_id, "enrollments.create", "enrollments", Some(e.id),
        None, Some(snapshot(&e)), "").await;
    Ok(e)
}

pub async fn list_enrollments(
    pool: &SqlitePool,
    actor: &Actor,
    course_id: Option<i64>,
    student_id: Option<i64>,
) -> AppResult<Vec<Enrollment>> {
    actor.require("courses.view")?;
    let sql = format!(
        "SELECT {ENR_COLS} FROM enrollments
         WHERE (?1 IS NULL OR course_id = ?1)
           AND (?2 IS NULL OR student_id = ?2)
         ORDER BY id DESC"
    );
    let q = sqlx::query(&sql)
    .bind(course_id)
    .bind(student_id);
    let rows = q.fetch_all(pool).await?;
    Ok(rows.iter().map(row_enrollment).collect())
}

pub async fn set_enrollment_status(
    pool: &SqlitePool,
    actor: &Actor,
    id: i64,
    new_status: &str,
    reason: &str,
) -> AppResult<Enrollment> {
    if !matches!(new_status, "active" | "completed" | "withdrawn" | "frozen" | "transferred" | "cancelled") {
        return Err(AppError::validation("حالة غير معروفة"));
    }
    if matches!(new_status, "withdrawn" | "cancelled" | "frozen") && reason.trim().is_empty() {
        return Err(AppError::validation("السبب مطلوب"));
    }
    actor.require("courses.edit")?;
    let before = enrollment_by_id(pool, id).await?;
    sqlx::query(
        "UPDATE enrollments SET status = ?2, frozen_reason = CASE WHEN ?2='frozen' THEN ?3 ELSE frozen_reason END,
         notes = CASE WHEN ?4 <> '' THEN ?4 ELSE notes END,
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?1",
    )
    .bind(id)
    .bind(new_status)
    .bind(reason)
    .bind(reason)
    .execute(pool)
    .await?;
    let after = enrollment_by_id(pool, id).await?;
    log_audit(pool, actor.user_id, "enrollments.status", "enrollments", Some(id),
        Some(snapshot(&before)), Some(snapshot(&after)), reason).await;
    Ok(after)
}
