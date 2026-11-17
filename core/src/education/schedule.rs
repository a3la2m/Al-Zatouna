//! Weekly schedule rules + session generation (Phase 18).
//! A course owns recurring weekly rules (weekday, time, room, teacher, effective
//! range). Rules are checked for room/teacher clashes before being saved, and
//! can be expanded into concrete sessions for a date range.

use chrono::Datelike;
use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::people::context::Actor;
use crate::AppResult;

#[derive(Debug, Serialize, Clone)]
pub struct ScheduleRule {
    pub id: i64,
    pub course_id: i64,
    pub course_name: String,
    pub weekday: i64,
    pub start_time: String,
    pub end_time: String,
    pub room_id: i64,
    pub room_name: String,
    pub teacher_id: i64,
    pub teacher_name: String,
    pub effective_from: String,
    pub effective_to: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct Clash {
    pub kind: String, // "room" | "teacher"
    pub message: String,
    pub with_course: String,
    pub weekday: i64,
    pub start_time: String,
    pub end_time: String,
    pub room_name: String,
    pub teacher_name: String,
}

/// SQLite weekday (0=Sun..6=Sat) → ours (1=Mon..7=Sun)
const WEEKDAY_SQL: &str =
    "CASE WHEN CAST(strftime('%w', session_date) AS INTEGER)=0 THEN 7 ELSE CAST(strftime('%w', session_date) AS INTEGER) END";

fn overlap(a_start: &str, a_end: &str, b_start: &str, b_end: &str) -> bool {
    a_start < b_end && a_end > b_start
}

/// Check a proposed weekly rule against existing rules and concrete sessions.
/// Returns structured clashes (empty vector = free).
pub async fn check_rule_clashes(
    pool: &SqlitePool,
    course_id: i64,
    weekday: i64,
    start: &str,
    end: &str,
    room_id: i64,
    teacher_id: i64,
) -> AppResult<Vec<Clash>> {
    let mut out = Vec::new();

    // 1) other weekly rules (same weekday, overlapping time)
    let rules = sqlx::query(
        "SELECT cs.course_id, c.name AS course_name, cs.weekday, cs.start_time, cs.end_time,
                COALESCE(r.name,'') AS room_name, COALESCE(t.full_name,'') AS teacher_name
         FROM course_schedules cs
         JOIN courses c ON c.id=cs.course_id
         LEFT JOIN rooms r ON r.id=cs.room_id
         LEFT JOIN teachers t ON t.id=cs.teacher_id
         WHERE cs.weekday=? AND c.status IN ('active','registration_open','draft')",
    )
    .bind(weekday)
    .fetch_all(pool)
    .await?;
    for r in rules {
        let r_start: String = r.get("start_time");
        let r_end: String = r.get("end_time");
        if !overlap(start, end, &r_start, &r_end) { continue; }
        let other_course: i64 = r.get("course_id");
        if other_course == course_id { continue; }
        let same_room = sqlx::query_scalar::<_, i64>(
            "SELECT room_id FROM course_schedules WHERE course_id=? AND weekday=? AND start_time=?",
        )
        .bind(other_course).bind(weekday).bind(&r_start)
        .fetch_optional(pool).await?.unwrap_or(0) == room_id;
        let other_teacher: i64 = sqlx::query_scalar(
            "SELECT teacher_id FROM course_schedules WHERE course_id=? AND weekday=? AND start_time=?",
        )
        .bind(other_course).bind(weekday).bind(&r_start)
        .fetch_optional(pool).await?.unwrap_or(0);
        if same_room {
            out.push(Clash {
                kind: "room".into(),
                message: format!("القاعة محجوزة في هذا الوقت — اختر قاعة أخرى أو وقتاً آخر"),
                with_course: r.get("course_name"),
                weekday, start_time: r_start.clone(), end_time: r_end.clone(),
                room_name: r.get("room_name"), teacher_name: r.get("teacher_name"),
            });
        }
        if other_teacher == teacher_id {
            out.push(Clash {
                kind: "teacher".into(),
                message: format!("المدرس مشغول في دورة أخرى في هذا الوقت — اختر مدرساً أو وقتاً آخر"),
                with_course: r.get("course_name"),
                weekday, start_time: r_start, end_time: r_end,
                room_name: r.get("room_name"), teacher_name: r.get("teacher_name"),
            });
        }
    }

    // 2) concrete sessions on that weekday
    let sess = sqlx::query(&format!(
        "SELECT s.course_id, c.name AS course_name, s.start_time, s.end_time,
                COALESCE(r.name,'') AS room_name, COALESCE(t.full_name,'') AS teacher_name,
                s.room_id, s.teacher_id
         FROM sessions s JOIN courses c ON c.id=s.course_id
         LEFT JOIN rooms r ON r.id=s.room_id
         LEFT JOIN teachers t ON t.id=s.teacher_id
         WHERE {} = ? AND s.status NOT IN ('cancelled')",
        WEEKDAY_SQL
    ))
    .bind(weekday)
    .fetch_all(pool)
    .await?;
    for r in sess {
        let s_start: String = r.get("start_time");
        let s_end: String = r.get("end_time");
        if !overlap(start, end, &s_start, &s_end) { continue; }
        let other_course: i64 = r.get("course_id");
        if other_course == course_id { continue; }
        if r.get::<i64, _>("room_id") == room_id {
            out.push(Clash {
                kind: "room".into(),
                message: "القاعة محجوزة بحصة فعلية في هذا الوقت".into(),
                with_course: r.get("course_name"), weekday,
                start_time: s_start.clone(), end_time: s_end.clone(),
                room_name: r.get("room_name"), teacher_name: r.get("teacher_name"),
            });
        }
        if r.get::<i64, _>("teacher_id") == teacher_id {
            out.push(Clash {
                kind: "teacher".into(),
                message: "المدرس مشغول بحصة فعلية في هذا الوقت".into(),
                with_course: r.get("course_name"), weekday,
                start_time: s_start, end_time: s_end,
                room_name: r.get("room_name"), teacher_name: r.get("teacher_name"),
            });
        }
    }
    Ok(out)
}

/// Add a weekly rule, refusing it when it clashes.
pub async fn add_rule(
    pool: &SqlitePool,
    actor: &Actor,
    course_id: i64,
    weekday: i64,
    start: &str,
    end: &str,
    room_id: i64,
    teacher_id: i64,
    effective_from: &str,
    effective_to: &str,
) -> AppResult<ScheduleRule> {
    actor.require("schedule.manage")?;
    if !(1..=7).contains(&weekday) { return Err(AppError::validation("يوم الأسبوع غير صالح")); }
    if start >= end { return Err(AppError::validation("وقت البداية يجب أن precede النهاية")); }
    let clashes = check_rule_clashes(pool, course_id, weekday, start, end, room_id, teacher_id).await?;
    if !clashes.is_empty() {
        return Err(AppError::Conflict(clashes[0].message.clone()));
    }
    let res = sqlx::query(
        "INSERT INTO course_schedules (course_id, weekday, start_time, end_time, room_id, teacher_id, effective_from, effective_to)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(course_id).bind(weekday).bind(start).bind(end)
    .bind(room_id).bind(teacher_id).bind(effective_from).bind(effective_to)
    .execute(pool).await?;
    crate::people::context::log_audit(pool, actor.user_id, "schedule.create", "course_schedules",
        Some(res.last_insert_rowid()), None, None, "").await;
    rule_by_id(pool, res.last_insert_rowid()).await
}

async fn rule_by_id(pool: &SqlitePool, id: i64) -> AppResult<ScheduleRule> {
    let r = sqlx::query(
        "SELECT cs.*, c.name AS course_name, COALESCE(rm.name,'') AS room_name,
                COALESCE(t.full_name,'') AS teacher_name
         FROM course_schedules cs
         JOIN courses c ON c.id=cs.course_id
         LEFT JOIN rooms rm ON rm.id=cs.room_id
         LEFT JOIN teachers t ON t.id=cs.teacher_id
         WHERE cs.id=?",
    ).bind(id).fetch_optional(pool).await?
        .ok_or_else(|| AppError::not_found("قاعدة الجدول غير موجودة"))?;
    Ok(row_rule(&r))
}

fn row_rule(r: &sqlx::sqlite::SqliteRow) -> ScheduleRule {
    ScheduleRule {
        id: r.get("id"), course_id: r.get("course_id"), course_name: r.get("course_name"),
        weekday: r.get("weekday"), start_time: r.get("start_time"), end_time: r.get("end_time"),
        room_id: r.get("room_id"), room_name: r.get("room_name"),
        teacher_id: r.get("teacher_id"), teacher_name: r.get("teacher_name"),
        effective_from: r.try_get("effective_from").unwrap_or_default(),
        effective_to: r.try_get("effective_to").unwrap_or_default(),
    }
}

pub async fn list_rules(
    pool: &SqlitePool,
    actor: &Actor,
    course_id: Option<i64>,
) -> AppResult<Vec<ScheduleRule>> {
    actor.require("courses.view")?;
    let sql = format!(
        "SELECT cs.*, c.name AS course_name, COALESCE(rm.name,'') AS room_name,
                COALESCE(t.full_name,'') AS teacher_name
         FROM course_schedules cs
         JOIN courses c ON c.id=cs.course_id
         LEFT JOIN rooms rm ON rm.id=cs.room_id
         LEFT JOIN teachers t ON t.id=cs.teacher_id
         WHERE (?1 IS NULL OR cs.course_id=?1)
         ORDER BY cs.weekday, cs.start_time, c.name"
    );
    let rows = sqlx::query(&sql).bind(course_id).fetch_all(pool).await?;
    Ok(rows.iter().map(row_rule).collect())
}

pub async fn delete_rule(pool: &SqlitePool, actor: &Actor, id: i64) -> AppResult<()> {
    actor.require("schedule.manage")?;
    sqlx::query("DELETE FROM course_schedules WHERE id=?").bind(id).execute(pool).await?;
    Ok(())
}

#[derive(Debug, Serialize, Clone)]
pub struct GeneratedSession {
    pub course_id: i64,
    pub course_name: String,
    pub session_date: String,
    pub start_time: String,
    pub end_time: String,
    pub room_id: i64,
    pub room_name: String,
    pub teacher_id: i64,
    pub teacher_name: String,
    pub source_schedule_id: i64,
    pub seq_in_course: i64,
}

fn date_matches_weekday(date: &str, weekday: i64) -> bool {
    let ok = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|d| {
            let wd = d.weekday().num_days_from_monday() + 1; // 1=Mon..7=Sun
            wd as i64 == weekday
        })
        .unwrap_or(false);
    ok
}

/// Expand all weekly rules of a course into concrete session rows for a range.
/// `commit=false` returns a preview without writing.
pub async fn generate_sessions(
    pool: &SqlitePool,
    actor: &Actor,
    course_id: i64,
    from: &str,
    to: &str,
    commit: bool,
) -> AppResult<Vec<GeneratedSession>> {
    actor.require("schedule.manage")?;
    let rules = list_rules(pool, actor, Some(course_id)).await?;
    let start = chrono::NaiveDate::parse_from_str(from, "%Y-%m-%d")
        .map_err(|_| AppError::validation("تاريخ البداية غير صالح"))?;
    let end = chrono::NaiveDate::parse_from_str(to, "%Y-%m-%d")
        .map_err(|_| AppError::validation("تاريخ النهاية غير صالح"))?;
    if start > end { return Err(AppError::validation("تاريخ البداية بعد النهاية")); }
    let mut out = Vec::new();
    let mut d = start;
    while d <= end {
        let date = d.format("%Y-%m-%d").to_string();
        for rule in &rules {
            if !date_matches_weekday(&date, rule.weekday) { continue; }
            if !rule.effective_from.is_empty() && date < rule.effective_from { continue; }
            if !rule.effective_to.is_empty() && date > rule.effective_to { continue; }
            // skip if a session already exists for this course/date/time
            let exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sessions WHERE course_id=? AND session_date=? AND start_time=?",
            ).bind(course_id).bind(&date).bind(&rule.start_time)
                .fetch_one(pool).await?;
            if exists > 0 { continue; }
            // clash check against other courses' sessions on this exact date
            let clash = check_date_clash(pool, course_id, &date, &rule.start_time, &rule.end_time, rule.room_id, rule.teacher_id).await?;
            if let Some(c) = clash {
                return Err(AppError::Conflict(format!("{} — {}", date, c)));
            }
            let seq: i64 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(seq_in_course),0)+1 FROM sessions WHERE course_id=?",
            ).bind(course_id).fetch_one(pool).await?;
            if commit {
                sqlx::query(
                    "INSERT INTO sessions (course_id, source_schedule_id, seq_in_course, session_date,
                         start_time, end_time, room_id, teacher_id, status, session_kind, created_by)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'scheduled', 'regular', ?)",
                )
                .bind(course_id).bind(rule.id).bind(seq).bind(&date)
                .bind(&rule.start_time).bind(&rule.end_time)
                .bind(rule.room_id).bind(rule.teacher_id).bind(actor.user_id)
                .execute(pool).await?;
            }
            out.push(GeneratedSession {
                course_id, course_name: rule.course_name.clone(), session_date: date.clone(),
                start_time: rule.start_time.clone(), end_time: rule.end_time.clone(),
                room_id: rule.room_id, room_name: rule.room_name.clone(),
                teacher_id: rule.teacher_id, teacher_name: rule.teacher_name.clone(),
                source_schedule_id: rule.id, seq_in_course: seq,
            });
        }
        d = d.succ_opt().unwrap_or(end);
    }
    Ok(out)
}

