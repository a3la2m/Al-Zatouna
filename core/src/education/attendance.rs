//! Attendance (Phase 5).

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::AppResult;
use crate::people::context::{log_audit, snapshot, Actor};

#[derive(Debug, Serialize, Clone)]
pub struct StudentAttendance {
    pub id: i64,
    pub session_id: i64,
    pub student_id: i64,
    pub status: String,
    pub late_minutes: i64,
    pub notes: String,
}

pub async fn record_student_attendance(
    pool: &SqlitePool,
    actor: &Actor,
    session_id: i64,
    student_id: i64,
    status: &str,
    late_minutes: i64,
    notes: &str,
) -> AppResult<StudentAttendance> {
    if !matches!(status, "present" | "absent" | "late" | "excused") {
        return Err(AppError::validation("حالة حضور غير معروفة"));
    }
    if late_minutes < 0 {
        return Err(AppError::validation("دقائق التأخير لا يمكن أن تكون سالبة"));
    }
    actor.require("attendance.record")?;
    // enrolled in the course of this session?
    let enrolled: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM enrollments e
         JOIN sessions s ON s.course_id = e.course_id
         WHERE s.id = ? AND e.student_id = ? AND e.status IN ('active','frozen')",
    )
    .bind(session_id)
    .bind(student_id)
    .fetch_one(pool)
    .await?;
    if enrolled == 0 {
        return Err(AppError::Conflict(
            "الطالب غير مسجّل في دورة هذه الحصة".into(),
        ));
    }
    let res = sqlx::query(
        "INSERT INTO student_attendance (session_id, student_id, status, late_minutes, notes, recorded_by)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT (session_id, student_id) DO UPDATE SET
            status = excluded.status,
            late_minutes = excluded.late_minutes,
            notes = excluded.notes,
            recorded_by = excluded.recorded_by,
            recorded_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(session_id)
    .bind(student_id)
    .bind(status)
    .bind(late_minutes)
    .bind(notes)
    .bind(actor.user_id)
    .execute(pool)
    .await?;
    let r = sqlx::query(
        "SELECT id, session_id, student_id, status, late_minutes, notes
         FROM student_attendance WHERE session_id = ? AND student_id = ?",
    )
    .bind(session_id)
    .bind(student_id)
    .fetch_one(pool)
    .await?;
    let sa = StudentAttendance {
        id: r.get("id"),
        session_id: r.get("session_id"),
        student_id: r.get("student_id"),
        status: r.get("status"),
        late_minutes: r.get("late_minutes"),
        notes: r.get("notes"),
    };
    let _ = res;
    log_audit(pool, actor.user_id, "attendance.record", "student_attendance", Some(sa.id),
        None, Some(snapshot(&sa)), "").await;
    Ok(sa)
}

#[derive(Debug, Serialize, Clone)]
pub struct TeacherAttendance {
    pub id: i64,
    pub session_id: i64,
    pub teacher_id: i64,
    pub status: String,
    pub notes: String,
}

pub async fn record_teacher_attendance(
    pool: &SqlitePool,
    actor: &Actor,
    session_id: i64,
    teacher_id: i64,
    status: &str,
    notes: &str,
) -> AppResult<TeacherAttendance> {
    if !matches!(status, "present" | "absent" | "late" | "excused") {
        return Err(AppError::validation("حالة حضور غير معروفة"));
    }
    actor.require("attendance.record")?;
    let res = sqlx::query(
        "INSERT INTO teacher_attendance (session_id, teacher_id, status, notes, recorded_by)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT (session_id) DO UPDATE SET
            teacher_id = excluded.teacher_id,
            status = excluded.status,
            notes = excluded.notes,
            recorded_by = excluded.recorded_by,
            recorded_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(session_id)
    .bind(teacher_id)
    .bind(status)
    .bind(notes)
    .bind(actor.user_id)
    .execute(pool)
    .await?;
    let r = sqlx::query(
        "SELECT id, session_id, teacher_id, status, notes
         FROM teacher_attendance WHERE session_id = ?",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await?;
    let ta = TeacherAttendance {
        id: r.get("id"),
        session_id: r.get("session_id"),
        teacher_id: r.get("teacher_id"),
        status: r.get("status"),
        notes: r.get("notes"),
    };
    let _ = res;
    log_audit(pool, actor.user_id, "attendance.record", "teacher_attendance", Some(ta.id),
        None, Some(snapshot(&ta)), "").await;
    Ok(ta)
}

/// Whether cancelled sessions count toward teacher entitlement per agreement conditions.
pub fn cancelled_sessions_count(conditions_json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(conditions_json)
        .ok()
        .and_then(|v| v.get("cancelled_effects").and_then(|x| x.get("count_for_teacher")).and_then(|b| b.as_bool()))
        .unwrap_or(true)
}

/// Makeup session inherits weight of original (returns original_session_id).
pub fn makeup_weight_inherited(original_session_id: Option<i64>) -> bool {
    original_session_id.is_some()
}

// ---------- attendance sheet (Phase 16: the attendance screen) ----------

/// One row of a session's attendance sheet: an enrolled student plus whatever
/// has been recorded for them so far (`status = null` means "not recorded yet").
#[derive(Debug, Serialize, Clone)]
pub struct AttendanceSheetRow {
    pub student_id: i64,
    pub student_no: i64,
    pub full_name: String,
    pub attendance_id: Option<i64>,
    pub status: Option<String>,
    pub late_minutes: Option<i64>,
    pub notes: String,
    pub recorded_at: Option<String>,
}

/// The full sheet for one session, ordered by student number.
///
/// Students come from the enrollments of the session's course (active/frozen),
/// left-joined with whatever has already been recorded — so the screen shows
/// exactly who is still missing a mark, without the caller doing any joins.
pub async fn attendance_sheet(
    pool: &SqlitePool,
    actor: &Actor,
    session_id: i64,
) -> AppResult<Vec<AttendanceSheetRow>> {
    actor.require("attendance.record")?;
    let rows = sqlx::query(
        "SELECT e.student_id AS student_id,
                st.student_no AS student_no,
                st.full_name AS full_name,
                a.id AS attendance_id,
                a.status AS status,
                a.late_minutes AS late_minutes,
                COALESCE(a.notes, '') AS notes,
                a.recorded_at AS recorded_at
         FROM sessions s
         JOIN enrollments e
           ON e.course_id = s.course_id AND e.status IN ('active','frozen')
         JOIN students st ON st.id = e.student_id
         LEFT JOIN student_attendance a
           ON a.session_id = s.id AND a.student_id = e.student_id
         WHERE s.id = ?
         ORDER BY st.student_no, st.id",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| AttendanceSheetRow {
            student_id: r.get("student_id"),
            student_no: r.get("student_no"),
            full_name: r.get("full_name"),
            attendance_id: r.get::<Option<i64>, _>("attendance_id"),
            status: r.get::<Option<String>, _>("status"),
            late_minutes: r.get::<Option<i64>, _>("late_minutes"),
            notes: r.get("notes"),
            recorded_at: r.get::<Option<String>, _>("recorded_at"),
        })
        .collect())
}
