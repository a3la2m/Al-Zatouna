//! Schedules and sessions (Phase 5).

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::AppResult;
use crate::people::context::{log_audit, snapshot, Actor};

#[derive(Debug, Serialize, Clone)]
pub struct CourseSchedule {
    pub id: i64,
    pub course_id: i64,
    pub weekday: i64,
    pub start_time: String,
    pub end_time: String,
    pub room_id: i64,
    pub teacher_id: i64,
    pub effective_from: String,
    pub effective_to: String,
}

pub async fn add_schedule(
    pool: &SqlitePool,
    actor: &Actor,
    course_id: i64,
    weekday: i64,
    start_time: &str,
    end_time: &str,
    room_id: i64,
    teacher_id: i64,
) -> AppResult<CourseSchedule> {
    actor.require("schedule.manage")?;
    if !(1..=7).contains(&weekday) {
        return Err(AppError::validation("يوم غير صالح"));
    }
    if start_time >= end_time {
        return Err(AppError::validation("وقت البداية يجب أن يسبق الانتهاء"));
    }
    let res = sqlx::query(
        "INSERT INTO course_schedules (course_id, weekday, start_time, end_time,
         room_id, teacher_id) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(course_id)
    .bind(weekday)
    .bind(start_time)
    .bind(end_time)
    .bind(room_id)
    .bind(teacher_id)
    .execute(pool)
    .await?;
    let r = sqlx::query(
        "SELECT id, course_id, weekday, start_time, end_time, room_id, teacher_id,
                effective_from, effective_to FROM course_schedules WHERE id = ?",
    )
    .bind(res.last_insert_rowid())
    .fetch_one(pool)
    .await?;
    let s = CourseSchedule {
        id: r.get("id"),
        course_id: r.get("course_id"),
        weekday: r.get("weekday"),
        start_time: r.get("start_time"),
        end_time: r.get("end_time"),
        room_id: r.get("room_id"),
        teacher_id: r.get("teacher_id"),
        effective_from: r.get("effective_from"),
        effective_to: r.get("effective_to"),
    };
    log_audit(pool, actor.user_id, "schedule.manage", "course_schedules", Some(s.id),
        None, Some(snapshot(&s)), "").await;
    Ok(s)
}

// ---------- sessions ----------

#[derive(Debug, Serialize, Clone)]
pub struct Session {
    pub id: i64,
    pub course_id: i64,
    pub source_schedule_id: Option<i64>,
    pub seq_in_course: i64,
    pub session_date: String,
    pub start_time: String,
    pub end_time: String,
    pub room_id: i64,
    pub teacher_id: i64,
    pub substitute_teacher_id: Option<i64>,
    pub status: String,
    pub session_kind: String,
    pub original_session_id: Option<i64>,
    pub reason: String,
}

fn row_session(r: &sqlx::sqlite::SqliteRow) -> Session {
    Session {
        id: r.get("id"),
        course_id: r.get("course_id"),
        source_schedule_id: r.get("source_schedule_id"),
        seq_in_course: r.get("seq_in_course"),
        session_date: r.get("session_date"),
        start_time: r.get("start_time"),
        end_time: r.get("end_time"),
        room_id: r.get("room_id"),
        teacher_id: r.get("teacher_id"),
        substitute_teacher_id: r.get("substitute_teacher_id"),
        status: r.get("status"),
        session_kind: r.get("session_kind"),
        original_session_id: r.get("original_session_id"),
        reason: r.get("reason"),
    }
}

const SESSION_COLS: &str =
    "id, course_id, source_schedule_id, seq_in_course, session_date, start_time,
     end_time, room_id, teacher_id, substitute_teacher_id, status, session_kind,
     original_session_id, reason";

