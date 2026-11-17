//! Phase 5 integration tests: courses/sessions/attendance/enrollments/agreements.
//! Test plan §3 edu001–edu010.

use ec_core::database::test_utils::fresh_seeded_pool;
use ec_core::education::courses::{create_course, set_course_status, NewCourse};
use ec_core::education::sessions::{create_session, set_session_status, link_makeup_original};
use ec_core::education::attendance::{record_student_attendance, record_teacher_attendance};
use ec_core::education::enrollments::{enroll, set_enrollment_status};
use ec_core::education::agreements::{create_agreement_draft, approve_agreement};
use ec_core::people::context::Actor;
use ec_core::people::students::create_student;
use ec_core::people::teachers::create_teacher;
use ec_core::people::teachers::create_room;
use ec_core::people::teachers::create_subject;
use ec_core::auth::permissions::load_for_user;
use sqlx::SqlitePool;

async fn admin_actor(pool: &SqlitePool) -> Actor {
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, full_name, password_hash)
         VALUES ('adminE', 'Admin', 'x') RETURNING id",
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
    Actor { user_id: uid, username: "adminE".into(), permissions: ps }
}

async fn setup_course(pool: &SqlitePool, admin: &Actor, branch: i64, private: bool, max: i64) -> (i64, i64, i64) {
    let subject = create_subject(pool, admin, "MATH", "رياضيات", "").await.unwrap();
    let teacher = create_teacher(pool, admin, branch, &ec_core::people::teachers::NewTeacher {
        full_name: "مدرس".into(), ..Default::default()
    }).await.unwrap();
    let room = create_room(pool, admin, branch, "R1", "ق1", 30, "").await.unwrap();
    let course = create_course(pool, admin, branch, &NewCourse {
        name: "دورة".into(),
        subject_id: subject.id,
        teacher_id: teacher.id,
        room_id: room.id,
        fee_minor: 10000,
        is_private: private,
        max_students: max,
        min_students: 1,
        ..Default::default()
    }).await.unwrap();
    // move to registration_open
    let _ = set_course_status(pool, admin, course.id, "registration_open", "").await.unwrap();
    (course.id, subject.id, teacher.id)
}

/// edu001: private course allows exactly one enrollment.
#[tokio::test]
async fn edu001_single_student_course() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let (course_id, _, _) = setup_course(&pool, &admin, branch, true, 1).await;

    let s1 = create_student(&pool, &admin, branch, &ec_core::people::students::NewStudent {
        full_name: "طالب 1".into(), national_id: "111".into(), ..Default::default()
    }).await.unwrap();
    let e1 = enroll(&pool, &admin, branch, s1.id, course_id, 10000, 0, 0, "2026-01-01", "").await;
    assert!(e1.is_ok(), "first enrollment must succeed");

    let s2 = create_student(&pool, &admin, branch, &ec_core::people::students::NewStudent {
        full_name: "طالب 2".into(), national_id: "222".into(), ..Default::default()
    }).await.unwrap();
    let e2 = enroll(&pool, &admin, branch, s2.id, course_id, 10000, 0, 0, "2026-01-02", "").await;
    assert!(e2.is_err(), "second enrollment on private course must be rejected");
}

/// edu002: capacity min/max respected.
#[tokio::test]
async fn edu002_multi_students_course() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let (course_id, _, _) = setup_course(&pool, &admin, branch, false, 2).await;

    for i in 0..2 {
        let s = create_student(&pool, &admin, branch, &ec_core::people::students::NewStudent {
            full_name: format!("طالب {}", i), national_id: format!("n{}", i), ..Default::default()
        }).await.unwrap();
        enroll(&pool, &admin, branch, s.id, course_id, 10000, 0, 0, "2026-01-01", "").await.unwrap();
    }
    // third rejected (max=2)
    let s3 = create_student(&pool, &admin, branch, &ec_core::people::students::NewStudent {
        full_name: "طالب 3".into(), national_id: "n3".into(), ..Default::default()
    }).await.unwrap();
    let r = enroll(&pool, &admin, branch, s3.id, course_id, 10000, 0, 0, "2026-01-01", "").await;
    assert!(r.is_err(), "over-capacity enrollment must be rejected");
}

/// edu003: late enrollment records enrolled_date for eligibility basis.
#[tokio::test]
async fn edu003_late_enrollment() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let (course_id, _, _) = setup_course(&pool, &admin, branch, false, 10).await;
    let s = create_student(&pool, &admin, branch, &ec_core::people::students::NewStudent {
        full_name: "متأخر".into(), national_id: "L1".into(), ..Default::default()
    }).await.unwrap();
    let e = enroll(&pool, &admin, branch, s.id, course_id, 10000, 0, 0, "2026-02-15", "").await.unwrap();
    assert_eq!(e.enrolled_date, "2026-02-15");
    assert_eq!(e.status, "active");
}

