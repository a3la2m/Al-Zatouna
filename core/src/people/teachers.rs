//! Teacher + catalog domain services (Phase 4). Teachers get the same
//! create/list/edit/status lifecycle as students (permission `teachers.*`),
//! and manage subjects/rooms (`*.manage` is unnamed — see code below).

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::common::seq::{allocate_next_no, SequenceKind};
use crate::error::AppError;
use crate::AppResult;
use crate::people::context::{log_audit, snapshot, Actor};

const TEACHER_COLS: &str =
    "id, branch_id, teacher_no, full_name, phone, email, specialty,
     qualifications, address, hire_date, status, notes";

#[derive(Debug, Serialize, Clone)]
pub struct Teacher {
    pub id: i64,
    pub branch_id: i64,
    pub teacher_no: i64,
    pub full_name: String,
    pub phone: String,
    pub email: String,
    pub specialty: String,
    pub qualifications: String,
    pub address: String,
    pub hire_date: String,
    pub status: String,
    pub notes: String,
}

#[derive(Debug, Default)]
pub struct NewTeacher {
    pub full_name: String,
    pub phone: String,
    pub email: String,
    pub specialty: String,
    pub qualifications: String,
    pub address: String,
    pub hire_date: String,
    pub notes: String,
}

#[derive(Debug, Default)]
pub struct TeacherFilter {
    pub status: Option<String>,
    pub search: Option<String>,
    pub limit: i64,
}

fn row_teacher(r: &sqlx::sqlite::SqliteRow) -> Teacher {
    Teacher {
        id: r.get("id"),
        branch_id: r.get("branch_id"),
        teacher_no: r.get("teacher_no"),
        full_name: r.get("full_name"),
        phone: r.get("phone"),
        email: r.get("email"),
        specialty: r.get("specialty"),
        qualifications: r.get("qualifications"),
        address: r.get("address"),
        hire_date: r.get("hire_date"),
        status: r.get("status"),
        notes: r.get("notes"),
    }
}

