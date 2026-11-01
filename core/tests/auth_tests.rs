//! Phase 3 integration tests: auth flows + security tests sec001–sec006
//! (adapted to what exists at this phase). docs/TEST_PLAN.md §8, §10.

use std::time::Duration;

use ec_core::auth::password;
use ec_core::auth::service::{
    AuthService, FAILED_ATTEMPTS_LIMIT, INACTIVITY_TIMEOUT, LOCKOUT_MINUTES,
};
use ec_core::database::test_utils::fresh_seeded_pool;

/// Helper: bootstrap an active admin + service on a fresh seeded pool.
async fn admin_ready() -> (sqlx::SqlitePool, tempfile::TempDir, AuthService, String) {
    let (pool, t, _b) = fresh_seeded_pool().await;
    let svc = AuthService::new(pool.clone());
    let id = svc
        .create_first_admin("admin", "المدير العام", "Sup3r-Secret!")
        .await
        .unwrap();
    assert_eq!(id, 1, "first admin is user #1");
    let res = svc.login("admin", "Sup3r-Secret!").await.unwrap();
    assert_eq!(res.user.user_id, id);
    let token_hex = res.token.clone();
    let must_change = res.must_change_password;
    assert!(must_change, "first login forces password change");
    (pool, t, svc, token_hex)
}

/// auth: first-admin creation is a one-shot.
#[tokio::test]
async fn auth001_first_admin_one_shot() {
    let (pool, _t, svc, _) = admin_ready().await;
    let err = svc
        .create_first_admin("second", "x", "password123")
        .await
        .unwrap_err();
    assert!(matches!(err, ec_core::AppError::Conflict(_)));
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 1);
}

/// auth: happy login issues a working token; password change requires old.
#[tokio::test]
async fn auth002_login_and_change_password() {
    let (_pool, _t, svc, token) = admin_ready().await;
    // session authorizes
    let su = svc.session_user(&token).await.unwrap();
    assert_eq!(su.username, "admin");
    assert!(su.permissions.can("users.manage"), "system_admin");

    // wrong "current" password refused
    let err = svc
        .change_password(&token, "wrong-old-pw", "NewPass-2026")
        .await;
    assert!(err.is_err());

    svc.change_password(&token, "Sup3r-Secret!", "NewPass-2026")
        .await
        .unwrap();
    // login with the NEW password works now
    let res = svc.login("admin", "NewPass-2026").await.unwrap();
    assert_eq!(res.must_change_password, false);
}

