//! Permission evaluation (docs/SECURITY.md §4).
//!
//! Effective set = (roles ∪ allowed overrides) − denied overrides.
//! Reads come entirely from seeded tables; nothing is hard-coded.

use std::collections::HashSet;

use sqlx::SqlitePool;

use crate::AppResult;

/// One user's resolved permission profile.
#[derive(Debug, Clone, PartialEq)]
pub struct PermissionSet {
    pub user_id: i64,
    pub roles: Vec<String>,
    pub permissions: HashSet<String>,
}

impl PermissionSet {
    pub fn can(&self, code: &str) -> bool {
        self.permissions.contains(code)
    }

    pub fn can_any(&self, codes: &[&str]) -> bool {
        codes.iter().any(|c| self.permissions.contains(*c))
    }
}

/// Load the effective permission set for a user.
pub async fn load_for_user(pool: &SqlitePool, user_id: i64) -> AppResult<PermissionSet> {
    // active system roles of the user
    let roles: Vec<(String,)> = sqlx::query_as(
        "SELECT r.code FROM roles r
         JOIN user_roles ur ON ur.role_id = r.id
         WHERE ur.user_id = ? AND r.is_system = 1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    // permissions granted via roles
    let role_perms: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT p.code FROM permissions p
         JOIN role_permissions rp ON rp.permission_id = p.id
         JOIN user_roles ur ON ur.role_id = rp.role_id
         WHERE ur.user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    // overrides: allowed=1 adds, allowed=0 removes
    let overrides: Vec<(String, i64)> = sqlx::query_as(
        "SELECT p.code, o.allowed FROM permissions p
         JOIN user_permission_overrides o ON o.permission_id = p.id
         WHERE o.user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut permissions: HashSet<String> = role_perms.into_iter().map(|(c,)| c).collect();
    for (code, allowed) in overrides {
        if allowed == 1 {
            permissions.insert(code);
        } else {
            permissions.remove(&code);
        }
    }

    Ok(PermissionSet {
        user_id,
        roles: roles.into_iter().map(|(c,)| c).collect(),
        permissions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_utils::fresh_seeded_pool;

    #[tokio::test]
    async fn admin_has_all_and_empty_user_has_none() {
        let (pool, _t, _branch) = fresh_seeded_pool().await;
        // create a bare user + one assigned system_admin user
        sqlx::query("INSERT INTO users (id, username, full_name, password_hash) VALUES (910, 'bare', 'Bare', 'x')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO users (id, username, full_name, password_hash) VALUES (911, 'root', 'Root', 'x')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (911, (SELECT id FROM roles WHERE code='system_admin'))")
            .execute(&pool).await.unwrap();

        let bare = load_for_user(&pool, 910).await.unwrap();
        let root = load_for_user(&pool, 911).await.unwrap();

        assert!(!bare.can("users.manage"));
        assert!(root.can("users.manage"));
        assert!(root.can("backup.create"));
        assert!(root.can("ledger.view"), "admin must hold every seeded permission");
        assert_eq!(root.roles, vec!["system_admin".to_string()]);
    }

    #[tokio::test]
    async fn overrides_add_and_remove() {
        let (pool, _t, _branch) = fresh_seeded_pool().await;
        sqlx::query("INSERT INTO users (id, username, full_name, password_hash) VALUES (920, 'u20', 'U20', 'x')")
            .execute(&pool).await.unwrap();
        // accountant role
        sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (920, (SELECT id FROM roles WHERE code='accountant'))")
            .execute(&pool).await.unwrap();
        // deny override on a permission the accountant has
        sqlx::query(
            "INSERT INTO user_permission_overrides (user_id, permission_id, allowed, reason, granted_by)
             VALUES (920, (SELECT id FROM permissions WHERE code='ledger.view'), 0, 'test', 920)"
        ).execute(&pool).await.unwrap();

        let ps = load_for_user(&pool, 920).await.unwrap();
        assert!(!ps.can("ledger.view"), "denied override must win");
    }
}
