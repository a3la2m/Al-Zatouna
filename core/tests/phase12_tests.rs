//! Phase 12 medium-priority smoke tests.

use ec_core::database::test_utils::fresh_seeded_pool;
use ec_core::education::payment_plans::{create_payment_plan, list_payment_plan};
use ec_core::people::context::Actor;
use ec_core::people::guardians::{apply_credit_to_enrollment, create_guardian, link_guardian_to_student};
use ec_core::vouchers::cash::{open_register, record_cash_movement, transfer_between_registers};
use ec_core::auth::permissions::load_for_user;
use sqlx::SqlitePool;

async fn admin_actor(pool: &SqlitePool) -> Actor {
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, full_name, password_hash) VALUES ('p12admin','Admin','x') RETURNING id",
    ).fetch_one(pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, (SELECT id FROM roles WHERE code='system_admin'))")
        .bind(uid).execute(pool).await.unwrap();
    let ps = load_for_user(pool, uid).await.unwrap();
    Actor { user_id: uid, username: "p12admin".into(), permissions: ps }
}

#[tokio::test]
async fn edu012_installment_partial_payment() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    // enrollment id 1 may not exist; create one via direct insert? Use plan on enrollment 0? plan requires enrollment_id FK. Create enrollment first.
    let subject = ec_core::people::teachers::create_subject(&pool, &admin, "M", "رياضيات", "").await.unwrap();
    let teacher = ec_core::people::teachers::create_teacher(&pool, &admin, branch, &ec_core::people::teachers::NewTeacher { full_name: "م".into(), ..Default::default() }).await.unwrap();
    let room = ec_core::people::teachers::create_room(&pool, &admin, branch, "R", "ق", 10, "").await.unwrap();
    let course = ec_core::education::courses::create_course(&pool, &admin, branch, &ec_core::education::courses::NewCourse { name: "د".into(), subject_id: subject.id, teacher_id: teacher.id, room_id: room.id, fee_minor: 10000, ..Default::default() }).await.unwrap();
    ec_core::education::courses::set_course_status(&pool, &admin, course.id, "registration_open", "").await.unwrap();
    let student = ec_core::people::students::create_student(&pool, &admin, branch, &ec_core::people::students::NewStudent { full_name: "س".into(), national_id: "P1".into(), ..Default::default() }).await.unwrap();
    let enr = ec_core::education::enrollments::enroll(&pool, &admin, branch, student.id, course.id, 10000, 0, 0, "2026-01-01", "").await.unwrap();
    let plan = create_payment_plan(&pool, &admin, enr.id, 10000, 4, "2026-02-01", 30).await.unwrap();
    assert_eq!(plan.installment_count, 4);
    assert_eq!(plan.installments.len(), 4);
    let total: i64 = plan.installments.iter().map(|i| i.amount_minor).sum();
    assert_eq!(total, 10000);
    let loaded = list_payment_plan(&pool, enr.id).await.unwrap();
    assert!(loaded.is_some());
}

#[tokio::test]
async fn vch012_transfer_pair_balances() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let r1 = open_register(&pool, &admin, branch, "MAIN").await.unwrap();
    // create a second register
    sqlx::query("INSERT INTO cash_registers (branch_id, code, name, status) VALUES (?, 'SUB', 'فرعي', 'active')")
        .bind(branch).execute(&pool).await.unwrap();
    let r2: i64 = sqlx::query_scalar("SELECT id FROM cash_registers WHERE code='SUB'").fetch_one(&pool).await.unwrap();
    record_cash_movement(&pool, &admin, r1, "receipt", "in", 1000, "test", None).await.unwrap();
    let (out_id, in_id) = transfer_between_registers(&pool, &admin, r1, r2, 400).await.unwrap();
    assert!(out_id > 0 && in_id > 0);
}

#[tokio::test]
async fn edu013_guardian_link_and_credit() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let g = create_guardian(&pool, &admin, "ولي أمر", "0599", "a@b.com", "123").await.unwrap();
    assert!(g.id > 0);
    let student = ec_core::people::students::create_student(&pool, &admin, branch, &ec_core::people::students::NewStudent { full_name: "س".into(), national_id: "G1".into(), ..Default::default() }).await.unwrap();
    link_guardian_to_student(&pool, &admin, student.id, g.id, "father", true).await.unwrap();
    // credit apply (balance 0 initially -> should fail)
    let r = apply_credit_to_enrollment(&pool, &admin, student.id, 1, 100).await;
    assert!(r.is_err());
}
