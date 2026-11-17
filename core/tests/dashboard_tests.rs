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

/// dash002: full KPIs.
#[tokio::test]
async fn dash002_kpis_full() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    let k = ec_core::dashboard::compute_kpis(&pool).await.unwrap();
    // new fields present (zero on empty db)
    assert_eq!(k.teacher_payables_minor, 0);
    assert_eq!(k.due_fees_minor, 0);
    assert_eq!(k.courses_needing_settlement, 0);
}

/// dash003: KPIs reconcile to ledger (cash balance equals sum of movements).
#[tokio::test]
async fn dash003_kpis_reconcile_to_ledger() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    let k = ec_core::dashboard::compute_kpis(&pool).await.unwrap();
    let mov: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(CASE WHEN direction='in' THEN amount_minor ELSE -amount_minor END),0) FROM cash_movements").fetch_one(&pool).await.unwrap();
    assert_eq!(k.cash_balance_minor, mov);
    assert!(ec_core::finance::ledger::ledger_balanced(&pool).await.unwrap());
}

/// rpt003: report registry has >= 40 reports.
#[tokio::test]
async fn rpt003_report_registry_count() {
    assert!(ec_core::reports::report_count() >= 40);
    assert!(ec_core::reports::report_codes().len() >= 40);
}

/// rpt004: each report runs.
#[tokio::test]
async fn rpt004_each_report_runs() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    for code in ec_core::reports::report_codes() {
        let r = ec_core::reports::run_report(&pool, code).await;
        assert!(r.is_ok(), "report {} failed: {:?}", code, r);
    }
}

/// rpt005: saved filters and favorites.
#[tokio::test]
async fn rpt005_saved_filters_and_favorites() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    let id = ec_core::reports::save_report_with_filters(&pool, "fav", "{}", "{\"x\":1}", true, true).await.unwrap();
    assert!(id > 0);
    ec_core::reports::toggle_favorite(&pool, id, false).await.unwrap();
    let list = ec_core::reports::list_reports(&pool).await.unwrap();
    assert_eq!(list.len(), 1);
    assert!(!list[0].is_favorite);
}

/// rpt007: every registered report executes and returns well-formed rows whose
/// keys exactly match the registry columns (the UI renders straight from these).
#[tokio::test]
async fn rpt007_each_report_executes_with_rows() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    for def in ec_core::reports::list_report_definitions() {
        let res = ec_core::reports::execute_report(&pool, def.code)
            .await
            .unwrap_or_else(|e| panic!("report {} failed: {e:?}", def.code));
        assert_eq!(res.code, def.code);
        assert_eq!(res.columns, def.columns, "columns mismatch for {}", def.code);
        assert_eq!(res.row_count, res.rows.len() as i64);
        for row in &res.rows {
            let obj = row.as_object().unwrap_or_else(|| {
                panic!("report {} returned a non-object row: {row}", def.code)
            });
            for col in def.columns {
                assert!(
                    obj.contains_key(*col),
                    "report {} row is missing column {col}: {row}",
                    def.code
                );
            }
            assert_eq!(
                obj.len(),
                def.columns.len(),
                "report {} returned extra columns: {row}",
                def.code
            );
        }
    }
}

/// rpt008: a report code outside the registry is rejected (no raw SQL path).
#[tokio::test]
async fn rpt008_unknown_report_code_rejected() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    assert!(ec_core::reports::execute_report(&pool, "rpt_1=1").await.is_err());
    assert!(ec_core::reports::execute_report(&pool, "students; DROP TABLE users").await.is_err());
    assert!(ec_core::reports::run_report(&pool, "nope").await.is_err());
}

// ---------- Phase 16: attendance sheet + payment-voucher listing ----------

