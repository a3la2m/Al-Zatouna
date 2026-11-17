//! Detail views (Phase 17): aggregate every screen a manager needs for one
//! student / teacher / course / room. All money is integer minor units.
//! Every function requires a view permission and returns real rows only.

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::people::context::Actor;
use crate::AppResult;

// ---------------------------------------------------------------- students

#[derive(Debug, Serialize, Clone)]
pub struct StudentEnrollmentDetail {
    pub enrollment_id: i64,
    pub course_id: i64,
    pub course_name: String,
    pub status: String,
    pub base_fee_minor: i64,
    pub discount_minor: i64,
    pub waiver_minor: i64,
    pub net_fee_minor: i64,
    pub paid_minor: i64,
    pub remaining_minor: i64,
    pub enrolled_date: String,
    pub installment_count: i64,
    pub installments_open: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct StudentLedgerRow {
    pub id: i64,
    pub ref_type: String,
    pub credit_minor: i64,
    pub occurred_at: String,
    pub note: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct StudentReceiptRow {
    pub id: i64,
    pub voucher_no: i64,
    pub voucher_date: String,
    pub amount_minor: i64,
    pub course_name: String,
    pub method: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct StudentDetail {
    pub student_id: i64,
    pub full_name: String,
    pub student_no: i64,
    pub phone: String,
    pub enrollments: Vec<StudentEnrollmentDetail>,
    pub credit_ledger: Vec<StudentLedgerRow>,
    pub receipts: Vec<StudentReceiptRow>,
    pub attendance_total: i64,
    pub attendance_present: i64,
    pub total_fee_minor: i64,
    pub total_paid_minor: i64,
    pub total_remaining_minor: i64,
    pub credit_balance_minor: i64,
}

pub async fn student_detail(
    pool: &SqlitePool,
    actor: &Actor,
    student_id: i64,
) -> AppResult<StudentDetail> {
    actor.require("students.view")?;
    let student: (String, i64, String) = sqlx::query_as(
        "SELECT full_name, student_no, phone FROM students WHERE id = ?",
    )
    .bind(student_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("الطالب غير موجود"))?;

    let rows = sqlx::query(
        "SELECT e.id AS enrollment_id, e.course_id, c.name AS course_name, e.status,
                e.base_fee_minor, e.discount_minor, e.waiver_minor, e.net_fee_minor,
                e.enrolled_date,
                COALESCE((SELECT SUM(pa.amount_minor) FROM payment_applications pa
                          WHERE pa.enrollment_id = e.id), 0) AS paid_minor,
                COALESCE((SELECT COUNT(*) FROM payment_plans pp
                          WHERE pp.enrollment_id = e.id), 0) AS installment_count,
                COALESCE((SELECT COUNT(*) FROM payment_plans pp JOIN installments i
                          ON i.payment_plan_id = pp.id
                          WHERE pp.enrollment_id = e.id AND i.status <> 'paid'), 0) AS installments_open
         FROM enrollments e JOIN courses c ON c.id = e.course_id
         WHERE e.student_id = ? ORDER BY e.id DESC",
    )
    .bind(student_id)
    .fetch_all(pool)
    .await?;
    let enrollments: Vec<StudentEnrollmentDetail> = rows
        .iter()
        .map(|r| StudentEnrollmentDetail {
            enrollment_id: r.get("enrollment_id"),
            course_id: r.get("course_id"),
            course_name: r.get("course_name"),
            status: r.get("status"),
            base_fee_minor: r.get("base_fee_minor"),
            discount_minor: r.get("discount_minor"),
            waiver_minor: r.get("waiver_minor"),
            net_fee_minor: r.get("net_fee_minor"),
            paid_minor: r.get("paid_minor"),
            remaining_minor: r.get::<i64, _>("net_fee_minor") - r.get::<i64, _>("paid_minor"),
            enrolled_date: r.get("enrolled_date"),
            installment_count: r.get("installment_count"),
            installments_open: r.get("installments_open"),
        })
        .collect();

    let ledger = sqlx::query(
        "SELECT id, ref_type, credit_minor,
                COALESCE(occurred_at,'') AS occurred_at, COALESCE(note,'') AS note
         FROM student_credit_ledger WHERE student_id = ? ORDER BY id DESC LIMIT 100",
    )
    .bind(student_id)
    .fetch_all(pool)
    .await?;
    let credit_ledger: Vec<StudentLedgerRow> = ledger
        .iter()
        .map(|r| StudentLedgerRow {
            id: r.get("id"),
            ref_type: r.get("ref_type"),
            credit_minor: r.get("credit_minor"),
            occurred_at: r.get("occurred_at"),
            note: r.get("note"),
        })
        .collect();
    let credit_balance_minor: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(credit_minor),0) FROM student_credit_ledger WHERE student_id = ?",
    )
    .bind(student_id)
    .fetch_one(pool)
    .await?;

    let receipts = sqlx::query(
        "SELECT rv.id, rv.voucher_no, rv.voucher_date, rv.amount_minor, rv.method,
                COALESCE(c.name,'') AS course_name
         FROM receipt_vouchers rv LEFT JOIN courses c ON c.id = rv.course_id
         WHERE rv.student_id = ? AND rv.status='posted' ORDER BY rv.id DESC LIMIT 100",
    )
    .bind(student_id)
    .fetch_all(pool)
    .await?;
    let receipts: Vec<StudentReceiptRow> = receipts
        .iter()
        .map(|r| StudentReceiptRow {
            id: r.get("id"),
            voucher_no: r.get("voucher_no"),
            voucher_date: r.get("voucher_date"),
            amount_minor: r.get("amount_minor"),
            course_name: r.get("course_name"),
            method: r.get("method"),
        })
        .collect();

    let (att_total, att_present): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN status IN ('present','late') THEN 1 ELSE 0 END),0)
         FROM student_attendance WHERE student_id = ?",
    )
    .bind(student_id)
    .fetch_one(pool)
    .await?;

    let total_fee_minor: i64 = enrollments.iter().map(|e| e.net_fee_minor).sum();
    let total_paid_minor: i64 = enrollments.iter().map(|e| e.paid_minor).sum();
    Ok(StudentDetail {
        student_id,
        full_name: student.0,
        student_no: student.1,
        phone: student.2,
        total_remaining_minor: total_fee_minor - total_paid_minor,
        attendance_total: att_total,
        attendance_present: att_present,
        enrollments,
        credit_ledger,
        receipts,
        total_fee_minor,
        total_paid_minor,
        credit_balance_minor,
    })
}

