//! Financial agreements (Phase 5): versioned, immutable once active.

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::AppResult;
use crate::people::context::{log_audit, snapshot, Actor};

const METHODS: &[&str] = &[
    "percent_of_agreed",
    "percent_of_collected",
    "percent_of_executed_sessions",
    "flat_course",
    "flat_session",
    "flat_plus_percent",
];

#[derive(Debug, Serialize, Clone)]
pub struct Agreement {
    pub id: i64,
    pub course_id: i64,
    pub version: i64,
    pub status: String,
    pub method: String,
    pub rate_bp: Option<i64>,
    pub center_rate_bp: Option<i64>,
    pub flat_minor: Option<i64>,
    pub per_session_minor: Option<i64>,
    pub due_timing_json: String,
    pub conditions_json: String,
    pub snapshot_json: String,
    pub approved_by: Option<i64>,
    pub approved_at: Option<String>,
}

fn row_agreement(r: &sqlx::sqlite::SqliteRow) -> Agreement {
    Agreement {
        id: r.get("id"),
        course_id: r.get("course_id"),
        version: r.get("version"),
        status: r.get("status"),
        method: r.get("method"),
        rate_bp: r.get("rate_bp"),
        center_rate_bp: r.get("center_rate_bp"),
        flat_minor: r.get("flat_minor"),
        per_session_minor: r.get("per_session_minor"),
        due_timing_json: r.get("due_timing_json"),
        conditions_json: r.get("conditions_json"),
        snapshot_json: r.get("snapshot_json"),
        approved_by: r.get("approved_by"),
        approved_at: r.get("approved_at"),
    }
}

const AGR_COLS: &str =
    "id, course_id, version, status, method, rate_bp, center_rate_bp, flat_minor,
     per_session_minor, due_timing_json, conditions_json, snapshot_json,
     approved_by, approved_at";

pub async fn create_agreement_draft(
    pool: &SqlitePool,
    actor: &Actor,
    course_id: i64,
    method: &str,
    rate_bp: Option<i64>,
    center_rate_bp: Option<i64>,
    flat_minor: Option<i64>,
    per_session_minor: Option<i64>,
    due_timing_json: &str,
    conditions_json: &str,
) -> AppResult<Agreement> {
    actor.require("agreements.manage")?;
    if !METHODS.contains(&method) {
        return Err(AppError::validation("طريقة غير معروفة"));
    }
    // validate rate ranges
    if let Some(r) = rate_bp {
        if !(0..=10000).contains(&r) {
            return Err(AppError::validation("نسبة غير صالحة"));
        }
    }
    // next version for this course
    let next_ver: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM financial_agreements WHERE course_id = ?",
    )
    .bind(course_id)
    .fetch_one(pool)
    .await?;
    let snap_json = serde_json::json!({
        "method": method,
        "rate_bp": rate_bp,
        "center_rate_bp": center_rate_bp,
        "flat_minor": flat_minor,
        "per_session_minor": per_session_minor,
        "due_timing_json": due_timing_json,
        "conditions_json": conditions_json,
        "created_at": chrono::Utc::now().to_rfc3339(),
    })
    .to_string();
    let res = sqlx::query(
        "INSERT INTO financial_agreements (course_id, version, status, method,
         rate_bp, center_rate_bp, flat_minor, per_session_minor, due_timing_json,
         conditions_json, snapshot_json)
         VALUES (?, ?, 'draft', ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(course_id)
    .bind(next_ver)
    .bind(method)
    .bind(rate_bp)
    .bind(center_rate_bp)
    .bind(flat_minor)
    .bind(per_session_minor)
    .bind(due_timing_json)
    .bind(conditions_json)
    .bind(&snap_json)
    .execute(pool)
    .await?;
    let r = sqlx::query(&format!("SELECT {AGR_COLS} FROM financial_agreements WHERE id = ?"))
        .bind(res.last_insert_rowid())
        .fetch_one(pool)
        .await?;
    let a = row_agreement(&r);
    log_audit(pool, actor.user_id, "agreements.create", "financial_agreements", Some(a.id),
        None, Some(snapshot(&a)), "").await;
    Ok(a)
}

