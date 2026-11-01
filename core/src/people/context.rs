//! Shared command-context plumbing for domain services (Phase 4+).
//!
//! Every domain service entry point takes an `Actor` — resolved in the
//! shell from the session token — and enforcement happens in Rust here
//! (ADR-008: the frontend only hides UI elements; it is not security).

use crate::auth::permissions::PermissionSet;
use crate::error::AppError;
use crate::AppResult;

/// The calling user as resolved from the session store.
#[derive(Debug, Clone)]
pub struct Actor {
    pub user_id: i64,
    pub username: String,
    pub permissions: PermissionSet,
}

impl Actor {
    /// Gate: `permission` must hold or the call is refused. Admin actions
    /// like refusing are audited by the domain service, not here.
    pub fn require(&self, permission: &str) -> AppResult<()> {
        if self.permissions.can(permission) {
            Ok(())
        } else {
            Err(AppError::permission(format!(
                "ليس لديك صلاحية: {permission}"
            )))
        }
    }
}

/// Helper for capturing before/after snapshots for the audit trail.
pub fn snapshot<T: serde::Serialize>(v: &T) -> serde_json::Value {
    serde_json::to_value(v).unwrap_or(serde_json::Value::Null)
}

/// Shared audit convenience used by domain services.
pub async fn log_audit(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    action: &'static str,
    entity_type: &str,
    entity_id: Option<i64>,
    old: Option<serde_json::Value>,
    new: Option<serde_json::Value>,
    reason: &str,
) {
    let uid = if user_id == 0 { None } else { Some(user_id) };
    let ev = crate::auth::audit::AuditEvent {
        user_id: uid,
        action,
        entity_type,
        entity_id,
        old: old.as_ref(),
        new: new.as_ref(),
        reason,
        ctx: None,
    };
    let _ = crate::auth::audit::log(pool, ev).await;
}
