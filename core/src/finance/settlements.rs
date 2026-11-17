//! Settlements (Phase 6).

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::AppResult;
use crate::people::context::{log_audit, Actor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementKind {
    Course,
    Teacher,
    Center,
    Shortfall,
    Refunds,
    Partial,
    Final,
}

impl SettlementKind {
    pub fn parse(s: &str) -> AppResult<Self> {
        Ok(match s {
            "course" => Self::Course,
            "teacher" => Self::Teacher,
            "center" => Self::Center,
            "shortfall" => Self::Shortfall,
            "refunds" => Self::Refunds,
            "partial" => Self::Partial,
            "final" => Self::Final,
            _ => return Err(AppError::validation("نوع تسوية غير معروف")),
        })
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Course => "course",
            Self::Teacher => "teacher",
            Self::Center => "center",
            Self::Shortfall => "shortfall",
            Self::Refunds => "refunds",
            Self::Partial => "partial",
            Self::Final => "final",
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct Settlement {
    pub id: i64,
    pub branch_id: i64,
    pub kind: String,
    pub scope_id: Option<i64>,
    pub period_from: String,
    pub period_to: String,
    pub status: String,
    pub engine_snapshot_json: String,
    pub reversed_by_id: Option<i64>,
}

pub async fn prepare_settlement(
    pool: &SqlitePool,
    actor: &Actor,
    kind: SettlementKind,
    _scope_id: Option<i64>,
    period_from: &str,
    period_to: &str,
    snapshot_json: &str,
) -> AppResult<Settlement> {
    actor.require("settlements.prepare")?;
    let res = sqlx::query(
        "INSERT INTO settlements (course_id, teacher_id, kind, period_from, period_to,
         status, engine_snapshot_json, created_by)
         VALUES (?, ?, ?, ?, ?, 'pending_approval', ?, ?)",
    )
    .bind(None::<i64>)
    .bind(None::<i64>)
    .bind(kind.as_str())
    .bind(period_from)
    .bind(period_to)
    .bind(snapshot_json)
    .bind(actor.user_id)
    .execute(pool)
    .await?;
    let s = settlement_by_id(pool, res.last_insert_rowid()).await?;
    log_audit(pool, actor.user_id, "settlements.prepare", "settlements", Some(s.id),
        None, None, "").await;
    Ok(s)
}

pub async fn approve_settlement(
    pool: &SqlitePool,
    actor: &Actor,
    settlement_id: i64,
    maker_user_id: i64,
    override_reason: Option<&str>,
) -> AppResult<Settlement> {
    actor.require("settlements.approve")?;
    let s = settlement_by_id(pool, settlement_id).await?;
    if s.status != "pending_approval" {
        return Err(AppError::Conflict("التسوية ليست بانتظار اعتماد".into()));
    }
    // maker-checker
    // creator lookup
    let creator: i64 = sqlx::query_scalar("SELECT created_by FROM settlements WHERE id = ?")
        .bind(settlement_id).fetch_one(pool).await?;
    if creator == actor.user_id && override_reason.is_none() && creator == maker_user_id {
        return Err(AppError::permission(
            "الاعتماد الذاتي غير مسموح — أدخل سبب تجاوز",
        ));
    }
    sqlx::query("UPDATE settlements SET status='approved', approved_by=?, approved_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?")
        .bind(actor.user_id)
        .bind(settlement_id)
        .execute(pool)
        .await?;
    let after = settlement_by_id(pool, settlement_id).await?;
    log_audit(pool, actor.user_id, "settlements.approve", "settlements", Some(settlement_id),
        Some(serde_json::json!({"status": s.status})), Some(serde_json::json!({"status": "approved"})), override_reason.unwrap_or("")).await;
    Ok(after)
}

pub async fn reverse_settlement(
    pool: &SqlitePool,
    actor: &Actor,
    settlement_id: i64,
    reason: &str,
) -> AppResult<Settlement> {
    actor.require("settlements.approve")?;
    if reason.trim().is_empty() {
        return Err(AppError::validation("السبب مطلوب"));
    }
    let s = settlement_by_id(pool, settlement_id).await?;
    if s.status != "approved" {
        return Err(AppError::Conflict("التسوية ليست معتمدة".into()));
    }
    // create a reversal settlement linked
    let res = sqlx::query(
        "INSERT INTO settlements (kind, period_from, period_to,
         status, engine_snapshot_json, created_by, reversed_by_id)
         VALUES (?, ?, ?, 'approved', ?, ?, ?)",
    )
    .bind(&s.kind)
    .bind(&s.period_from)
    .bind(&s.period_to)
    .bind(&s.engine_snapshot_json)
    .bind(actor.user_id)
    .bind(settlement_id)
    .execute(pool)
    .await?;
    let new_id = res.last_insert_rowid();
    // mark original reversed_by_id
    sqlx::query("UPDATE settlements SET reversed_by_id = ? WHERE id = ?")
        .bind(new_id)
        .bind(settlement_id)
        .execute(pool)
        .await?;
    settlement_by_id(pool, new_id).await
}

async fn settlement_by_id(pool: &SqlitePool, id: i64) -> AppResult<Settlement> {
    let r = sqlx::query(
        "SELECT id, kind, period_from, period_to, status,
                engine_snapshot_json, reversed_by_id FROM settlements WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(Settlement {
        id: r.get("id"),
        branch_id: 0,
        kind: r.get("kind"),
        scope_id: None,
        period_from: r.get("period_from"),
        period_to: r.get("period_to"),
        status: r.get("status"),
        engine_snapshot_json: r.get("engine_snapshot_json"),
        reversed_by_id: r.get("reversed_by_id"),
    })
}