/// Approve a draft agreement → active; supersedes the previous active one.
/// Enforces maker-checker (creator != approver unless override_reason).
pub async fn approve_agreement(
    pool: &SqlitePool,
    actor: &Actor,
    agreement_id: i64,
    creator_user_id: i64,
    override_reason: Option<&str>,
) -> AppResult<Agreement> {
    actor.require("agreements.manage")?;
    let a = agreement_by_id(pool, agreement_id).await?;
    if a.status != "draft" {
        return Err(AppError::Conflict("الاتفاقية ليست مسودة".into()));
    }
    // maker-checker
    if creator_user_id == actor.user_id && override_reason.is_none() {
        return Err(AppError::permission(
            "الاعتماد الذاتي غير مسموح — أدخل سبب تجاوز",
        ));
    }
    let mut tx = pool.begin().await?;
    // supersede previous active
    sqlx::query(
        "UPDATE financial_agreements SET status='superseded',
         updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE course_id = ? AND status='active' AND id <> ?",
    )
    .bind(a.course_id)
    .bind(agreement_id)
    .execute(&mut *tx)
    .await?;
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    sqlx::query(
        "UPDATE financial_agreements SET status='active', approved_by=?2, approved_at=?3,
         updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?1",
    )
    .bind(agreement_id)
    .bind(actor.user_id)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let after = agreement_by_id(pool, agreement_id).await?;
    log_audit(pool, actor.user_id, "agreements.approve", "financial_agreements", Some(agreement_id),
        Some(snapshot(&a)), Some(snapshot(&after)), override_reason.unwrap_or("")).await;
    Ok(after)
}

async fn agreement_by_id(pool: &SqlitePool, id: i64) -> AppResult<Agreement> {
    let r = sqlx::query(&format!("SELECT {AGR_COLS} FROM financial_agreements WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("الاتفاقية غير موجودة"))?;
    Ok(row_agreement(&r))
}

pub async fn active_agreement_for_course(
    pool: &SqlitePool,
    course_id: i64,
) -> AppResult<Option<Agreement>> {
    let r = sqlx::query(&format!(
        "SELECT {AGR_COLS} FROM financial_agreements WHERE course_id = ? AND status='active'"
    ))
    .bind(course_id)
    .fetch_optional(pool)
    .await?;
    Ok(r.map(|row| row_agreement(&row)))
}

pub async fn amend_agreement(
    pool: &SqlitePool,
    actor: &Actor,
    agreement_id: i64,
    reason: &str,
    new_method: Option<&str>,
    new_rate_bp: Option<i64>,
) -> AppResult<Agreement> {
    actor.require("agreements.manage")?;
    if reason.trim().is_empty() {
        return Err(AppError::validation("سبب التعديل مطلوب"));
    }
    let old = agreement_by_id(pool, agreement_id).await?;
    if old.status != "active" {
        return Err(AppError::Conflict("التعديل ينطبق على اتفاقية نشطة".into()));
    }
    // create new draft version inheriting old terms overridden by supplied
    let method = new_method.unwrap_or(&old.method);
    let rate = new_rate_bp.or(old.rate_bp);
    let new_draft = create_agreement_draft(
        pool,
        actor,
        old.course_id,
        method,
        rate,
        old.center_rate_bp,
        old.flat_minor,
        old.per_session_minor,
        &old.due_timing_json,
        &old.conditions_json,
    )
    .await?;
    // record amendment row
    sqlx::query(
        "INSERT INTO agreement_amendments (agreement_id, old_version, new_version,
         changes_json, reason, created_by, approved_by)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(agreement_id)
    .bind(old.version)
    .bind(new_draft.version)
    .bind(serde_json::json!({"method": method, "rate_bp": rate}).to_string())
    .bind(reason)
    .bind(actor.user_id)
    .bind(actor.user_id)
    .execute(pool)
    .await?;
    Ok(new_draft)
}