/// att001: the attendance sheet lists exactly the enrolled students of the
/// session's course, with `status = null` until something is recorded.
#[tokio::test]
async fn att001_sheet_lists_enrolled_students() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let actor = admin_actor(&pool).await;
    let uid = actor.user_id;

    // minimal education scaffolding: subject, teacher, room, course, session
    let subject: i64 = sqlx::query_scalar(
        "INSERT INTO subjects (code, name) VALUES ('S1', 'مادة') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let teacher: i64 = sqlx::query_scalar(
        "INSERT INTO teachers (branch_id, teacher_no, full_name) VALUES (?, 1, 'مدرس') RETURNING id",
    ).bind(branch).fetch_one(&pool).await.unwrap();
    let room: i64 = sqlx::query_scalar(
        "INSERT INTO rooms (branch_id, code, name, capacity) VALUES (?, 'R1', 'قاعة', 20) RETURNING id",
    ).bind(branch).fetch_one(&pool).await.unwrap();
    let course: i64 = sqlx::query_scalar(
        "INSERT INTO courses (branch_id, course_no, name, subject_id, teacher_id, room_id, created_by)
         VALUES (?, 1, 'دورة تجريبية', ?, ?, ?, ?) RETURNING id",
    ).bind(branch).bind(subject).bind(teacher).bind(room).bind(uid)
        .fetch_one(&pool).await.unwrap();
    let sess: i64 = sqlx::query_scalar(
        "INSERT INTO sessions (course_id, seq_in_course, session_date, start_time, end_time,
                               room_id, teacher_id, created_by)
         VALUES (?, 1, '2026-11-15', '10:00', '11:00', ?, ?, ?) RETURNING id",
    ).bind(course).bind(room).bind(teacher).bind(uid)
        .fetch_one(&pool).await.unwrap();

    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (branch_id, student_no, full_name) VALUES (?, 1, 'طالب أ') RETURNING id",
    ).bind(branch).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO students (branch_id, student_no, full_name) VALUES (?, 2, 'طالب ب')")
        .bind(branch).execute(&pool).await.unwrap();

    // only student 1 is enrolled — student 2 must not appear on the sheet
    sqlx::query(
        "INSERT INTO enrollments (branch_id, student_id, course_id, created_by,
                                  enrolled_date, base_fee_minor, net_fee_minor, status)
         VALUES (?, ?, ?, ?, '2026-11-01', 100000, 100000, 'active')",
    ).bind(branch).bind(sid).bind(course).bind(uid).execute(&pool).await.unwrap();

    let sheet = ec_core::education::attendance::attendance_sheet(&pool, &actor, sess)
        .await
        .unwrap();
    assert_eq!(sheet.len(), 1, "only enrolled students belong on the sheet");
    assert_eq!(sheet[0].student_id, sid);
    assert_eq!(sheet[0].full_name, "طالب أ");
    assert!(sheet[0].status.is_none(), "nothing recorded yet");

    // record one mark and the sheet reflects it
    ec_core::education::attendance::record_student_attendance(
        &pool, &actor, sess, sid, "late", 12, "",
    ).await.unwrap();
    let sheet2 = ec_core::education::attendance::attendance_sheet(&pool, &actor, sess)
        .await
        .unwrap();
    assert_eq!(sheet2[0].status.as_deref(), Some("late"));
    assert_eq!(sheet2[0].late_minutes, Some(12));
}

/// att002: the sheet requires the attendance permission.
#[tokio::test]
async fn att002_sheet_requires_permission() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, full_name, password_hash) VALUES ('viewer','V','x') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    // read_only has no `attendance.record` permission
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, (SELECT id FROM roles WHERE code='read_only'))")
        .bind(uid).execute(&pool).await.unwrap();
    let ps = load_for_user(&pool, uid).await.unwrap();
    assert!(!ps.can("attendance.record"));
    let actor = Actor { user_id: uid, username: "viewer".into(), permissions: ps };
    assert!(ec_core::education::attendance::attendance_sheet(&pool, &actor, 1).await.is_err());
}

