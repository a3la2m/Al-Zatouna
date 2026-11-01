//! Student domain service (Phase 4): create / list / get / edit / freeze /
//! archive. Every mutation is audited; the student_no sequence is minted
//! inside the same INSERT transaction (ADR-010).

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::common::seq::{allocate_next_no, SequenceKind};
use crate::error::AppError;
use crate::AppResult;
use crate::people::context::{log_audit, snapshot, Actor};

/// Row shape returned by reads (serialized straight to the UI).
#[derive(Debug, Serialize, Clone)]
pub struct Student {
    pub id: i64,
    pub branch_id: i64,
    pub student_no: i64,
    pub full_name: String,
    pub national_id: String,
    pub birth_date: String,
    pub gender: Option<String>,
    pub phone: String,
    pub email: String,
    pub address: String,
    pub notes: String,
    pub status: String,
    pub registered_at: String,
}

#[derive(Debug, Default)]
pub struct NewStudent {
    pub full_name: String,
    pub national_id: String,
    pub birth_date: String,
    pub gender: Option<String>,
    pub phone: String,
    pub email: String,
    pub address: String,
    pub notes: String,
}

/// List filter; empty fields mean "no filter".
#[derive(Debug, Default)]
pub struct StudentFilter {
    pub status: Option<String>,
    pub search: Option<String>,
    pub limit: i64,
}

impl StudentFilter {
    pub fn new() -> Self {
        Self { limit: 100, ..Default::default() }
    }
}

fn row_to_student(r: &sqlx::sqlite::SqliteRow) -> Student {
    Student {
        id: r.get("id"),
        branch_id: r.get("branch_id"),
        student_no: r.get("student_no"),
        full_name: r.get("full_name"),
        national_id: r.get("national_id"),
        birth_date: r.get("birth_date"),
        gender: r.get("gender"),
        phone: r.get("phone"),
        email: r.get("email"),
        address: r.get("address"),
        notes: r.get("notes"),
        status: r.get("status"),
        registered_at: r.get("registered_at"),
    }
}

const STUDENT_COLS: &str =
    "id, branch_id, student_no, full_name, national_id, birth_date, gender,
     phone, email, address, notes, status, registered_at";