// ---------------------------------------------------------------- teachers

#[derive(Debug, Serialize, Clone)]
pub struct TeacherCourseRow {
    pub course_id: i64,
    pub course_name: String,
    pub subject_name: String,
    pub status: String,
    pub start_date: String,
    pub fee_minor: i64,
    pub students_count: i64,
    pub sessions_count: i64,
    pub hours_done: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct TeacherPaymentRow {
    pub id: i64,
    pub voucher_no: i64,
    pub voucher_date: String,
    pub amount_minor: i64,
    pub purpose: String,
    pub status: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct TeacherDetail {
    pub teacher_id: i64,
    pub full_name: String,
    pub specialty: String,
    pub phone: String,
    pub courses: Vec<TeacherCourseRow>,
    pub subjects: Vec<String>,
    pub payments: Vec<TeacherPaymentRow>,
    pub payable_minor: i64,
    pub paid_minor: i64,
    pub remaining_minor: i64,
    pub sessions_taught: i64,
    pub hours_taught: f64,
    pub attendance_records: i64,
    pub teacher_account_balance_minor: i64,
}

pub async fn teacher_detail(
    pool: &SqlitePool,
    actor: &Actor,
    teacher_id: i64,
) -> AppResult<TeacherDetail> {
    actor.require("teachers.view")?;
    let teacher: (String, String, String) = sqlx::query_as(
        "SELECT full_name, specialty, phone FROM teachers WHERE id = ?",
    )
    .bind(teacher_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("المدرس غير موجود"))?;

    let rows = sqlx::query(
        "SELECT c.id AS course_id, c.name AS course_name,
                COALESCE(s.name,'') AS subject_name, c.status, c.start_date, c.fee_minor,
                (SELECT COUNT(*) FROM enrollments e WHERE e.course_id=c.id AND e.status IN ('active','frozen')) AS students_count,
                (SELECT COUNT(*) FROM sessions se WHERE se.course_id=c.id AND se.status='executed') AS sessions_count,
                COALESCE((SELECT SUM((julianday(se.end_time)-julianday(se.start_time))*24)
                          FROM sessions se WHERE se.course_id=c.id AND se.status='executed' AND se.teacher_id=?2),0.0) AS hours_done
         FROM courses c
         LEFT JOIN subjects s ON s.id=c.subject_id
         WHERE c.teacher_id=?2 OR EXISTS (
            SELECT 1 FROM course_schedules cs WHERE cs.course_id=c.id AND cs.teacher_id=?2
         )
         ORDER BY c.id DESC",
    )
    .bind(teacher_id)
    .fetch_all(pool)
    .await?;
    let courses: Vec<TeacherCourseRow> = rows
        .iter()
        .map(|r| TeacherCourseRow {
            course_id: r.get("course_id"),
            course_name: r.get("course_name"),
            subject_name: r.get("subject_name"),
            status: r.get("status"),
            start_date: r.get("start_date"),
            fee_minor: r.get("fee_minor"),
            students_count: r.get("students_count"),
            sessions_count: r.get("sessions_count"),
            hours_done: r.get::<f64, _>("hours_done"),
        })
        .collect();

    let mut subjects: Vec<String> = courses
        .iter()
        .filter(|c| !c.subject_name.is_empty())
        .map(|c| c.subject_name.clone())
        .collect();
    subjects.sort();
    subjects.dedup();

    let pays = sqlx::query(
        "SELECT id, voucher_no, voucher_date, amount_minor, purpose, status
         FROM payment_vouchers WHERE teacher_id = ? ORDER BY id DESC LIMIT 100",
    )
    .bind(teacher_id)
    .fetch_all(pool)
    .await?;
    let payments: Vec<TeacherPaymentRow> = pays
        .iter()
        .map(|r| TeacherPaymentRow {
            id: r.get("id"),
            voucher_no: r.get("voucher_no"),
            voucher_date: r.get("voucher_date"),
            amount_minor: r.get("amount_minor"),
            purpose: r.get("purpose"),
            status: r.get("status"),
        })
        .collect();

    let paid_minor: i64 = payments
        .iter()
        .filter(|p| p.status == "paid")
        .map(|p| p.amount_minor)
        .sum();
    let payable_minor: i64 = payments.iter().map(|p| p.amount_minor).sum();

    // teacher account balance from the ledger (1400 = مستحقات المدرسين)
    let teacher_account: Option<i64> =
        sqlx::query_scalar("SELECT id FROM accounts WHERE code='1400'")
            .fetch_optional(pool)
            .await?;
    let account_balance = match teacher_account {
        Some(id) => crate::finance::ledger::account_balance(pool, id).await.unwrap_or(0),
        None => 0,
    };

    let (sessions_taught, hours_taught): (i64, f64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM((julianday(end_time)-julianday(start_time))*24),0.0)
         FROM sessions WHERE teacher_id=? AND status='executed'",
    )
    .bind(teacher_id)
    .fetch_one(pool)
    .await?;
    let attendance_records: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM teacher_attendance WHERE teacher_id = ?",
    )
    .bind(teacher_id)
    .fetch_one(pool)
    .await?;