async fn check_date_clash(
    pool: &SqlitePool, course_id: i64, date: &str, start: &str, end: &str,
    room_id: i64, teacher_id: i64,
) -> AppResult<Option<String>> {
    let rows = sqlx::query(
        "SELECT s.start_time, s.end_time, c.name AS course_name,
                COALESCE(r.name,'') AS room_name, COALESCE(t.full_name,'') AS teacher_name,
                s.room_id, s.teacher_id
         FROM sessions s JOIN courses c ON c.id=s.course_id
         LEFT JOIN rooms r ON r.id=s.room_id LEFT JOIN teachers t ON t.id=s.teacher_id
         WHERE s.session_date=? AND s.status NOT IN ('cancelled') AND s.course_id <> ?",
    ).bind(date).bind(course_id).fetch_all(pool).await?;
    for r in rows {
        let s: String = r.get("start_time"); let e: String = r.get("end_time");
        if !overlap(start, end, &s, &e) { continue; }
        if r.get::<i64, _>("room_id") == room_id {
            return Ok(Some(format!("القاعة {} محجوزة في {} ({}–{}) بدورة {}",
                r.get::<String,_>("room_name"), date, s, e, r.get::<String,_>("course_name"))));
        }
        if r.get::<i64, _>("teacher_id") == teacher_id {
            return Ok(Some(format!("المدرس {} مشغول في {} ({}–{}) بدورة {}",
                r.get::<String,_>("teacher_name"), date, s, e, r.get::<String,_>("course_name"))));
        }
    }
    Ok(None)
}