/// edu004: withdraw/freeze status transitions.
#[tokio::test]
async fn edu004_withdraw_freeze() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let (course_id, _, _) = setup_course(&pool, &admin, branch, false, 10).await;
    let s = create_student(&pool, &admin, branch, &ec_core::people::students::NewStudent {
        full_name: "س".into(), national_id: "W1".into(), ..Default::default()
    }).await.unwrap();
    let e = enroll(&pool, &admin, branch, s.id, course_id, 10000, 0, 0, "2026-01-01", "").await.unwrap();

    // freeze requires reason
    let err = set_enrollment_status(&pool, &admin, e.id, "frozen", "").await;
    assert!(err.is_err());
    let f = set_enrollment_status(&pool, &admin, e.id, "frozen", "سفر").await.unwrap();
    assert_eq!(f.status, "frozen");
    let w = set_enrollment_status(&pool, &admin, e.id, "withdrawn", "انسحاب").await.unwrap();
    assert_eq!(w.status, "withdrawn");
}

/// edu005: session status transitions legal/illegal.
#[tokio::test]
async fn edu005_sessions_status_set() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let (course_id, _, _) = setup_course(&pool, &admin, branch, false, 10).await;
    let sess = create_session(&pool, &admin, course_id, "2026-03-01", "09:00", "10:00",
        0, 0, None, 1, None).await;
    // room/teacher ids invalid (0) — instead fetch course's room/teacher
    // redo properly:
    let _ = sess;
    // Use valid room/teacher from course
    let c = ec_core::education::courses::get_course(&pool, &admin, course_id).await.unwrap();
    let sess = create_session(&pool, &admin, course_id, "2026-03-01", "09:00", "10:00",
        c.room_id, c.teacher_id, None, 1, None).await.unwrap();

    // scheduled -> executed ok
    let e = set_session_status(&pool, &admin, sess.id, "executed", "تم", None, None).await.unwrap();
    assert_eq!(e.status, "executed");
    // executed -> cancelled illegal
    let bad = set_session_status(&pool, &admin, sess.id, "cancelled", "x", None, None).await;
    assert!(bad.is_err(), "executed→cancelled must be illegal");

    // new session for cancel/postpone tests
    let s2 = create_session(&pool, &admin, course_id, "2026-03-02", "09:00", "10:00",
        c.room_id, c.teacher_id, None, 2, None).await.unwrap();
    // cancelled requires reason
    let err = set_session_status(&pool, &admin, s2.id, "cancelled", "", None, None).await;
    assert!(err.is_err());
    let c2 = set_session_status(&pool, &admin, s2.id, "cancelled", "إجازة", None, None).await.unwrap();
    assert_eq!(c2.status, "cancelled");
}

/// edu006: room clash rejected.
#[tokio::test]
async fn edu006_room_clash() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let (course_id, _, _) = setup_course(&pool, &admin, branch, false, 10).await;
    let c = ec_core::education::courses::get_course(&pool, &admin, course_id).await.unwrap();

    create_session(&pool, &admin, course_id, "2026-04-01", "09:00", "10:00",
        c.room_id, c.teacher_id, None, 1, None).await.unwrap();
    // overlapping same room (different teacher) -> clash
    let teacher2 = create_teacher(&pool, &admin, branch, &ec_core::people::teachers::NewTeacher {
        full_name: "م2".into(), ..Default::default()
    }).await.unwrap();
    let r = create_session(&pool, &admin, course_id, "2026-04-01", "09:30", "10:30",
        c.room_id, teacher2.id, None, 2, None).await;
    assert!(r.is_err(), "room clash must be rejected");
    assert!(r.unwrap_err().to_string().contains("تعارض"));
}

/// edu007: teacher clash rejected; substitute teacher allowed when primary free.
#[tokio::test]
async fn edu007_teacher_clash() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let (course_id, _, _) = setup_course(&pool, &admin, branch, false, 10).await;
    let c = ec_core::education::courses::get_course(&pool, &admin, course_id).await.unwrap();

    create_session(&pool, &admin, course_id, "2026-04-02", "09:00", "10:00",
        c.room_id, c.teacher_id, None, 1, None).await.unwrap();
    let room2 = create_room(&pool, &admin, branch, "R2", "ق2", 30, "").await.unwrap();
    // same teacher overlapping -> clash
    let r = create_session(&pool, &admin, course_id, "2026-04-02", "09:30", "10:30",
        room2.id, c.teacher_id, None, 2, None).await;
    assert!(r.is_err(), "teacher clash must be rejected");

    // substitute teacher (different primary) allowed
    let teacher2 = create_teacher(&pool, &admin, branch, &ec_core::people::teachers::NewTeacher {
        full_name: "م2".into(), ..Default::default()
    }).await.unwrap();
    let r2 = create_session(&pool, &admin, course_id, "2026-04-02", "11:00", "12:00",
        room2.id, teacher2.id, Some(c.teacher_id), 3, None).await;
    assert!(r2.is_ok(), "substitute (primary free) should be allowed");
}

