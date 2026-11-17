//! Phase 11 high-priority backend tests.

use ec_core::auth::service::AuthService;
use ec_core::config;
use ec_core::database::test_utils::fresh_seeded_pool;
use ec_core::education::agreements::{approve_agreement, create_agreement_draft};
use ec_core::education::courses::{create_course, set_course_status, NewCourse};
use ec_core::finance::ledger::{ledger_balanced, post_entry, JournalLine, NewJournalEntry};
use ec_core::finance::{accrue_enrollment, recognize_revenue};
use ec_core::people::context::Actor;
use ec_core::people::students::create_student;
use ec_core::people::teachers::{create_room, create_subject, create_teacher, NewTeacher};
use ec_core::auth::permissions::load_for_user;
use sqlx::SqlitePool;
use tempfile::TempDir;

async fn admin_actor(pool: &SqlitePool) -> Actor {
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, full_name, password_hash) VALUES ('p11admin','Admin','x') RETURNING id",
    ).fetch_one(pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, (SELECT id FROM roles WHERE code='system_admin'))")
        .bind(uid).execute(pool).await.unwrap();
    let ps = load_for_user(pool, uid).await.unwrap();
    Actor { user_id: uid, username: "p11admin".into(), permissions: ps }
}

async fn setup_course(pool: &SqlitePool, admin: &Actor, branch: i64) -> i64 {
    let subject = create_subject(pool, admin, "M", "رياضيات", "").await.unwrap();
    let teacher = create_teacher(pool, admin, branch, &NewTeacher { full_name: "م".into(), ..Default::default() }).await.unwrap();
    let room = create_room(pool, admin, branch, "R", "ق", 10, "").await.unwrap();
    let course = create_course(pool, admin, branch, &NewCourse {
        name: "د".into(), subject_id: subject.id, teacher_id: teacher.id, room_id: room.id,
        fee_minor: 10000, ..Default::default()
    }).await.unwrap();
    set_course_status(pool, admin, course.id, "registration_open", "").await.unwrap();
    course.id
}

/// edu011: course cannot be approved without active agreement.
#[tokio::test]
async fn edu011_course_requires_agreement() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let course_id = setup_course(&pool, &admin, branch).await;
    let r = set_course_status(&pool, &admin, course_id, "active", "").await;
    assert!(r.is_err(), "approve without agreement must fail");
    // add agreement draft+approve
    let draft = create_agreement_draft(&pool, &admin, course_id, "flat_course", None, None, Some(5000), None, "{}", "{}").await.unwrap();
    approve_agreement(&pool, &admin, draft.id, admin.user_id, Some("ov")).await.unwrap();
    let c = set_course_status(&pool, &admin, course_id, "active", "").await.unwrap();
    assert_eq!(c.status, "active");
}

/// sec007: disable user blocks login.
#[tokio::test]
async fn sec007_disable_user_blocks_login() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    let svc = AuthService::new(pool.clone());
    svc.create_first_admin("admin", "Admin", "Sup3r-Secret!").await.unwrap();
    let admin_token = svc.login("admin", "Sup3r-Secret!").await.unwrap().token;
    // create a second user
    let uid = ec_core::auth::service::create_user(&pool, 1, "helper", "Helper", "Helper-Pass-1", &["read_only"]).await.unwrap();
    let svc2 = AuthService::new(pool.clone());
    let _ = svc2.login("helper", "Helper-Pass-1").await.unwrap();
    // disable via admin
    let svc3 = AuthService::new(pool.clone());
    // share session store? AuthService::new creates new store; use the original svc for session. Instead call disable via a fresh service with same pool (sessions not shared but disable doesn't need session verify? it does). Use admin_token on original svc.
    svc.disable_user(&admin_token, uid, "test").await.unwrap();
    // login now blocked
    let r = svc.login("helper", "Helper-Pass-1").await;
    assert!(r.is_err(), "disabled user must not log in");
    let msg = r.unwrap_err().to_string();
    assert!(msg.contains("معطّل") || msg.contains("معطل"), "msg: {}", msg);
}

/// fin019: revenue recognition accrual posts balanced entry.
#[tokio::test]
async fn fin019_revenue_recognition_accrual() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let cash: i64 = sqlx::query_scalar("SELECT id FROM accounts LIMIT 1").fetch_one(&pool).await.unwrap();
    let deferred: i64 = sqlx::query_scalar("SELECT id FROM accounts LIMIT 1 OFFSET 1").fetch_one(&pool).await.unwrap();
    let revenue: i64 = sqlx::query_scalar("SELECT id FROM accounts LIMIT 1 OFFSET 2").fetch_one(&pool).await.unwrap();
    let recv = cash;
    let je1 = accrue_enrollment(&pool, &admin, 1, 5000, recv, deferred).await.unwrap();
    assert!(je1 > 0);
    let je2 = recognize_revenue(&pool, &admin, 1, 2000, deferred, revenue).await.unwrap();
    assert!(je2 > 0);
    assert!(ledger_balanced(&pool).await.unwrap());
}

/// cfg001: free space check returns a value.
#[tokio::test]
async fn cfg001_free_space_check() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    let _ = pool;
    let dd = config::data_dir::DataDir::from_root(std::path::PathBuf::from("/tmp"));
    assert!(dd.free_space_bytes() > 0);
}

/// cfg002: data dir change roundtrip.
#[tokio::test]
async fn cfg002_data_dir_change_roundtrip() {
    let tmp1 = TempDir::new().unwrap();
    let (pool, dd) = config::bootstrap_first_run(Some(tmp1.path())).await.unwrap();
    pool.close().await;
    let tmp2 = TempDir::new().unwrap();
    let (pool2, dd2) = config::change_data_dir(tmp2.path()).await.unwrap();
    assert!(dd2.db_path().exists());
    let chk = ec_core::database::integrity_check(&pool2).await.unwrap();
    assert_eq!(chk, "ok");
    pool2.close().await;
    let _ = dd;
}

/// sec008: teacher scope enforced.
#[tokio::test]
async fn sec008_teacher_scope_enforced() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    // admin has no linked teacher -> None
    let scope = ec_core::people::teachers::scope_for_teacher(&pool, &admin).await.unwrap();
    assert!(scope.is_none());
    // link user to a teacher
    sqlx::query("INSERT INTO teachers (branch_id, teacher_no, full_name) VALUES (1, 77, 'م')").execute(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar("SELECT id FROM teachers WHERE teacher_no=77").fetch_one(&pool).await.unwrap();
    sqlx::query("UPDATE users SET teacher_id = ? WHERE id = ?").bind(tid).bind(admin.user_id).execute(&pool).await.unwrap();
    let scope2 = ec_core::people::teachers::scope_for_teacher(&pool, &admin).await.unwrap();
    assert_eq!(scope2, Some(tid));
}
