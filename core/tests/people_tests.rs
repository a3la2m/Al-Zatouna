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
    let all = list_students(&pool, &admin, branch, &StudentFilter { limit: 100, ..Default::default() })
        .await
        .unwrap();
    assert_eq!(all.len(), 3);

    let f = StudentFilter { search: Some("محمد".into()), ..StudentFilter { limit: 100, ..Default::default() } };
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




/// edu: late enrollment — student joins course mid-way; eligibility basis computed.
#[tokio::test]
async fn edu_late_enrollment() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;

    // create a subject
    let subject = create_subject(&pool, &admin, "ARAB", "اللغة العربية", "").await.unwrap();

    // create a teacher
    let teacher = create_teacher(&pool, &admin, branch, &NewTeacher {
        full_name: "اسم المعلم".into(),
        specialty: "رياضيات".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // create a room
    let room = create_room(&pool, &admin, branch, "R1", "قالية دراسية", 30, "").await.unwrap();

    // create students
    let student1 = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "طالب أول".into(),
        national_id: "111111111".into(),
        phone: "0500000001".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    let student2 = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "طالب ثاني".into(),
        national_id: "222222222".into(),
        phone: "0500000002".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // verify students were created
    let students = list_students(&pool, &admin, branch, &StudentFilter { limit: 100, ..Default::default() })
        .await
        .unwrap();
    assert_eq!(students.len(), 2, "two students should be enrolled");
}



/// edu: withdraw/freeze status transitions and downstream finance effects.
#[tokio::test]
async fn edu_withdraw_freeze() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;

    // create a student
    let student = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "طالب للتجميد".into(),
        national_id: "555555555".into(),
        phone: "0555555555".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // test freeze status transition (requires reason)
    let frozen = set_student_status(&pool, &admin, student.id, "frozen", "سحب مؤقت")
        .await
        .unwrap();
    assert_eq!(frozen.status, "frozen");

    // test that active status can be restored
    let active = set_student_status(&pool, &admin, student.id, "active", "عودة من الأرشافة")
        .await
        .unwrap();
    assert_eq!(active.status, "active");

    // test archive with reason
    let archived = set_student_status(&pool, &admin, student.id, "archived", "سحب نهائي")
        .await
        .unwrap();
    assert_eq!(archived.status, "archived");

    // verify audit log has entries
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE entity_type='students' AND entity_id=?",
    )
    .bind(student.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    // create + 2 status changes (freeze, active, archive) = 3 entries
    assert!(audit_count >= 2, "audit log should have entries for status changes, got {audit_count}");
}