async fn session_by_id(pool: &SqlitePool, id: i64) -> AppResult<Session> {
    let r = sqlx::query(&format!("SELECT {SESSION_COLS} FROM sessions WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("الحصة غير موجودة"))?;
    Ok(row_session(&r))
}

/// Room/teacher clash check for a candidate session slot.
async fn clash_check(
    pool: &SqlitePool,
    room_id: i64,
    teacher_id: i64,
    substitute: Option<i64>,
    date: &str,
    start: &str,
    end: &str,
    exclude_session: Option<i64>,
) -> AppResult<()> {
    // room overlap
    let q = sqlx::query(
        "SELECT COUNT(*) FROM sessions
         WHERE room_id = ? AND session_date = ?
           AND start_time < ? AND end_time > ?
           AND status NOT IN ('cancelled')
           AND (? IS NULL OR id <> ?)",
    )
    .bind(room_id)
    .bind(date)
    .bind(end)
    .bind(start)
    .bind(exclude_session)
    .bind(exclude_session);
    let c: i64 = q.fetch_one(pool).await?.get(0);
    if c > 0 {
        return Err(AppError::Conflict("تعارض في القاعة: حصة متداخلة".into()));
    }
    // teacher overlap (primary or substitute)
    let q = sqlx::query(
        "SELECT COUNT(*) FROM sessions
         WHERE session_date = ? AND start_time < ? AND end_time > ?
           AND status NOT IN ('cancelled')
           AND (teacher_id = ? OR substitute_teacher_id = ?)
           AND (? IS NULL OR id <> ?)",
    )
    .bind(date)
    .bind(end)
    .bind(start)
    .bind(teacher_id)
    .bind(teacher_id)
    .bind(exclude_session)
    .bind(exclude_session);
    let c: i64 = q.fetch_one(pool).await?.get(0);
    if c > 0 {
        return Err(AppError::Conflict("تعارض في المدرس: حصة متداخلة".into()));
    }
    if let Some(sub) = substitute {
        let q = sqlx::query(
            "SELECT COUNT(*) FROM sessions
             WHERE session_date = ? AND start_time < ? AND end_time > ?
               AND status NOT IN ('cancelled')
               AND (teacher_id = ? OR substitute_teacher_id = ?)
               AND (? IS NULL OR id <> ?)",
        )
        .bind(date)
        .bind(end)
        .bind(start)
        .bind(sub)
        .bind(sub)
        .bind(exclude_session)
        .bind(exclude_session);
        let c: i64 = q.fetch_one(pool).await?.get(0);
        if c > 0 {
            return Err(AppError::Conflict("تعارض في المدرس البديل".into()));
        }
    }
    Ok(())
}

pub async fn create_session(
    pool: &SqlitePool,
    actor: &Actor,
    course_id: i64,
    session_date: &str,
    start_time: &str,
    end_time: &str,
    room_id: i64,
    teacher_id: i64,
    substitute_teacher_id: Option<i64>,
    seq_in_course: i64,
    source_schedule_id: Option<i64>,
) -> AppResult<Session> {
    actor.require("sessions.executed")?; // scheduling permission proxy
    actor.require("schedule.manage")?;
    if start_time >= end_time {
        return Err(AppError::validation("وقت البداية يجب أن يسبق الانتهاء"));
    }
    clash_check(pool, room_id, teacher_id, substitute_teacher_id,
        session_date, start_time, end_time, None).await?;

    let res = sqlx::query(
        "INSERT INTO sessions (course_id, source_schedule_id, seq_in_course,
         session_date, start_time, end_time, room_id, teacher_id,
         substitute_teacher_id, created_by)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(course_id)
    .bind(source_schedule_id)
    .bind(seq_in_course)
    .bind(session_date)
    .bind(start_time)
    .bind(end_time)
    .bind(room_id)
    .bind(teacher_id)
    .bind(substitute_teacher_id)
    .bind(actor.user_id)
    .execute(pool)
    .await?;
    let s = session_by_id(pool, res.last_insert_rowid()).await?;
    log_audit(pool, actor.user_id, "sessions.change", "sessions", Some(s.id),
        None, Some(snapshot(&s)), "").await;
    Ok(s)
}

/// Legal session status transitions (edu005).
pub async fn set_session_status(
    pool: &SqlitePool,
    actor: &Actor,
    id: i64,
    new_status: &str,
    reason: &str,
    substitute_teacher_id: Option<i64>,
    reschedule_to_id: Option<i64>,
) -> AppResult<Session> {
    const LEGAL: &[(&str, &str)] = &[
        ("scheduled", "executed"),
        ("scheduled", "cancelled"),
        ("scheduled", "postponed"),
        ("scheduled", "makeup"),
        ("postponed", "scheduled"),
        ("postponed", "cancelled"),
    ];
    let before = session_by_id(pool, id).await?;
    if !LEGAL.iter().any(|(a, b)| *a == before.status && *b == new_status) {
        return Err(AppError::Conflict(format!(
            "انتقال غير مسموح: {} → {}", before.status, new_status
        )));
    }
    if matches!(new_status, "cancelled" | "postponed") && reason.trim().is_empty() {
        return Err(AppError::validation("السبب مطلوب"));
    }
    actor.require("sessions.cancel")?;
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    sqlx::query(
        "UPDATE sessions SET status = ?2, reason = ?3,
         substitute_teacher_id = COALESCE(?4, substitute_teacher_id),
         reschedule_to_id = ?5,
         cancelled_by = CASE WHEN ?2 IN ('cancelled','postponed') THEN ?6 ELSE cancelled_by END,
         executed_at = CASE WHEN ?2 = 'executed' THEN ?7 ELSE executed_at END,
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?1",
    )
    .bind(id)
    .bind(new_status)
    .bind(reason)
    .bind(substitute_teacher_id)
    .bind(reschedule_to_id)
    .bind(actor.user_id)
    .bind(&now)
    .execute(pool)
    .await?;
    // makeup: neutralize original session (edu008) by marking original_session_id link
    if new_status == "makeup" {
        // makeup inherits weight implicitly via original_session_id link set by caller;
        // nothing else here — original is already cancelled/postponed.
    }
    let after = session_by_id(pool, id).await?;
    if substitute_teacher_id.is_some() {
        sqlx::query(
            "INSERT INTO course_change_log (course_id, changed_by, action, before_json, after_json, reason)
             VALUES (?, ?, 'substitute_teacher', ?, ?, ?)",
        )
        .bind(before.course_id)
        .bind(actor.user_id)
        .bind(snapshot(&before).to_string())
        .bind(snapshot(&after).to_string())
        .bind(reason)
        .execute(pool)
        .await
        .ok();
    }
    log_audit(pool, actor.user_id, "sessions.change", "sessions", Some(id),
        Some(snapshot(&before)), Some(snapshot(&after)), reason).await;
    Ok(after)
}

/// Neutralize an original session when a makeup is created (edu008): set
/// original_session_id on the makeup session. Handled by caller passing
/// original_session_id via create_session? We expose a helper to link.
pub async fn link_makeup_original(
    pool: &SqlitePool,
    makeup_session_id: i64,
    original_session_id: i64,
) -> AppResult<()> {
    sqlx::query("UPDATE sessions SET original_session_id = ?2 WHERE id = ?1")
        .bind(makeup_session_id)
        .bind(original_session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_sessions(
    pool: &SqlitePool,
    actor: &Actor,
    course_id: Option<i64>,
    from: Option<&str>,
    to: Option<&str>,
) -> AppResult<Vec<Session>> {
    actor.require("courses.view")?;
    let sql = format!(
        "SELECT {SESSION_COLS} FROM sessions
         WHERE (?1 IS NULL OR course_id = ?1)
           AND (?2 IS NULL OR session_date >= ?2)
           AND (?3 IS NULL OR session_date <= ?3)
         ORDER BY session_date, start_time"
    );
    let q = sqlx::query(&sql)
    .bind(course_id)
    .bind(from)
    .bind(to);
    let rows = q.fetch_all(pool).await?;
    Ok(rows.iter().map(row_session).collect())
}

/// Change room/teacher/substitute for a session and write course_change_log (FR-6.5).
pub async fn change_session_room_teacher(
    pool: &SqlitePool,
    actor: &Actor,
    session_id: i64,
    room_id: i64,
    teacher_id: i64,
    substitute_teacher_id: Option<i64>,
    reason: &str,
) -> AppResult<Session> {
    actor.require("schedule.manage")?;
    if reason.trim().is_empty() {
        return Err(AppError::validation("السبب مطلوب"));
    }
    let before = session_by_id(pool, session_id).await?;
    clash_check(pool, room_id, teacher_id, substitute_teacher_id,
        &before.session_date, &before.start_time, &before.end_time, Some(session_id)).await?;
    sqlx::query(
        "UPDATE sessions SET room_id=?, teacher_id=?, substitute_teacher_id=?, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?",
    )
    .bind(room_id)
    .bind(teacher_id)
    .bind(substitute_teacher_id)
    .bind(session_id)
    .execute(pool)
    .await?;
    let after = session_by_id(pool, session_id).await?;
    sqlx::query(
        "INSERT INTO course_change_log (course_id, changed_by, action, before_json, after_json, reason)
         VALUES (?, ?, 'session_room_teacher_change', ?, ?, ?)",
    )
    .bind(before.course_id)
    .bind(actor.user_id)
    .bind(snapshot(&before).to_string())
    .bind(snapshot(&after).to_string())
    .bind(reason)
    .execute(pool)
    .await
    .ok();
    log_audit(pool, actor.user_id, "sessions.change", "sessions", Some(session_id),
        Some(snapshot(&before)), Some(snapshot(&after)), reason).await;
    Ok(after)
}

pub async fn update_session_room_teacher(
    pool: &SqlitePool,
    actor: &Actor,
    session_id: i64,
    room_id: i64,
    teacher_id: i64,
    substitute_teacher_id: Option<i64>,
    reason: &str,
) -> AppResult<Session> {
    change_session_room_teacher(pool, actor, session_id, room_id, teacher_id, substitute_teacher_id, reason).await
}
