//! Phase 10 dashboard & report tests.

use ec_core::database::test_utils::fresh_seeded_pool;
use ec_core::dashboard::compute_kpis;
use ec_core::people::context::Actor;
use ec_core::reports::{is_valid_column, save_report, list_reports};
use ec_core::auth::permissions::load_for_user;
use sqlx::SqlitePool;

async fn admin_actor(pool: &SqlitePool) -> Actor {
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, full_name, password_hash) VALUES ('dadmin','Admin','x') RETURNING id",
    ).fetch_one(pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, (SELECT id FROM roles WHERE code='system_admin'))")
        .bind(uid).execute(pool).await.unwrap();
    let ps = load_for_user(pool, uid).await.unwrap();
    Actor { user_id: uid, username: "dadmin".into(), permissions: ps }
}

#[tokio::test]
async fn dash001_kpis_correct() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    let kpis = compute_kpis(&pool).await.unwrap();
    // seeded DB has 0 students/courses/enrollments
    assert_eq!(kpis.students_total, 0);
    assert_eq!(kpis.courses_total, 0);
    assert_eq!(kpis.enrollments_active, 0);
    // insert a student and recompute
    sqlx::query("INSERT INTO students (branch_id, student_no, full_name) VALUES (1, 1, 'س')").execute(&pool).await.unwrap();
    let kpis2 = compute_kpis(&pool).await.unwrap();
    assert_eq!(kpis2.students_total, 1);
    assert_eq!(kpis2.students_active, 1);
}

#[tokio::test]
async fn rpt001_permissions_respected() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    // teacher role has courses.view but not users.manage
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, full_name, password_hash) VALUES ('t1','T','x') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, (SELECT id FROM roles WHERE code='teacher'))")
        .bind(uid).execute(&pool).await.unwrap();
    let ps = load_for_user(&pool, uid).await.unwrap();
    assert!(ps.can("courses.view"));
    assert!(!ps.can("users.manage"));
    assert!(!ps.can("payments.record"));
}

#[tokio::test]
async fn rpt002_saved_report_builder() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    // valid column accepted
    assert!(is_valid_column("students", "full_name"));
    // unknown column rejected (no raw SQL accepted)
    assert!(!is_valid_column("students", "password_hash; DROP TABLE students;--"));
    assert!(!is_valid_column("students", "nonexistent"));
    // save + list
    let id = save_report(&pool, "تقرير 1", r#"{"columns":["full_name"]}"#).await.unwrap();
    assert!(id > 0);
    let list = list_reports(&pool).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "تقرير 1");
}

#[tokio::test]
async fn rpt006_excel_export_valid() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    sqlx::query("INSERT INTO students (branch_id, student_no, full_name) VALUES (1, 1, 'س')").execute(&pool).await.unwrap();
    let bytes = ec_core::reports::export_excel_students(&pool).await.unwrap();
    assert!(bytes.len() > 100);
    // xlsx magic bytes
    assert_eq!(&bytes[0..2], b"PK");
}