    Ok(TeacherDetail {
        teacher_id,
        full_name: teacher.0,
        specialty: teacher.1,
        phone: teacher.2,
        courses,
        subjects,
        payments,
        payable_minor,
        paid_minor,
        remaining_minor: payable_minor - paid_minor,
        sessions_taught,
        hours_taught,
        attendance_records,
        teacher_account_balance_minor: account_balance,
    })
}

// ---------------------------------------------------------------- courses

#[derive(Debug, Serialize, Clone)]
pub struct CourseStudentRow {
    pub enrollment_id: i64,
    pub student_id: i64,
    pub student_name: String,
    pub student_no: i64,
    pub net_fee_minor: i64,
    pub paid_minor: i64,
    pub remaining_minor: i64,
    pub status: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct CourseSessionRow {
    pub id: i64,
    pub seq: i64,
    pub session_date: String,
    pub start_time: String,
    pub end_time: String,
    pub status: String,
    pub room_name: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct CourseDetail {
    pub course_id: i64,
    pub course_name: String,
    pub status: String,
    pub teacher_name: String,
    pub subject_name: String,
    pub room_name: String,
    pub fee_minor: i64,
    pub start_date: String,
    pub end_date: String,
    pub students: Vec<CourseStudentRow>,
    pub sessions: Vec<CourseSessionRow>,
    pub schedule: Vec<CourseSessionRow>,
    pub collected_minor: i64,
    pub outstanding_minor: i64,
    pub sessions_total: i64,
    pub sessions_done: i64,
    pub hours_planned: f64,
    pub hours_done: f64,
    pub days_remaining: i64,
    pub agreement_status: String,
}

pub async fn course_detail(
    pool: &SqlitePool,
    actor: &Actor,
    course_id: i64,
) -> AppResult<CourseDetail> {
    actor.require("courses.view")?;
    let c: (String, String, String, String, String, i64, String, String, i64, i64) = sqlx::query_as(
        "SELECT c.name, c.status,
                COALESCE(t.full_name,'') AS teacher_name,
                COALESCE(s.name,'') AS subject_name,
                COALESCE(r.name,'') AS room_name,
                c.fee_minor, c.start_date, c.end_date,
                COALESCE(c.max_students,0) AS max_students, c.is_private
         FROM courses c
         LEFT JOIN teachers t ON t.id=c.teacher_id
         LEFT JOIN subjects s ON s.id=c.subject_id
         LEFT JOIN rooms r ON r.id=c.room_id
         WHERE c.id = ?",
    )
    .bind(course_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("الدورة غير موجودة"))?;

    let srows = sqlx::query(
        "SELECT e.id AS enrollment_id, e.student_id, st.full_name AS student_name,
                st.student_no, e.net_fee_minor, e.status,
                COALESCE((SELECT SUM(pa.amount_minor) FROM payment_applications pa
                          WHERE pa.enrollment_id=e.id),0) AS paid_minor
         FROM enrollments e JOIN students st ON st.id=e.student_id
         WHERE e.course_id=? ORDER BY st.student_no",
    )
    .bind(course_id)
    .fetch_all(pool)
    .await?;
    let students: Vec<CourseStudentRow> = srows
        .iter()
        .map(|r| CourseStudentRow {
            enrollment_id: r.get("enrollment_id"),
            student_id: r.get("student_id"),
            student_name: r.get("student_name"),
            student_no: r.get("student_no"),
            net_fee_minor: r.get("net_fee_minor"),
            paid_minor: r.get("paid_minor"),
            remaining_minor: r.get::<i64, _>("net_fee_minor") - r.get::<i64, _>("paid_minor"),
            status: r.get("status"),
        })
        .collect();

    let sess = sqlx::query(
        "SELECT se.id, se.seq_in_course AS seq, se.session_date, se.start_time, se.end_time,
                se.status, COALESCE(r.name,'') AS room_name
         FROM sessions se LEFT JOIN rooms r ON r.id=se.room_id
         WHERE se.course_id=? ORDER BY se.session_date, se.seq_in_course",
    )
    .bind(course_id)
    .fetch_all(pool)
    .await?;
    let sessions: Vec<CourseSessionRow> = sess
        .iter()
        .map(|r| CourseSessionRow {
            id: r.get("id"),
            seq: r.get("seq"),
            session_date: r.get("session_date"),
            start_time: r.get("start_time"),
            end_time: r.get("end_time"),
            status: r.get("status"),
            room_name: r.get("room_name"),
        })
        .collect();

    let sched = sqlx::query(
        "SELECT cs.id, cs.weekday AS seq, '' AS session_date, cs.start_time, cs.end_time,
                COALESCE(r.name,'') AS room_name
         FROM course_schedules cs LEFT JOIN rooms r ON r.id=cs.room_id
         WHERE cs.course_id=? ORDER BY cs.weekday, cs.start_time",
    )
    .bind(course_id)
    .fetch_all(pool)
    .await?;
    // course_schedules has no `status` column — use a neutral label
    let schedule: Vec<CourseSessionRow> = sched
        .iter()
        .map(|r| CourseSessionRow {
            id: r.get("id"),
            seq: r.get("seq"),
            session_date: r.get("session_date"),
            start_time: r.get("start_time"),
            end_time: r.get("end_time"),
            status: "مجدول".into(),
            room_name: r.get("room_name"),
        })
        .collect();

    let collected_minor: i64 = students.iter().map(|s| s.paid_minor).sum();
    let outstanding_minor: i64 = students.iter().map(|s| s.remaining_minor).sum();
    let sessions_done = sessions.iter().filter(|x| x.status == "executed").count() as i64;
    let hours_planned: f64 = sessions
        .iter()
        .map(|x| {
            let (h1, m1) = parse_hhmm(&x.start_time);
            let (h2, m2) = parse_hhmm(&x.end_time);
            ((h2 * 60 + m2) as f64 - (h1 * 60 + m1) as f64) / 60.0
        })
        .filter(|h| *h > 0.0)
        .sum();
    let hours_done: f64 = sessions
        .iter()
        .filter(|x| x.status == "executed")
        .map(|x| {
            let (h1, m1) = parse_hhmm(&x.start_time);
            let (h2, m2) = parse_hhmm(&x.end_time);
            ((h2 * 60 + m2) as f64 - (h1 * 60 + m1) as f64) / 60.0
        })
        .filter(|h| *h > 0.0)
        .sum();

    let days_remaining: i64 = if c.7.is_empty() {
        -1
    } else {
        let days = (chrono::NaiveDate::parse_from_str(&c.7, "%Y-%m-%d")
            .ok()
            .map(|d| (d - chrono::Utc::now().date_naive()).num_days()))
            .unwrap_or(-1);
        days
    };
    let agreement_status: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT status FROM financial_agreements WHERE course_id=? AND status='active' LIMIT 1),'none')",
    )
    .bind(course_id)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|_| "none".into());

    Ok(CourseDetail {
        course_id,
        course_name: c.0,
        status: c.1,
        teacher_name: c.2,
        subject_name: c.3,
        room_name: c.4,
        fee_minor: c.5,
        start_date: c.6,
        end_date: c.7,
        students,
        sessions,
        schedule,
        collected_minor,
        outstanding_minor,
        sessions_total: sessions_total(&sess),
        sessions_done,
        hours_planned,
        hours_done,
        days_remaining,
        agreement_status,
    })
}
fn sessions_total(rows: &[sqlx::sqlite::SqliteRow]) -> i64 {
    rows.len() as i64
}
fn parse_hhmm(s: &str) -> (i64, i64) {
    let mut it = s.split(':');
    let h = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let m = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    (h, m)
}