async fn fetch_one(pool: &SqlitePool, id: i64) -> AppResult<Student> {
    let r = sqlx::query(&format!(
        "SELECT {STUDENT_COLS} FROM students WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("الطالب غير موجود"))?;
    Ok(row_to_student(&r))
}

/// Create a student: validates, mints the yearly-free student_no inside the
/// insert transaction, audits `students.create`.
pub async fn create_student(
    pool: &SqlitePool,
    actor: &Actor,
    branch_id: i64,
    new: &NewStudent,
) -> AppResult<Student> {
    actor.require("students.create")?;
    if new.full_name.trim().is_empty() {
        return Err(AppError::validation("اسم الطالب مطلوب"));
    }
    if !new.national_id.is_empty() {
        let dup: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM students
             WHERE national_id = ? AND branch_id = ?",
        )
        .bind(&new.national_id)
        .bind(branch_id)
        .fetch_one(pool)
        .await?;
        if dup > 0 {
            return Err(AppError::Conflict(
                "رقم الهوية مسجّل لطالب آخر".into(),
            ));
        }
    }

    let mut tx = pool.begin().await?;
    let no = allocate_next_no(&mut tx, branch_id, SequenceKind::StudentNo, 0).await?;
    let res = sqlx::query(
        "INSERT INTO students (branch_id, student_no, full_name, national_id,
         birth_date, gender, phone, email, address, notes)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(branch_id)
    .bind(no)
    .bind(new.full_name.trim())
    .bind(&new.national_id)
    .bind(&new.birth_date)
    .bind(&new.gender)
    .bind(&new.phone)
    .bind(&new.email)
    .bind(&new.address)
    .bind(&new.notes)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let id = res.last_insert_rowid();
    let student = fetch_one(pool, id).await?;
    log_audit_for(pool, actor.user_id, "students.create", id, None,
        Some(snapshot(&student)), "").await;
    Ok(student)
}

/// Search/list with optional status + text search (name/phone/national id).
pub async fn list_students(
    pool: &SqlitePool,
    actor: &Actor,
    branch_id: i64,
    f: &StudentFilter,
) -> AppResult<Vec<Student>> {
    actor.require("students.view")?;
    let sql = format!(
        "SELECT {STUDENT_COLS} FROM students
         WHERE branch_id = ?4
           AND (?1 IS NULL OR status = ?1)
           AND (?2 IS NULL OR full_name LIKE '%' || ?2 || '%'
                OR phone LIKE '%' || ?2 || '%'
                OR national_id LIKE '%' || ?2 || '%')
         ORDER BY id DESC
         LIMIT ?3"
    );
    let q = sqlx::query(&sql)
    .bind(&f.status)
    .bind(&f.search)
    .bind(f.limit.max(1).min(500))
    .bind(branch_id);
    let rows = q.fetch_all(pool).await?;
    Ok(rows.iter().map(row_to_student).collect())
}

/// Partial edit: only supplied (Some) fields change; audited with before/after.
#[derive(Default)]
pub struct StudentPatch {
    pub full_name: Option<String>,
    pub national_id: Option<String>,
    pub birth_date: Option<String>,
    pub gender: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub notes: Option<String>,
}

pub async fn edit_student(
    pool: &SqlitePool,
    actor: &Actor,
    id: i64,
    patch: &StudentPatch,
) -> AppResult<Student> {
    actor.require("students.edit")?;
    let before = fetch_one(pool, id).await?;
    sqlx::query(
        "UPDATE students SET
           full_name   = COALESCE(?2, full_name),
           national_id = COALESCE(?3, national_id),
           birth_date  = COALESCE(?4, birth_date),
           gender      = COALESCE(?5, gender),
           phone       = COALESCE(?6, phone),
           email       = COALESCE(?7, email),
           address     = COALESCE(?8, address),
           notes       = COALESCE(?9, notes),
           updated_at  = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?1",
    )
    .bind(id)
    .bind(patch.full_name.clone())
    .bind(patch.national_id.clone())
    .bind(patch.birth_date.clone())
    .bind(patch.gender.clone())
    .bind(patch.phone.clone())
    .bind(patch.email.clone())
    .bind(patch.address.clone())
    .bind(patch.notes.clone())
    .execute(pool)
    .await?;
    let after = fetch_one(pool, id).await?;
    log_audit_for(pool, actor.user_id, "students.edit", id,
        Some(snapshot(&before)), Some(snapshot(&after)), "").await;
    Ok(after)
}

/// Status transitions with audit reason (mandatory for archive).
pub async fn set_student_status(
    pool: &SqlitePool,
    actor: &Actor,
    id: i64,
    new_status: &str,
    reason: &str,
) -> AppResult<Student> {
    if !matches!(new_status, "active" | "frozen" | "archived") {
        return Err(AppError::validation("حالة غير معروفة"));
    }
    // archive needs an explicit reason
    if new_status == "archived" && reason.trim().is_empty() {
        return Err(AppError::validation(
            "أرشفة الطالب تتطلب سبباً مكتوباً",
        ));
    }
    actor.require("students.edit")?;
    let before = fetch_one(pool, id).await?;
    sqlx::query("UPDATE students SET status = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?1")
        .bind(id)
        .bind(new_status)
        .execute(pool)
        .await?;
    let after = fetch_one(pool, id).await?;
    log_audit_for(pool, actor.user_id, "students.edit", id,
        Some(snapshot(&before)), Some(snapshot(&after)), reason).await;
    Ok(after)
}

/// Audit helper local to people domain (delegates to the shared one).
async fn log_audit_for(
    pool: &SqlitePool,
    user_id: i64,
    action: &'static str,
    entity_id: i64,
    old: Option<serde_json::Value>,
    new: Option<serde_json::Value>,
    reason: &str,
) {
    log_audit(pool, user_id, action, "students", Some(entity_id), old, new, reason).await;
}