/// edu008: makeup session links to original; original neutralized.
#[tokio::test]
async fn edu008_makeup_session_weight() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let (course_id, _, _) = setup_course(&pool, &admin, branch, false, 10).await;
    let c = ec_core::education::courses::get_course(&pool, &admin, course_id).await.unwrap();
    let orig = create_session(&pool, &admin, course_id, "2026-04-03", "09:00", "10:00",
        c.room_id, c.teacher_id, None, 1, None).await.unwrap();
    // postpone original
    set_session_status(&pool, &admin, orig.id, "postponed", "مرض", None, None).await.unwrap();
    // create makeup
    let makeup = create_session(&pool, &admin, course_id, "2026-04-10", "09:00", "10:00",
        c.room_id, c.teacher_id, None, 2, None).await.unwrap();
    link_makeup_original(&pool, makeup.id, orig.id).await.unwrap();
    let sessions = ec_core::education::sessions::list_sessions(&pool, &admin, Some(course_id), None, None).await.unwrap();
    let m = sessions.iter().find(|s| s.id == makeup.id).unwrap();
    assert_eq!(m.original_session_id, Some(orig.id));
    assert_eq!(m.session_kind, "regular"); // kind default; caller may set 'makeup' via direct update
    // set kind to makeup
    sqlx::query("UPDATE sessions SET session_kind='makeup' WHERE id=?").bind(makeup.id).execute(&pool).await.unwrap();
    let sessions = ec_core::education::sessions::list_sessions(&pool, &admin, Some(course_id), None, None).await.unwrap();
    let m = sessions.iter().find(|s| s.id == makeup.id).unwrap();
    assert_eq!(m.session_kind, "makeup");
}

/// edu009: attendance recorded once per (session, student); late minutes; teacher attendance once.
#[tokio::test]
async fn edu009_attendance_record() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let (course_id, _, _) = setup_course(&pool, &admin, branch, false, 10).await;
    let c = ec_core::education::courses::get_course(&pool, &admin, course_id).await.unwrap();
    let sess = create_session(&pool, &admin, course_id, "2026-04-04", "09:00", "10:00",
        c.room_id, c.teacher_id, None, 1, None).await.unwrap();
    let s = create_student(&pool, &admin, branch, &ec_core::people::students::NewStudent {
        full_name: "حاضر".into(), national_id: "A1".into(), ..Default::default()
    }).await.unwrap();
    enroll(&pool, &admin, branch, s.id, course_id, 10000, 0, 0, "2026-01-01", "").await.unwrap();

    let a1 = record_student_attendance(&pool, &admin, sess.id, s.id, "late", 15, "تأخر").await.unwrap();
    assert_eq!(a1.late_minutes, 15);
    assert_eq!(a1.status, "late");
    // re-record updates (upsert) instead of duplicate
    let a2 = record_student_attendance(&pool, &admin, sess.id, s.id, "present", 0, "").await.unwrap();
    assert_eq!(a2.status, "present");
    let cnt: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM student_attendance WHERE session_id=? AND student_id=?",
    ).bind(sess.id).bind(s.id).fetch_one(&pool).await.unwrap();
    assert_eq!(cnt, 1, "must be one row per (session, student)");

    // teacher attendance once per session (upsert)
    let t1 = record_teacher_attendance(&pool, &admin, sess.id, c.teacher_id, "present", "").await.unwrap();
    assert_eq!(t1.status, "present");
    let t2 = record_teacher_attendance(&pool, &admin, sess.id, c.teacher_id, "late", "تأخر").await.unwrap();
    assert_eq!(t2.status, "late");
    let tcnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM teacher_attendance WHERE session_id=?")
        .bind(sess.id).fetch_one(&pool).await.unwrap();
    assert_eq!(tcnt, 1);
}