/// att003: payment vouchers are listable and keep money as integers.
#[tokio::test]
async fn att003_payment_vouchers_listable() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let actor = admin_actor(&pool).await;

    let before = ec_core::vouchers::payment_vouchers::list_payment_vouchers(&pool, &actor, None, 100)
        .await
        .unwrap();
    assert!(before.is_empty());

    let acct: i64 = sqlx::query_scalar("SELECT id FROM accounts WHERE is_postable = 1 LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    ec_core::vouchers::payment_vouchers::create_payment_voucher(
        &pool, &actor, branch,     // branch_id
        "vendor",                   // beneficiary_type
        None,                       // teacher_id
        "مورّد الاختبار",            // purpose -> also stored as beneficiary_name
        25_000,                     // amount_minor
        "cash",                     // method
        acct,                       // account_id
        "other",                    // source_type
        None,                       // source_id
        "att003-key",               // idempotency_key
    ).await.unwrap();

    let after = ec_core::vouchers::payment_vouchers::list_payment_vouchers(&pool, &actor, None, 100)
        .await
        .unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].amount_minor, 25_000, "money must stay an integer");
    assert_eq!(after[0].beneficiary_name, "مورّد الاختبار");
}

// ---------- Phase 17: detail views + receipt linking ----------

#[tokio::test]
async fn det001_student_detail_shows_enrollments_and_balance() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let actor = admin_actor(&pool).await;
    let uid = actor.user_id;
    let sid: i64 = sqlx::query_scalar("INSERT INTO students (branch_id, student_no, full_name) VALUES (?,1,'طالب تفاصيل') RETURNING id")
        .bind(branch).fetch_one(&pool).await.unwrap();
    // minimal course
    let subj: i64 = sqlx::query_scalar("INSERT INTO subjects (code,name) VALUES ('D1','مادة') RETURNING id").fetch_one(&pool).await.unwrap();
    let tea: i64 = sqlx::query_scalar("INSERT INTO teachers (branch_id,teacher_no,full_name) VALUES (?,1,'مدرس') RETURNING id").bind(branch).fetch_one(&pool).await.unwrap();
    let room: i64 = sqlx::query_scalar("INSERT INTO rooms (branch_id,code,name,capacity) VALUES (?, 'DR','قاعة',20) RETURNING id").bind(branch).fetch_one(&pool).await.unwrap();
    let cid: i64 = sqlx::query_scalar("INSERT INTO courses (branch_id,course_no,name,subject_id,teacher_id,room_id,created_by) VALUES (?,1,'دورة تفاصيل',?,?,?,?) RETURNING id")
        .bind(branch).bind(subj).bind(tea).bind(room).bind(uid).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO enrollments (branch_id,student_id,course_id,created_by,enrolled_date,base_fee_minor,discount_minor,waiver_minor,net_fee_minor,status) VALUES (?,?,?,?,'2026-01-01',100000,10000,0,90000,'active')")
        .bind(branch).bind(sid).bind(cid).bind(uid).execute(&pool).await.unwrap();
    let d = ec_core::details::student_detail(&pool, &actor, sid).await.unwrap();
    assert_eq!(d.enrollments.len(), 1);
    assert_eq!(d.enrollments[0].course_name, "دورة تفاصيل");
    assert_eq!(d.total_fee_minor, 90000);
    assert_eq!(d.total_remaining_minor, 90000);
}