/// auth: lockout after N failed attempts, then reset clears it.
#[tokio::test]
async fn auth003_lockout_after_failures() {
    let (_pool, _t, svc, _token) = admin_ready().await;
    assert_eq!(FAILED_ATTEMPTS_LIMIT, 5);
    assert_eq!(LOCKOUT_MINUTES, 5);

    for i in 1..=FAILED_ATTEMPTS_LIMIT {
        let err = svc.login("admin", "totally-wrong").await.unwrap_err();
        if i < FAILED_ATTEMPTS_LIMIT {
            assert!(err.to_string().contains("غير صحيحة"), "attempt {i}: {err}");
        } else {
            assert!(err.to_string().contains("قفل"), "attempt {i}: {err}");
        }
    }

    // still locked with the CORRECT password
    let err = svc.login("admin", "Sup3r-Secret!").await.unwrap_err();
    assert!(err.to_string().contains("مقفول"));

    // lockout window must be recorded
    let (pool, _) = (svc.pool.clone(), ());
    let locked: Option<String> = sqlx::query_scalar("SELECT locked_until FROM users WHERE id=1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(locked.is_some(), "locked_until must be recorded");
}

/// auth: inactivity timeout ends the session (auto-lock, desktop).
#[tokio::test]
async fn auth004_inactivity_auto_lock() {
    let (_pool, _t, svc, token) = admin_ready().await;
    assert_eq!(INACTIVITY_TIMEOUT.as_secs(), 20 * 60, "default 20 min");
    // tiny timeout ⇒ expired (backdate the session 21 minutes)
    svc.sessions.backdate(&token, Duration::from_secs(21 * 60)).await;
    let gone = svc
        .sessions
        .verify_with_timeout(&token, INACTIVITY_TIMEOUT)
        .await;
    assert!(gone.is_none(), "idle session must be dropped");
    let err = svc.session_user(&token).await.unwrap_err();
    assert!(err.to_string().contains("أعد تسجيل الدخول"));
}

/// auth: logout invalidates immediately and is audited.
#[tokio::test]
async fn auth005_logout_invalidates() {
    let (pool, _t, svc, token) = admin_ready().await;
    svc.logout(&token).await.unwrap();
    assert!(svc.session_user(&token).await.is_err());
    let n = ec_core::auth::audit::count_action(&pool, "logout").await.unwrap();
    assert_eq!(n, 1, "logout must be audited");
}

/// sec003: stored password is Argon2id PHC and no plaintext anywhere.
#[tokio::test]
async fn sec003_hashes_not_plaintext() {
    let (pool, t, svc, _token) = admin_ready().await;
    let (hash, uname): (String, String) =
        sqlx::query_as("SELECT password_hash, username FROM users WHERE id=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(hash.starts_with("$argon2id$"), "stored hash: {hash}");
    assert!(!hash.contains("Sup3r-Secret!"));
    assert_ne!(hash, uname);
    // verification still round-trips
    assert!(password::verify_password("Sup3r-Secret!", &hash));
    drop(svc);
    drop(t);
}

/// sec001 (part): role-based permission matrix must match the seeded map.
#[tokio::test]
async fn sec001_role_perm_matrix() {
    let (pool, _t, _b) = fresh_seeded_pool().await;

    // expected subset per role from docs/SECURITY.md §4 + 0002_seed.sql
    let matrix: Vec<(&str, Vec<&str>, Vec<&str>)> = vec![
        (
            "system_admin",
            vec!["users.manage", "backup.restore", "audit.view", "ledger.view"],
            vec![],
        ),
        (
            "accountant",
            vec!["payments.record", "cash.close", "vouchers.receipt.approve"],
            vec!["users.manage", "backup.restore", "students.create"],
        ),
        (
            "registration_clerk",
            vec!["students.create", "courses.view", "payments.record", "vouchers.receipt.create"],
            vec!["users.manage", "backup.restore", "cash.close", "ledger.view"],
        ),
        (
            "read_only",
            vec!["students.view"],
            vec!["students.create", "payments.record", "users.manage"],
        ),
    ];

    for (role, must_have, must_not) in &matrix {
        sqlx::query(
            "INSERT OR IGNORE INTO users (id, username, full_name, password_hash)
             VALUES (700, ?, 'Test User', 'x')",
        )
        .bind(format!("u_{role}"))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT OR REPLACE INTO user_roles (user_id, role_id)
             VALUES (700, (SELECT id FROM roles WHERE code = ?))",
        )
        .bind(role)
        .execute(&pool)
        .await
        .unwrap();

        let ps = ec_core::auth::permissions::load_for_user(&pool, 700).await
            .unwrap_or_else(|e| panic!("load {role}: {e}"));
        for want in must_have {
            assert!(
                ps.can(want),
                "role '{role}' must hold '{want}' (got: {:?})",
                ps.permissions
            );
        }
        for deny in must_not {
            assert!(!ps.can(deny), "role '{role}' must NOT hold '{deny}'");
        }
        // cleanup for next matrix row
        sqlx::query("DELETE FROM user_roles WHERE user_id = 700")
            .execute(&pool)
            .await
            .unwrap();
    }
}

/// sec006: security events from this phase produce audit rows; the chain
/// is immutable (covered again on fresh inserts here).
#[tokio::test]
async fn sec006_audit_coverage_login_events() {
    let (pool, _t, svc, _token) = admin_ready().await;
    // user_created at bootstrap
    assert_eq!(
        ec_core::auth::audit::count_action(&pool, "user_created").await.unwrap(),
        1
    );
    // one failure
    svc.login("admin", "nope").await.ok();
    assert_eq!(
        ec_core::auth::audit::count_action(&pool, "login_failed").await.unwrap(),
        1
    );
    // successful logins counted (admin_ready logged in once)
    assert!(
        ec_core::auth::audit::count_action(&pool, "login").await.unwrap() >= 1
    );
    // rows cannot be altered even by tests
    let err = sqlx::query("UPDATE audit_log SET action='tampered'")
        .execute(&pool)
        .await;
    assert!(err.is_err(), "audit must be append-only");
}

/// admin reset forces must_change plus logs with reason.
#[tokio::test]
async fn auth006_admin_password_reset() {
    let (_pool, _t, svc, token) = admin_ready().await;
    // reset self (admin has users.password_reset)
    svc.admin_reset_password(&token, 1, "Reset-Password-1", "test reason")
        .await
        .unwrap();
    let must: bool = sqlx::query_scalar("SELECT must_change_password FROM users WHERE id=1")
        .fetch_one(&svc.pool)
        .await
        .unwrap();
    assert!(must, "admin reset must force change at next login");
    // new password accepted, old refused
    assert!(svc.login("admin", "Sup3r-Secret!").await.is_err());
    let res = svc.login("admin", "Reset-Password-1").await.unwrap();
    assert!(res.must_change_password);
    // reset forbidden without permission: make a role-less user
    let clerk = ec_core::auth::service::create_user(
        &svc.pool, 1, "helper", "Helper", "Helper-Pass-1", &["read_only"],
    )
    .await
    .unwrap();
    let login = svc.login("helper", "Helper-Pass-1").await.unwrap();
    assert!(login.must_change_password);
    let err = svc
        .admin_reset_password(&login.token, 1, "Xpassword1", "no")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("صلاحية"), "err: {err}");
    let _ = clerk;
}
