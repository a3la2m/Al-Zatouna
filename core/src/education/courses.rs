//! Education domain (Phase 5): courses, schedules, sessions, attendance,
//! enrollments, agreements. Business logic in Rust (ADR-008).

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::common::seq::{allocate_next_no, SequenceKind};
use crate::error::AppError;
use crate::AppResult;
use crate::people::context::{log_audit, snapshot, Actor};

const COURSE_COLS: &str =
    "id, branch_id, course_no, name, subject_id, description, start_date, end_date,
     default_time_json, teacher_id, room_id, expected_students, min_students,
     max_students, fee_minor, status, is_private, created_by, approved_by,
     approved_at, notes, created_at, updated_at";

#[derive(Debug, Serialize, Clone)]
pub struct Course {
    pub id: i64,
    pub branch_id: i64,
    pub course_no: i64,
    pub name: String,
    pub subject_id: i64,
    pub description: String,
    pub start_date: String,
    pub end_date: String,
    pub teacher_id: i64,
    pub room_id: i64,
    pub expected_students: i64,
    pub min_students: i64,
    pub max_students: i64,
    pub fee_minor: i64,
    pub status: String,
    pub is_private: bool,
    pub created_by: i64,
    pub approved_by: Option<i64>,
    pub approved_at: Option<String>,
    pub notes: String,
}

#[derive(Debug, Default)]
pub struct NewCourse {
    pub name: String,
    pub subject_id: i64,
    pub description: String,
    pub start_date: String,
    pub end_date: String,
    pub teacher_id: i64,
    pub room_id: i64,
    pub expected_students: i64,
    pub min_students: i64,
    pub max_students: i64,
    pub fee_minor: i64,
    pub is_private: bool,
    pub notes: String,
}

fn row_course(r: &sqlx::sqlite::SqliteRow) -> Course {
    Course {
        id: r.get("id"),
        branch_id: r.get("branch_id"),
        course_no: r.get("course_no"),
        name: r.get("name"),
        subject_id: r.get("subject_id"),
        description: r.get("description"),
        start_date: r.get("start_date"),
        end_date: r.get("end_date"),
        teacher_id: r.get("teacher_id"),
        room_id: r.get("room_id"),
        expected_students: r.get("expected_students"),
        min_students: r.get("min_students"),
        max_students: r.get("max_students"),
        fee_minor: r.get("fee_minor"),
        status: r.get("status"),
        is_private: r.get::<i64, _>("is_private") == 1,
        created_by: r.get("created_by"),
        approved_by: r.get("approved_by"),
        approved_at: r.get("approved_at"),
        notes: r.get("notes"),
    }
}

async fn course_by_id(pool: &SqlitePool, id: i64) -> AppResult<Course> {
    let r = sqlx::query(&format!("SELECT {COURSE_COLS} FROM courses WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("الدورة غير موجودة"))?;
    Ok(row_course(&r))
}

pub async fn create_course(
    pool: &SqlitePool,
    actor: &Actor,
    branch_id: i64,
    new: &NewCourse,
) -> AppResult<Course> {
    actor.require("courses.create")?;
    if new.name.trim().is_empty() {
        return Err(AppError::validation("اسم الدورة مطلوب"));
    }
    if new.subject_id == 0 || new.teacher_id == 0 || new.room_id == 0 {
        return Err(AppError::validation("المادة والمدرس والقاعة مطلوبة"));
    }
    let mut tx = pool.begin().await?;
    let no = allocate_next_no(&mut tx, branch_id, SequenceKind::CourseNo, 0).await?;
    let res = sqlx::query(
        "INSERT INTO courses (branch_id, course_no, name, subject_id, description,
         start_date, end_date, teacher_id, room_id, expected_students,
         min_students, max_students, fee_minor, is_private, notes, created_by)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(branch_id)
    .bind(no)
    .bind(new.name.trim())
    .bind(new.subject_id)
    .bind(&new.description)
    .bind(&new.start_date)
    .bind(&new.end_date)
    .bind(new.teacher_id)
    .bind(new.room_id)
    .bind(new.expected_students)
    .bind(new.min_students)
    .bind(new.max_students)
    .bind(new.fee_minor)
    .bind(new.is_private as i64)
    .bind(&new.notes)
    .bind(actor.user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let c = course_by_id(pool, res.last_insert_rowid()).await?;
    log_audit(pool, actor.user_id, "courses.create", "courses", Some(c.id),
        None, Some(snapshot(&c)), "").await;
    Ok(c)
}

pub async fn get_course(pool: &SqlitePool, actor: &Actor, id: i64) -> AppResult<Course> {
    actor.require("courses.view")?;
    course_by_id(pool, id).await
}

pub async fn list_courses(
    pool: &SqlitePool,
    actor: &Actor,
    branch_id: i64,
    status: Option<&str>,
) -> AppResult<Vec<Course>> {
    actor.require("courses.view")?;
    let sql = format!(
        "SELECT {COURSE_COLS} FROM courses WHERE branch_id = ?1
          AND (?2 IS NULL OR status = ?2) ORDER BY id DESC"
    );
    let q = sqlx::query(&sql)
    .bind(branch_id)
    .bind(status);
    let rows = q.fetch_all(pool).await?;
    Ok(rows.iter().map(row_course).collect())
}

/// Status transition with audit; maker-checker for approve handled by caller
/// (shell checks permission `courses.approve`).
pub async fn set_course_status(
    pool: &SqlitePool,
    actor: &Actor,
    id: i64,
    new_status: &str,
    reason: &str,
) -> AppResult<Course> {
    const LEGAL: &[(&str, &str)] = &[
        ("draft", "registration_open"),
        ("draft", "cancelled"),
        ("registration_open", "active"),
        ("registration_open", "cancelled"),
        ("active", "completed"),
        ("active", "suspended"),
        ("suspended", "active"),
        ("suspended", "cancelled"),
        ("completed", "archived"),
    ];
    let before = course_by_id(pool, id).await?;
    if new_status == "archived" && reason.trim().is_empty() {
        return Err(AppError::validation("أرشفة تتطلب سبباً"));
    }
    if !LEGAL.iter().any(|(a, b)| *a == before.status && *b == new_status) {
        return Err(AppError::Conflict(format!(
            "انتقال غير مسموح: {} → {}", before.status, new_status
        )));
    }
    // approve gate: registration_open→active requires courses.approve
    if new_status == "active" {
        actor.require("courses.approve")?;
    } else {
        actor.require("courses.edit")?;
    }
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE courses SET status = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?1",
    )
    .bind(id)
    .bind(new_status)
    .execute(&mut *tx)
    .await?;
    if new_status == "active" {
        sqlx::query(
            "UPDATE courses SET approved_by = ?2, approved_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?1",
        )
        .bind(id)
        .bind(actor.user_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    let after = course_by_id(pool, id).await?;
    log_audit(pool, actor.user_id, "courses.edit", "courses", Some(id),
        Some(snapshot(&before)), Some(snapshot(&after)), reason).await;
    Ok(after)
}

/// Maker-checker: approving own course is blocked unless override reason supplied.
pub fn check_maker_checker(creator: i64, approver: i64, override_reason: Option<&str>) -> AppResult<()> {
    if creator == approver && override_reason.is_none() {
        return Err(AppError::permission(
            "الاعتماد الذاتي غير مسموح — أدخل سبب تجاوز",
        ));
    }
    Ok(())
}