/// edu010: attendance permission denied for unauthorized actor.
#[tokio::test]
async fn edu010_attendance_security() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let (course_id, _, _) = setup_course(&pool, &admin, branch, false, 10).await;
    let c = ec_core::education::courses::get_course(&pool, &admin, course_id).await.unwrap();
    let sess = create_session(&pool, &admin, course_id, "2026-04-05", "09:00", "10:00",
        c.room_id, c.teacher_id, None, 1, None).await.unwrap();

    // actor without attendance.record permission
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, full_name, password_hash) VALUES ('norole','x','x') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let ps = ec_core::auth::permissions::PermissionSet {
        user_id: uid, roles: vec![], permissions: std::collections::HashSet::new(),
    };
    let actor = Actor { user_id: uid, username: "norole".into(), permissions: ps };
    let r = record_student_attendance(&pool, &actor, sess.id, 1, "present", 0, "").await;
    assert!(r.is_err(), "unauthorized attendance must be denied");
    assert!(r.unwrap_err().to_string().contains("صلاحية"));
}

/// agreements: draft -> approve (maker-checker), snapshot frozen.
#[tokio::test]
async fn edu_agreement_snapshot_immutable() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let (course_id, _, _) = setup_course(&pool, &admin, branch, false, 10).await;
    let draft = create_agreement_draft(&pool, &admin, course_id, "flat_course", None, None,
        Some(5000), None, "{}", "{}").await.unwrap();
    assert_eq!(draft.status, "draft");
    // self-approval blocked without override
    let err = approve_agreement(&pool, &admin, draft.id, admin.user_id, None).await;
    assert!(err.is_err(), "self-approval without override must be blocked");
    // with override reason allowed
    let approved = approve_agreement(&pool, &admin, draft.id, admin.user_id, Some("override")).await.unwrap();
    assert_eq!(approved.status, "active");
    // terms immutable now
    let err2 = sqlx::query("UPDATE financial_agreements SET flat_minor=9999 WHERE id=?")
        .bind(approved.id).execute(&pool).await;
    assert!(err2.is_err(), "active agreement terms must be immutable");
}

/// edu014: course_change_log written on status change.
#[tokio::test]
async fn edu014_change_log_written() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    // reuse setup from edu011? Minimal: create course without agreement then change status draft->registration_open
    let subject = ec_core::people::teachers::create_subject(&pool, &admin, "M", "رياضيات", "").await.unwrap();
    let teacher = ec_core::people::teachers::create_teacher(&pool, &admin, branch, &ec_core::people::teachers::NewTeacher { full_name: "م".into(), ..Default::default() }).await.unwrap();
    let room = ec_core::people::teachers::create_room(&pool, &admin, branch, "R", "ق", 10, "").await.unwrap();
    let course = ec_core::education::courses::create_course(&pool, &admin, branch, &ec_core::education::courses::NewCourse {
        name: "د".into(), subject_id: subject.id, teacher_id: teacher.id, room_id: room.id,
        fee_minor: 10000, ..Default::default()
    }).await.unwrap();
    ec_core::education::courses::set_course_status(&pool, &admin, course.id, "registration_open", "open").await.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM course_change_log WHERE course_id = ?")
        .bind(course.id).fetch_one(&pool).await.unwrap();
    assert!(n >= 1);
}