// ---------------------------------------------------------------- rooms

#[derive(Debug, Serialize, Clone)]
pub struct RoomBookingRow {
    pub id: i64,
    pub course_name: String,
    pub session_date: String,
    pub start_time: String,
    pub end_time: String,
    pub status: String,
    pub teacher_name: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct RoomDetail {
    pub room_id: i64,
    pub code: String,
    pub name: String,
    pub capacity: i64,
    pub status: String,
    pub description: String,
    pub active_courses: i64,
    pub sessions_total: i64,
    pub sessions_executed: i64,
    pub hours_used: f64,
    pub bookings: Vec<RoomBookingRow>,
    pub schedule: Vec<RoomBookingRow>,
}

pub async fn room_detail(
    pool: &SqlitePool,
    actor: &Actor,
    room_id: i64,
) -> AppResult<RoomDetail> {
    actor.require("rooms.manage")?;
    let r: (String, String, i64, String, String) = sqlx::query_as(
        "SELECT code, name, capacity, status, COALESCE(description,'') FROM rooms WHERE id=?",
    )
    .bind(room_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("القاعة غير موجودة"))?;

    let sessions = sqlx::query(
        "SELECT se.id, c.name AS course_name, se.session_date, se.start_time, se.end_time,
                se.status, COALESCE(t.full_name,'') AS teacher_name
         FROM sessions se JOIN courses c ON c.id=se.course_id
         LEFT JOIN teachers t ON t.id=se.teacher_id
         WHERE se.room_id=? ORDER BY se.session_date DESC LIMIT 100",
    )
    .bind(room_id)
    .fetch_all(pool)
    .await?;
    let bookings = sqlx::query(
        "SELECT cs.id, c.name AS course_name, '' AS session_date, cs.start_time, cs.end_time,
                COALESCE(t.full_name,'') AS teacher_name
         FROM course_schedules cs JOIN courses c ON c.id=cs.course_id
         LEFT JOIN teachers t ON t.id=cs.teacher_id
         WHERE cs.room_id=? ORDER BY cs.weekday, cs.start_time",
    )
    .bind(room_id)
    .fetch_all(pool)
    .await?;
    let map_booking = |r: &sqlx::sqlite::SqliteRow| RoomBookingRow {
        id: r.get("id"),
        course_name: r.get("course_name"),
        session_date: r.get("session_date"),
        start_time: r.get("start_time"),
        end_time: r.get("end_time"),
        status: r.try_get("status").unwrap_or_else(|_| "مجدول".into()),
        teacher_name: r.get("teacher_name"),
    };
    let sessions: Vec<RoomBookingRow> = sessions.iter().map(map_booking).collect();
    let schedule: Vec<RoomBookingRow> = bookings.iter().map(map_booking).collect();
    let executed = sessions.iter().filter(|x| x.status == "executed").count() as i64;
    let hours_used: f64 = sessions
        .iter()
        .filter(|x| x.status == "executed")
        .map(|x| {
            let (h1, m1) = parse_hhmm(&x.start_time);
            let (h2, m2) = parse_hhmm(&x.end_time);
            ((h2 * 60 + m2) as f64 - (h1 * 60 + m1) as f64) / 60.0
        })
        .filter(|h| *h > 0.0)
        .sum();
    let active_courses: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM courses WHERE room_id=? AND status IN ('active','registration_open')",
    )
    .bind(room_id)
    .fetch_one(pool)
    .await?;

    Ok(RoomDetail {
        room_id,
        code: r.0,
        name: r.1,
        capacity: r.2,
        status: r.3,
        description: r.4,
        active_courses,
        sessions_total: sessions.len() as i64,
        sessions_executed: executed,
        hours_used,
        bookings: sessions,
        schedule,
    })
}
