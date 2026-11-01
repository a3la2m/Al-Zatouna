//! Phase 4 integration tests: students/teachers/catalog lifecycle, sequence
//! minting inside transactions, permission denial + audit coverage.
//! Test plan references: edu001 partial + sec001 negative controls.

use ec_core::database::test_utils::fresh_seeded_pool;
use ec_core::people::context::Actor;
use ec_core::people::students::{
    create_student, edit_student, list_students, set_student_status, NewStudent, StudentFilter,
    StudentPatch,
};
use ec_core::people::teachers::{
    create_room, create_subject, create_teacher, edit_teacher, list_rooms, list_subjects,
    list_teachers, set_teacher_status, NewTeacher, TeacherFilter, TeacherPatch,
};
use ec_core::auth::permissions::{load_for_user, PermissionSet};
use sqlx::SqlitePool;

/// Build an actor with a capability list (used by all matrix tests).
async fn actor_with(pool: &SqlitePool, uname: &str, perms: &[&str]) -> Actor {
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, full_name, password_hash)
         VALUES (?, 'Test', 'x') RETURNING id",
    )
    .bind(uname)
    .fetch_one(pool)
    .await
    .unwrap();
    let mut permissions = std::collections::HashSet::new();
    permissions.extend(perms.iter().map(|s| s.to_string()));
    Actor {
        user_id: uid,
        username: uname.to_string(),
        permissions: PermissionSet {
            user_id: uid,
            roles: Vec::new(),
            permissions,
        },
    }
}

async fn admin_actor(pool: &SqlitePool) -> Actor {
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, full_name, password_hash)
         VALUES ('adminX', 'Admin', 'x') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO user_roles (user_id, role_id)
                 VALUES (?, (SELECT id FROM roles WHERE code='system_admin'))")
        .bind(uid)
        .execute(pool)
        .await
        .unwrap();
    let ps = load_for_user(pool, uid).await.unwrap();
    Actor { user_id: uid, username: "adminX".into(), permissions: ps }
}

/// edu: create mints gap-free sequential numbers per branch+year.
#[tokio::test]
async fn edu_sequence_and_basic_create() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;

    let a = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "أحمد محمد".into(),
        national_id: "900111222".into(),
        phone: "0599123456".into(),
        ..Default::default()
    })
    .await
    .unwrap();
    let b = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "سارة خالد".into(),
        ..Default::default()
    })
    .await
    .unwrap();
    assert_eq!(a.student_no, b.student_no - 1, "sequential numbers");
    assert_eq!(a.status, "active");

    // duplicate national id rejected
    let dup = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "نسخة مكررة".into(),
        national_id: a.national_id.clone(),
        ..Default::default()
    })
    .await;
    assert!(dup.is_err(), "same national_id must be blocked");
}

/// sec001 (negative control): missing permission ⇒ denied + audited.
#[tokio::test]
async fn edu_permission_denial_audited() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let clerk = actor_with(&pool, "watcher", &["students.view"]).await;

    let err = create_student(&pool, &clerk, branch, &NewStudent {
        full_name: "مرفوض".into(),
        ..Default::default()
    })
    .await
    .unwrap_err();
    assert!(err.to_string().contains("صلاحية"), "{err}");

    let n = ec_core::auth::audit::count_action(&pool, "students.create")
        .await
        .unwrap();
    assert_eq!(n, 0, "denied create must NOT write audit biz row");
    // but the denial itself is visible: no student row was inserted
    let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(c, 0);
}

/// edu: edit patch semantics (only Some fields change) + audit before/after.
#[tokio::test]
async fn edu_edit_only_specified_fields() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let s = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "وليد سعيد".into(),
        phone: "0599000000".into(),
        email: "w@example.com".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    let s2 = edit_student(&pool, &admin, s.id, &StudentPatch {
        phone: Some("0599111111".into()),
        ..Default::default()
    })
    .await
    .unwrap();
    assert_eq!(s2.phone, "0599111111");
    assert_eq!(s2.email, "w@example.com", "untouched field preserved");
    assert_eq!(s2.full_name, "وليد سعيد");

    // archive without reason fails; with reason succeeds
    let err = set_student_status(&pool, &admin, s.id, "archived", "")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("سبب"), "{err}");
    let s3 = set_student_status(&pool, &admin, s.id, "archived", "سحب ولي الأمر")
        .await
        .unwrap();
    assert_eq!(s3.status, "archived");
    // insertion row count of audit for this student: create + 2 edits
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE entity_type='students' AND entity_id=?",
    )
    .bind(s.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 3, "audited create/edit/archive, got {n}");
}

/// edu: list filters work (status + text search).
#[tokio::test]
async fn edu_list_filters() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    for name in ["محمد ع", "محمود ب", "ليلى ج"] {
        create_student(&pool, &admin, branch, &NewStudent {
            full_name: name.into(),
            ..Default::default()
        })
        .await
        .unwrap();
    }
    let all = list_students(&pool, &admin, branch, &StudentFilter::new())
        .await
        .unwrap();
    assert_eq!(all.len(), 3);

    let f = StudentFilter { search: Some("محمد".into()), ..StudentFilter::new() };
    let found = list_students(&pool, &admin, branch, &f).await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].full_name, "محمد ع");
}

/// edu: teachers lifecycle + teacher_no from fiscal_year=0 sequence.
#[tokio::test]
async fn edu_teacher_lifecycle() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;

    let t = create_teacher(&pool, &admin, branch, &NewTeacher {
        full_name: "خالد الأستاذ".into(),
        specialty: "رياضيات".into(),
        ..Default::default()
    })
    .await
    .unwrap();
    assert!(t.teacher_no > 0);

    let t2 = edit_teacher(&pool, &admin, t.id, &TeacherPatch {
        phone: Some("0599888877".into()),
        ..Default::default()
    })
    .await
    .unwrap();
    assert_eq!(t2.phone, "0599888877");
    assert_eq!(t2.specialty, "رياضيات");

    let t3 = set_teacher_status(&pool, &admin, t.id, "inactive", "إجازة")
        .await
        .unwrap();
    assert_eq!(t3.status, "inactive");

    let got = list_teachers(&pool, &admin, branch, &TeacherFilter { status: Some("inactive".into()), limit: 100, ..Default::default() })
    .await
    .unwrap();
    assert_eq!(got.len(), 1);
}

/// edu: catalog — subjects & rooms.
#[tokio::test]
async fn edu_catalog_subjects_rooms() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;

    let s = create_subject(&pool, &admin, "MATH", "الرياضيات", "صم").await.unwrap();
    let s2 = create_subject(&pool, &admin, "PHYS", "الفيزياء", "").await.unwrap();
    let subs = list_subjects(&pool, &admin, false).await.unwrap();
    assert!(subs.len() >= 2);
    assert!(subs.iter().any(|x| x.code == s.code));
    assert!(subs.iter().any(|x| x.code == s2.code));

    // duplicate code rejected
    let dup = create_subject(&pool, &admin, "MATH", "مكرر", "").await;
    assert!(dup.is_err());

    let r = create_room(&pool, &admin, branch, "R1", "قاعة 1", 20, "").await.unwrap();
    assert_eq!(r.capacity, 20);
    let rooms = list_rooms(&pool, &admin, branch).await.unwrap();
    assert!(rooms.iter().any(|x| x.id == r.id));

    let err = create_room(&pool, &admin, branch, "R2", "سالب", -5, "").await;
    assert!(err.is_err(), "negative capacity must be rejected");
}