/// edu015: attendance weighting helper.
#[tokio::test]
async fn edu015_attendance_weighting() {
    assert!(ec_core::education::attendance::cancelled_sessions_count("{}"));
    assert!(!ec_core::education::attendance::cancelled_sessions_count(r#"{"cancelled_effects":{"count_for_teacher":false}}"#));
    assert!(ec_core::education::attendance::makeup_weight_inherited(Some(1)));
    assert!(!ec_core::education::attendance::makeup_weight_inherited(None));
}

/// Phase 17: every enrollment issues a draft receipt voucher + (when the
/// active agreement gives the teacher a share) a draft payment voucher.
#[tokio::test]
async fn edu_enrollment_creates_vouchers() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let (course, teacher, _room) = setup_course(&pool, &admin, branch, false, 30).await;

    // active agreement: teacher gets 50% of the agreed fee
    let agr = create_agreement_draft(&pool, &admin, course, "percent_of_agreed", Some(5000), None, None, None, "{}", "{}")
        .await.unwrap();
    approve_agreement(&pool, &admin, agr.id, admin.user_id, Some("test override")).await.unwrap();

    let st = create_student(&pool, &admin, branch, &ec_core::people::students::NewStudent {
        full_name: "طالب سند".into(), ..Default::default()
    }).await.unwrap();
    let en = enroll(&pool, &admin, branch, st.id, course, 10000, 0, 0, "2026-01-01", "")
        .await.unwrap();

    let rv: Option<(i64, String, i64)> = sqlx::query_as(
        "SELECT id, status, amount_minor FROM receipt_vouchers WHERE enrollment_id=?",
    ).bind(en.id).fetch_optional(&pool).await.unwrap();
    let rv = rv.expect("receipt voucher must exist");
    assert_eq!(rv.1, "draft");
    assert_eq!(rv.2, 10000);

    let pv: Option<(i64, String, i64, i64)> = sqlx::query_as(
        "SELECT id, status, amount_minor, teacher_id FROM payment_vouchers WHERE source_type='teacher_entitlement' AND source_id=?",
    ).bind(en.id).fetch_optional(&pool).await.unwrap();
    let pv = pv.expect("payment voucher must exist");
    assert_eq!(pv.1, "draft");
    assert_eq!(pv.2, 5000, "teacher gets 50% of 10000");
    assert_eq!(pv.3, teacher);
}

// Phase 18: weekly schedule rules, clash detection, session generation.
#[tokio::test]
async fn sched001_rule_clash_and_generation() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let (course, teacher, room) = setup_course(&pool, &admin, branch, false, 30).await;
    let subject: i64 = sqlx::query_scalar("SELECT subject_id FROM courses WHERE id=?").bind(course).fetch_one(&pool).await.unwrap();
    let room2 = create_room(&pool, &admin, branch, "R2", "ق2", 30, "").await.unwrap();

    // add Monday 10:00-11:00 rule
    let rule = ec_core::education::schedule::add_rule(&pool, &admin, course, 1, "10:00", "11:00", room, teacher, "", "")
        .await.unwrap();
    assert_eq!(rule.weekday, 1);

    // same teacher + overlapping time on another course → clash
    let c2 = create_course(&pool, &admin, branch, &NewCourse {
        name: "دورة ثانية".into(), subject_id: subject, teacher_id: teacher, room_id: room2.id,
        fee_minor: 5000, ..Default::default()
    }).await.unwrap();
    let err = ec_core::education::schedule::add_rule(&pool, &admin, c2.id, 1, "10:30", "11:30", room2.id, teacher, "", "")
        .await.unwrap_err();
    assert!(format!("{:?}", err).contains("مشغول") || format!("{:?}", err).contains("محجوز"), "{err:?}");

    // non-overlapping rule is fine
    let rule2 = ec_core::education::schedule::add_rule(&pool, &admin, c2.id, 1, "12:00", "13:00", room2.id, teacher, "", "")
        .await.unwrap();
    assert_eq!(rule2.id > rule.id, true);

    // generate sessions for course 1 over 2026-11-02 (Monday) .. 2026-11-08
    let preview = ec_core::education::schedule::generate_sessions(&pool, &admin, course, "2026-11-02", "2026-11-08", false)
        .await.unwrap();
    assert_eq!(preview.len(), 1, "one Monday in range");
    assert_eq!(preview[0].session_date, "2026-11-02");
    let count_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE course_id=?").bind(course).fetch_one(&pool).await.unwrap();
    assert_eq!(count_before, 0);
    let gen = ec_core::education::schedule::generate_sessions(&pool, &admin, course, "2026-11-02", "2026-11-08", true)
        .await.unwrap();
    assert_eq!(gen.len(), 1);
    let count_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE course_id=?").bind(course).fetch_one(&pool).await.unwrap();
    assert_eq!(count_after, 1);
    let src: i64 = sqlx::query_scalar("SELECT source_schedule_id FROM sessions WHERE course_id=?").bind(course).fetch_one(&pool).await.unwrap();
    assert_eq!(src, rule.id);
    // idempotent: second run creates nothing
    let gen2 = ec_core::education::schedule::generate_sessions(&pool, &admin, course, "2026-11-02", "2026-11-08", true)
        .await.unwrap();
    assert_eq!(gen2.len(), 0);
}

#[tokio::test]
async fn sched002_activation_with_percent_agreement() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let (course, teacher, _room) = setup_course(&pool, &admin, branch, false, 30).await;
    let _ = teacher;
    // activation requires an active agreement — the auto-created one uses
    // "percent_of_agreed" (must be in METHODS or create_agreement_draft fails)
    let agr = create_agreement_draft(&pool, &admin, course, "percent_of_agreed", Some(10000), Some(10000), None, None, "{}", "{}")
        .await.expect("percent_of_agreed is a valid method");
    approve_agreement(&pool, &admin, agr.id, admin.user_id, Some("test override")).await.unwrap();
    // setup_course already moved it to registration_open
    let c = set_course_status(&pool, &admin, course, "active", "").await.unwrap();
    assert_eq!(c.status, "active");
}