#[tokio::test]
async fn det002_course_detail_shows_students_and_sessions() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let actor = admin_actor(&pool).await;
    let uid = actor.user_id;
    let subj: i64 = sqlx::query_scalar("INSERT INTO subjects (code,name) VALUES ('D2','مادة') RETURNING id").fetch_one(&pool).await.unwrap();
    let tea: i64 = sqlx::query_scalar("INSERT INTO teachers (branch_id,teacher_no,full_name) VALUES (?,1,'مدرس') RETURNING id").bind(branch).fetch_one(&pool).await.unwrap();
    let room: i64 = sqlx::query_scalar("INSERT INTO rooms (branch_id,code,name,capacity) VALUES (?, 'DR2','قاعة',20) RETURNING id").bind(branch).fetch_one(&pool).await.unwrap();
    let cid: i64 = sqlx::query_scalar("INSERT INTO courses (branch_id,course_no,name,subject_id,teacher_id,room_id,created_by,end_date) VALUES (?,1,'دورة',?,?,?,?,'2026-12-31') RETURNING id")
        .bind(branch).bind(subj).bind(tea).bind(room).bind(uid).fetch_one(&pool).await.unwrap();
    let d = ec_core::details::course_detail(&pool, &actor, cid).await.unwrap();
    assert_eq!(d.course_name, "دورة");
    assert_eq!(d.teacher_name, "مدرس");
    assert_eq!(d.subject_name, "مادة");
    assert_eq!(d.room_name, "قاعة");
}

#[tokio::test]
async fn det003_receipt_links_student_and_course() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let actor = admin_actor(&pool).await;
    let uid = actor.user_id;
    let sid: i64 = sqlx::query_scalar("INSERT INTO students (branch_id,student_no,full_name) VALUES (?,1,'طالب قبض') RETURNING id").bind(branch).fetch_one(&pool).await.unwrap();
    let subj: i64 = sqlx::query_scalar("INSERT INTO subjects (code,name) VALUES ('D3','مادة') RETURNING id").fetch_one(&pool).await.unwrap();
    let tea: i64 = sqlx::query_scalar("INSERT INTO teachers (branch_id,teacher_no,full_name) VALUES (?,1,'مدرس') RETURNING id").bind(branch).fetch_one(&pool).await.unwrap();
    let room: i64 = sqlx::query_scalar("INSERT INTO rooms (branch_id,code,name,capacity) VALUES (?, 'DR3','قاعة',20) RETURNING id").bind(branch).fetch_one(&pool).await.unwrap();
    let cid: i64 = sqlx::query_scalar("INSERT INTO courses (branch_id,course_no,name,subject_id,teacher_id,room_id,created_by) VALUES (?,1,'دورة قبض',?,?,?,?) RETURNING id")
        .bind(branch).bind(subj).bind(tea).bind(room).bind(uid).fetch_one(&pool).await.unwrap();
    let eid: i64 = sqlx::query_scalar("INSERT INTO enrollments (branch_id,student_id,course_id,created_by,enrolled_date,base_fee_minor,discount_minor,waiver_minor,net_fee_minor,status) VALUES (?,?,?,?,'2026-01-01',100000,0,0,100000,'active') RETURNING id")
        .bind(branch).bind(sid).bind(cid).bind(uid).fetch_one(&pool).await.unwrap();
    let cash: i64 = sqlx::query_scalar("SELECT id FROM accounts WHERE code='1000'").fetch_one(&pool).await.unwrap();
    let recv: i64 = sqlx::query_scalar("SELECT id FROM accounts WHERE code='1300'").fetch_one(&pool).await.unwrap();
    ec_core::finance::payments::record_payment(&pool, &actor, branch, cash, recv, "det003-key",
        &[ec_core::finance::payments::PaymentAllocation { enrollment_id: eid, amount_minor: 50000 }], "دفعة").await.unwrap();
    let (st, co): (Option<i64>, Option<i64>) = sqlx::query_as("SELECT student_id, course_id FROM receipt_vouchers WHERE idempotency_key='det003-key'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(st, Some(sid), "receipt must be linked to the student");
    assert_eq!(co, Some(cid), "receipt must be linked to the course");
    let d = ec_core::details::student_detail(&pool, &actor, sid).await.unwrap();
    assert_eq!(d.receipts.len(), 1);
    assert_eq!(d.receipts[0].amount_minor, 50000);
    assert_eq!(d.enrollments[0].paid_minor, 50000);
}