async fn teacher_by_id(pool: &SqlitePool, id: i64) -> AppResult<Teacher> {
    let r = sqlx::query(&format!("SELECT {TEACHER_COLS} FROM teachers WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("المدرس غير موجود"))?;
    Ok(row_teacher(&r))
}

pub async fn create_teacher(
    pool: &SqlitePool,
    actor: &Actor,
    branch_id: i64,
    new: &NewTeacher,
) -> AppResult<Teacher> {
    actor.require("teachers.manage")?;
    if new.full_name.trim().is_empty() {
        return Err(AppError::validation("اسم المدرس مطلوب"));
    }
    let mut tx = pool.begin().await?;
    let no = allocate_next_no(&mut tx, branch_id, SequenceKind::TeacherNo, 0).await?;
    let res = sqlx::query(
        "INSERT INTO teachers (branch_id, teacher_no, full_name, phone, email,
         specialty, qualifications, address, hire_date, notes)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(branch_id)
    .bind(no)
    .bind(new.full_name.trim())
    .bind(&new.phone)
    .bind(&new.email)
    .bind(&new.specialty)
    .bind(&new.qualifications)
    .bind(&new.address)
    .bind(&new.hire_date)
    .bind(&new.notes)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let t = teacher_by_id(pool, res.last_insert_rowid()).await?;
    log_audit(pool, actor.user_id, "teachers.create", "teachers", Some(t.id),
        None, Some(snapshot(&t)), "").await;
    Ok(t)
}

pub async fn list_teachers(
    pool: &SqlitePool,
    actor: &Actor,
    branch_id: i64,
    f: &TeacherFilter,
) -> AppResult<Vec<Teacher>> {
    actor.require("teachers.view")?;
    let sql = format!(
        "SELECT {TEACHER_COLS} FROM teachers
         WHERE branch_id = ?4
           AND (?1 IS NULL OR status = ?1)
           AND (?2 IS NULL OR full_name LIKE '%' || ?2 || '%'
                OR phone LIKE '%' || ?2 || '%'
                OR specialty LIKE '%' || ?2 || '%')
         ORDER BY id DESC
         LIMIT ?3"
    );
    let q = sqlx::query(&sql)
    .bind(&f.status)
    .bind(&f.search)
    .bind(f.limit.max(1).min(500))
    .bind(branch_id);
    let rows = q.fetch_all(pool).await?;
    Ok(rows.iter().map(row_teacher).collect())
}

#[derive(Default)]
pub struct TeacherPatch {
    pub full_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub specialty: Option<String>,
    pub qualifications: Option<String>,
    pub address: Option<String>,
    pub hire_date: Option<String>,
    pub notes: Option<String>,
}

pub async fn edit_teacher(
    pool: &SqlitePool,
    actor: &Actor,
    id: i64,
    patch: &TeacherPatch,
) -> AppResult<Teacher> {
    actor.require("teachers.manage")?;
    let before = teacher_by_id(pool, id).await?;
    sqlx::query(
        "UPDATE teachers SET
           full_name      = COALESCE(?2, full_name),
           phone          = COALESCE(?3, phone),
           email          = COALESCE(?4, email),
           specialty      = COALESCE(?5, specialty),
           qualifications = COALESCE(?6, qualifications),
           address        = COALESCE(?7, address),
           hire_date      = COALESCE(?8, hire_date),
           notes          = COALESCE(?9, notes),
           updated_at     = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?1",
    )
    .bind(id)
    .bind(patch.full_name.clone())
    .bind(patch.phone.clone())
    .bind(patch.email.clone())
    .bind(patch.specialty.clone())
    .bind(patch.qualifications.clone())
    .bind(patch.address.clone())
    .bind(patch.hire_date.clone())
    .bind(patch.notes.clone())
    .execute(pool)
    .await?;
    let after = teacher_by_id(pool, id).await?;
    log_audit(pool, actor.user_id, "teachers.edit", "teachers", Some(id),
        Some(snapshot(&before)), Some(snapshot(&after)), "").await;
    Ok(after)
}

pub async fn set_teacher_status(
    pool: &SqlitePool,
    actor: &Actor,
    id: i64,
    new_status: &str,
    reason: &str,
) -> AppResult<Teacher> {
    if !matches!(new_status, "active" | "inactive" | "archived") {
        return Err(AppError::validation("حالة غير معروفة"));
    }
    if new_status == "archived" && reason.trim().is_empty() {
        return Err(AppError::validation("أرشفة المدرس تتطلب سبباً"));
    }
    actor.require("teachers.manage")?;
    let before = teacher_by_id(pool, id).await?;
    sqlx::query("UPDATE teachers SET status = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?1")
        .bind(id)
        .bind(new_status)
        .execute(pool)
        .await?;
    let after = teacher_by_id(pool, id).await?;
    log_audit(pool, actor.user_id, "teachers.edit", "teachers", Some(id),
        Some(snapshot(&before)), Some(snapshot(&after)), reason).await;
    Ok(after)
}

// ---------- catalog: subjects ----------

#[derive(Debug, Serialize, Clone)]
pub struct Subject {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub is_active: bool,
}

fn row_subject(r: &sqlx::sqlite::SqliteRow) -> Subject {
    Subject {
        id: r.get("id"),
        code: r.get("code"),
        name: r.get("name"),
        description: r.get("description"),
        is_active: r.get::<i64, _>("is_active") == 1,
    }
}

pub async fn create_subject(
    pool: &SqlitePool,
    actor: &Actor,
    code: &str,
    name: &str,
    description: &str,
) -> AppResult<Subject> {
    actor.require("subjects.manage")?;
    if code.trim().is_empty() || name.trim().is_empty() {
        return Err(AppError::validation("الكود والاسم مطلوبان"));
    }
    let res = sqlx::query(
        "INSERT INTO subjects (code, name, description) VALUES (?, ?, ?)",
    )
    .bind(code.trim())
    .bind(name.trim())
    .bind(description)
    .execute(pool)
    .await?;
    let r = sqlx::query("SELECT id, code, name, description, is_active FROM subjects WHERE id = ?")
        .bind(res.last_insert_rowid())
        .fetch_one(pool)
        .await?;
    let s = row_subject(&r);
    log_audit(pool, actor.user_id, "courses.create", "subjects", Some(s.id),
        None, Some(snapshot(&s)), "").await;
    Ok(s)
}

pub async fn list_subjects(
    pool: &SqlitePool,
    actor: &Actor,
    active_only: bool,
) -> AppResult<Vec<Subject>> {
    actor.require("courses.view")?;
    let q = sqlx::query(
        "SELECT id, code, name, description, is_active FROM subjects
         WHERE (?1 = 0 OR is_active = 1) ORDER BY name",
    )
    .bind(active_only as i64);
    let rows = q.fetch_all(pool).await?;
    Ok(rows.iter().map(row_subject).collect())
}

// ---------- catalog: rooms ----------

#[derive(Debug, Serialize, Clone)]
pub struct Room {
    pub id: i64,
    pub branch_id: i64,
    pub code: String,
    pub name: String,
    pub capacity: i64,
    pub description: String,
    pub status: String,
}

fn row_room(r: &sqlx::sqlite::SqliteRow) -> Room {
    Room {
        id: r.get("id"),
        branch_id: r.get("branch_id"),
        code: r.get("code"),
        name: r.get("name"),
        capacity: r.get("capacity"),
        description: r.get("description"),
        status: r.get("status"),
    }
}

pub async fn create_room(
    pool: &SqlitePool,
    actor: &Actor,
    branch_id: i64,
    code: &str,
    name: &str,
    capacity: i64,
    description: &str,
) -> AppResult<Room> {
    actor.require("rooms.manage")?;
    if code.trim().is_empty() || name.trim().is_empty() {
        return Err(AppError::validation("الكود والاسم مطلوبان"));
    }
    if capacity < 0 {
        return Err(AppError::validation("السعة لا يمكن أن تكون سالبة"));
    }
    let res = sqlx::query(
        "INSERT INTO rooms (branch_id, code, name, capacity, description)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(branch_id)
    .bind(code.trim())
    .bind(name.trim())
    .bind(capacity)
    .bind(description)
    .execute(pool)
    .await?;
    let r = sqlx::query(
        "SELECT id, branch_id, code, name, capacity, description, status FROM rooms WHERE id = ?",
    )
    .bind(res.last_insert_rowid())
    .fetch_one(pool)
    .await?;
    let room = row_room(&r);
    log_audit(pool, actor.user_id, "rooms.manage", "rooms", Some(room.id),
        None, Some(snapshot(&room)), "").await;
    Ok(room)
}

pub async fn list_rooms(
    pool: &SqlitePool,
    actor: &Actor,
    branch_id: i64,
) -> AppResult<Vec<Room>> {
    actor.require("courses.view")?;
    let q = sqlx::query(
        "SELECT id, branch_id, code, name, capacity, description, status FROM rooms
         WHERE branch_id = ? AND status <> 'archived' ORDER BY code",
    )
    .bind(branch_id);
    let rows = q.fetch_all(pool).await?;
    Ok(rows.iter().map(row_room).collect())
}