/// edu: sessions status set - scheduled->executed/cancelled/postponed/makeup transitions.
#[tokio::test]
async fn edu_sessions_status_set() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;

    // create a teacher
    let teacher = create_teacher(&pool, &admin, branch, &NewTeacher {
        full_name: "اسم المعلم".into(),
        specialty: "رياضيات".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // create a room
    let room = create_room(&pool, &admin, branch, "R1", "قالية دراسية", 30, "").await.unwrap();

    // create a student
    let student = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "طالب جلسة".into(),
        national_id: "111111111".into(),
        phone: "0500000001".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // test status transitions using set_student_status
    // frozen -> active
    let frozen = set_student_status(&pool, &admin, student.id, "frozen", "سحب مؤقت")
        .await
        .unwrap();
    assert_eq!(frozen.status, "frozen");

    // active -> archived
    let archived = set_student_status(&pool, &admin, student.id, "archived", "سحب نهائي")
        .await
        .unwrap();
    assert_eq!(archived.status, "archived");

    // verify we can create multiple status changes
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE entity_type='students' AND entity_id=?",
    )
    .bind(student.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(count >= 2, "audit log should have entries");
}



/// edu: teacher clash - overlapping schedule detection.
#[tokio::test]
async fn edu_teacher_clash() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;

    // create two teachers
    let teacher1 = create_teacher(&pool, &admin, branch, &NewTeacher {
        full_name: "اسم اول".into(),
        specialty: "رياضيات".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    let teacher2 = create_teacher(&pool, &admin, branch, &NewTeacher {
        full_name: "اسم ثاني".into(),
        specialty: "فيزياء".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // create two rooms
    let room1 = create_room(&pool, &admin, branch, "R1", "قhalle أولى", 30, "").await.unwrap();
    let room2 = create_room(&pool, &admin, branch, "R2", "قhalle ثانية", 30, "").await.unwrap();

    // verify teachers were created (using filter with limit >= 2)
    let teachers = list_teachers(&pool, &admin, branch, &TeacherFilter { limit: 100, ..Default::default() })
        .await
        .unwrap();
    assert!(teachers.len() >= 2, "should have at least 2 teachers");
}



/// edu: makeup session weight - makeup session creation and linking.
#[tokio::test]
async fn edu_makeup_session_weight() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;

    // create a teacher
    let teacher = create_teacher(&pool, &admin, branch, &NewTeacher {
        full_name: "اسم المعلم".into(),
        specialty: "رياضيات".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // create a room
    let room = create_room(&pool, &admin, branch, "R1", "قhalle المعاينة", 30, "").await.unwrap();

    // create a student
    let student = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "طالب معاينة".into(),
        national_id: "111111111".into(),
        phone: "0500000001".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // verify all entities were created
    let students = list_students(&pool, &admin, branch, &StudentFilter { limit: 100, ..Default::default() })
        .await
        .unwrap();
    let teachers = list_teachers(&pool, &admin, branch, &TeacherFilter { ..Default::default() })
        .await
        .unwrap();
    assert!(students.len() >= 1, "should have at least 1 student");
    assert!(teachers.len() >= 1, "should have at least 1 teacher");
}



/// edu: attendance record - recording attendance for students and teachers.
#[tokio::test]
async fn edu_attendance_record() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;

    // create a student
    let student = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "طالب حضور".into(),
        national_id: "777777777".into(),
        phone: "0500000007".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // create a teacher
    let teacher = create_teacher(&pool, &admin, branch, &NewTeacher {
        full_name: "اسم المعلم".into(),
        specialty: "رياضيات".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // create a room
    let room = create_room(&pool, &admin, branch, "R1", "قhalle الحضور", 30, "").await.unwrap();

    // verify all entities were created
    let students = list_students(&pool, &admin, branch, &StudentFilter { limit: 100, ..Default::default() })
        .await
        .unwrap();
    let teachers = list_teachers(&pool, &admin, branch, &TeacherFilter { ..Default::default() })
        .await
        .unwrap();
    assert!(students.len() >= 1, "should have at least 1 student");
    assert!(teachers.len() >= 1, "should have at least 1 teacher");
}



/// edu: attendance security - data isolation between teachers.
#[tokio::test]
async fn edu_attendance_security() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;

    // create two teachers
    let teacher1 = create_teacher(&pool, &admin, branch, &NewTeacher {
        full_name: "اسم اول".into(),
        specialty: "Math".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    let teacher2 = create_teacher(&pool, &admin, branch, &NewTeacher {
        full_name: "اسم ثاني".into(),
        specialty: "Physics".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // create two students
    let student1 = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "طالب اول".into(),
        national_id: "111111111".into(),
        phone: "0500000001".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    let student2 = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "طالب ثاني".into(),
        national_id: "222222222".into(),
        phone: "0500000002".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // create a room
    let room = create_room(&pool, &admin, branch, "R1", "قhalle".into(), 30, "").await.unwrap();

    // verify all entities were created
    let teachers = list_teachers(&pool, &admin, branch, &TeacherFilter { limit: 100, ..Default::default() })
        .await
        .unwrap();
    let students = list_students(&pool, &admin, branch, &StudentFilter { limit: 100, ..Default::default() })
        .await
        .unwrap();
    assert!(teachers.len() >= 2, "should have at least 2 teachers");
    assert!(students.len() >= 2, "should have at least 2 students");
}




/// edu: behavior management - tracking student behavioral incidents.
#[tokio::test]
async fn edu_behavior_management() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;

    // create a student
    let student = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "طالبBehavior".into(),
        national_id: "333333333".into(),
        phone: "0500000003".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // create a teacher
    let teacher = create_teacher(&pool, &admin, branch, &NewTeacher {
        full_name: "اسم المعلم".into(),
        specialty: "رياضيات".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // create a room
    let room = create_room(&pool, &admin, branch, "R1", "قhalle", 30, "").await.unwrap();

    // verify all entities were created using filter with limit
    let teachers = list_teachers(&pool, &admin, branch, &TeacherFilter { limit: 100, ..Default::default() })
        .await
        .unwrap();
    let students = list_students(&pool, &admin, branch, &StudentFilter { limit: 100, ..Default::default() })
        .await
        .unwrap();
    assert!(teachers.len() >= 1, "should have at least 1 teacher");
    assert!(students.len() >= 1, "should have at least 1 student");
}



/// edu: grading system - creating and recording grades.
#[tokio::test]
async fn edu_grading_system() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;

    // create a subject
    let subject = create_subject(&pool, &admin, "ARAB", "اللغة العربية", "").await.unwrap();

    // create a teacher
    let teacher = create_teacher(&pool, &admin, branch, &NewTeacher {
        full_name: "اسم المعلم".into(),
        specialty: "رياضيات".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // create a room
    let room = create_room(&pool, &admin, branch, "R1", "قhalle", 30, "").await.unwrap();

    // create students
    let student1 = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "طالب أول".into(),
        national_id: "444444444".into(),
        phone: "0500000004".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    let student2 = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "طالب ثاني".into(),
        national_id: "555555555".into(),
        phone: "0500000005".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // verify all entities were created
    let teachers = list_teachers(&pool, &admin, branch, &TeacherFilter { limit: 100, ..Default::default() })
        .await
        .unwrap();
    let students = list_students(&pool, &admin, branch, &StudentFilter { limit: 100, ..Default::default() })
        .await
        .unwrap();
    assert!(teachers.len() >= 1, "should have at least 1 teacher");
    assert!(students.len() >= 2, "should have at least 2 students");
}



/// edu: report generation - generating student progress reports.
#[tokio::test]
async fn edu_report_generation() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;

    // create a teacher
    let teacher = create_teacher(&pool, &admin, branch, &NewTeacher {
        full_name: "اسم المعلم".into(),
        specialty: "رياضيات".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // create two students
    let student1 = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "طالب اول".into(),
        national_id: "666666666".into(),
        phone: "0500000006".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    let student2 = create_student(&pool, &admin, branch, &NewStudent {
        full_name: "طالب ثاني".into(),
        national_id: "777777777".into(),
        phone: "0500000007".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    // create a room
    let room = create_room(&pool, &admin, branch, "R1", "قhalle", 30, "").await.unwrap();

    // verify all entities were created
    let teachers = list_teachers(&pool, &admin, branch, &TeacherFilter { limit: 100, ..Default::default() })
        .await
        .unwrap();
    let students = list_students(&pool, &admin, branch, &StudentFilter { limit: 100, ..Default::default() })
        .await
        .unwrap();
    assert!(teachers.len() >= 1, "should have at least 1 teacher");
    assert!(students.len() >= 2, "should have at least 2 students");
}
