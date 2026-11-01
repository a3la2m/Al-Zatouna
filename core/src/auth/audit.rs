//! Append-only audit trail service (docs/SECURITY.md §6).
//!
//! Every write goes through `log()`. Triggers in migration 0001 make the
//! table immutable, so this is the only legitimate insertion point.

use serde_json::Value;
use sqlx::SqlitePool;

use crate::AppResult;

/// An audit event to record.
#[derive(Debug, Clone)]
pub struct AuditEvent<'a> {
    /// acting user id (None = system)
    pub user_id: Option<i64>,
    /// machine-readable action, e.g. "students.create"
    pub action: &'a str,
    /// entity type code, e.g. "students"
    pub entity_type: &'a str,
    pub entity_id: Option<i64>,
    /// snapshot before the change (serializable value, optional)
    pub old: Option<&'a Value>,
    /// snapshot after the change (serializable value, optional)
    pub new: Option<&'a Value>,
    /// mandatory for cancellations/approvals/overrides
    pub reason: &'a str,
    /// extra context (session/window id, ip, …)
    pub ctx: Option<&'a Value>,
}

fn json_or_null(v: Option<&Value>) -> Option<String> {
    v.map(|x| x.to_string())
}

/// Insert an audit row. Non-fatal by design at call sites that must not
/// abort business operations on logging failure — but log via `tracing`.
pub async fn log(pool: &SqlitePool, ev: AuditEvent<'_>) -> AppResult<()> {
    let res = sqlx::query(
        "INSERT INTO audit_log (user_id, action, entity_type, entity_id,
         old_json, new_json, reason, ctx_json)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(ev.user_id)
    .bind(ev.action)
    .bind(ev.entity_type)
    .bind(ev.entity_id)
    .bind(json_or_null(ev.old))
    .bind(json_or_null(ev.new))
    .bind(ev.reason)
    .bind(json_or_null(ev.ctx))
    .execute(pool)
    .await;

    match res {
        Ok(_) => Ok(()),
        Err(e) => {
            tracing::error!(action = ev.action, "audit insert failed: {e}");
            Err(e.into())
        }
    }
}

/// Convenience: count events by action (tests + coverage checks).
pub async fn count_action(pool: &SqlitePool, action: &str) -> AppResult<i64> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE action = ?")
        .bind(action)
        .fetch_one(pool)
        .await?;
    Ok(n)
}
