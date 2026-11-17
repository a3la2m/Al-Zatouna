//! Reports (Phase 10): safe report registry, no raw SQL from UI.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use rust_xlsxwriter::Workbook;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SavedReport {
    pub id: i64,
    pub name: String,
    pub definition_json: String,
    #[serde(default)]
    pub is_shared: bool,
    #[serde(default)]
    pub is_favorite: bool,
    #[serde(default)]
    pub filters_json: String,
}

/// Allowed column codes per entity — the registry rejects unknown codes.
pub fn allowed_columns(entity: &str) -> &'static [&'static str] {
    match entity {
        "students" => &["id", "full_name", "phone", "status", "registered_at"],
        "courses" => &["id", "name", "status", "fee_minor"],
        "enrollments" => &["id", "student_id", "course_id", "status", "net_fee_minor"],
        _ => &[],
    }
}

pub fn is_valid_column(entity: &str, col: &str) -> bool {
    allowed_columns(entity).contains(&col)
}

pub async fn save_report(pool: &SqlitePool, name: &str, definition_json: &str) -> Result<i64, sqlx::Error> {
    // ensure an owner user exists (FK)
    sqlx::query("INSERT OR IGNORE INTO users (id, username, full_name, password_hash) VALUES (0, 'system', 'System', 'x')")
        .execute(pool).await.ok();
    let res = sqlx::query("INSERT INTO saved_reports (report_code, name, owner_id, config_json) VALUES (?, ?, 0, ?)")
        .bind("custom")
        .bind(name)
        .bind(definition_json)
        .execute(pool)
        .await?;
    Ok(res.last_insert_rowid())
}


/// Export a simple report (list of students) to Excel bytes.
pub async fn export_excel_students(pool: &SqlitePool) -> Result<Vec<u8>, sqlx::Error> {
    let rows = sqlx::query("SELECT id, full_name, phone, status FROM students ORDER BY id")
        .fetch_all(pool).await?;
    use sqlx::Row;
    let tmp = std::env::temp_dir().join("ec_export.xlsx");
    let mut wb = Workbook::new(tmp.to_str().unwrap());
    let ws = wb.add_worksheet();
    ws.write_string_only(0, 0, "id").ok();
    ws.write_string_only(0, 1, "full_name").ok();
    ws.write_string_only(0, 2, "phone").ok();
    ws.write_string_only(0, 3, "status").ok();
    for (i, r) in rows.iter().enumerate() {
        ws.write_number_only((i+1) as u32, 0, r.get::<i64,_>("id") as f64).ok();
        ws.write_string_only((i+1) as u32, 1, r.get::<String,_>("full_name").as_str()).ok();
        ws.write_string_only((i+1) as u32, 2, r.get::<String,_>("phone").as_str()).ok();
        ws.write_string_only((i+1) as u32, 3, r.get::<String,_>("status").as_str()).ok();
    }
    wb.close().map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    let bytes = std::fs::read(&tmp).map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    let _ = std::fs::remove_file(&tmp);
    Ok(bytes)
}

pub async fn toggle_favorite(pool: &SqlitePool, id: i64, fav: bool) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE saved_reports SET is_favorite = ? WHERE id = ?")
        .bind(fav as i64)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_report_shared(pool: &SqlitePool, id: i64, shared: bool) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE saved_reports SET is_shared = ? WHERE id = ?")
        .bind(shared as i64)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn save_report_with_filters(pool: &SqlitePool, name: &str, definition_json: &str, filters_json: &str, is_shared: bool, is_favorite: bool) -> Result<i64, sqlx::Error> {
    sqlx::query("INSERT OR IGNORE INTO users (id, username, full_name, password_hash) VALUES (0, 'system', 'System', 'x')")
        .execute(pool).await.ok();
    let res = sqlx::query("INSERT INTO saved_reports (report_code, name, owner_id, config_json, filters_json, is_shared, is_favorite) VALUES (?, ?, 0, ?, ?, ?, ?)")
        .bind("custom")
        .bind(name)
        .bind(definition_json)
        .bind(filters_json)
        .bind(is_shared as i64)
        .bind(is_favorite as i64)
        .execute(pool)
        .await?;
    Ok(res.last_insert_rowid())
}

pub async fn list_reports(pool: &SqlitePool) -> Result<Vec<SavedReport>, sqlx::Error> {
    let rows = sqlx::query("SELECT id, name, config_json, is_shared, is_favorite, filters_json FROM saved_reports")
        .fetch_all(pool)
        .await?;
    use sqlx::Row;
    Ok(rows.iter().map(|r| SavedReport {
        id: r.get("id"),
        name: r.get("name"),
        definition_json: r.get("config_json"),
        is_shared: r.get::<i64,_>("is_shared") != 0,
        is_favorite: r.get::<i64,_>("is_favorite") != 0,
        filters_json: r.get("filters_json"),
    }).collect())
}

/// In-memory report registry (Phase 13.1): >= 40 reports, no raw SQL from UI.
#[derive(Debug, Clone, Serialize)]
pub struct ReportDefinition {
    pub code: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub columns: &'static [&'static str],
}

pub fn report_registry() -> Vec<ReportDefinition> {
    let r = vec![
        ("rpt_students", "تقرير الطلاب", "admin", &["id","full_name","phone","status"] as &'static [&'static str]),
        ("rpt_teachers", "تقرير المدرسين", "admin", &["id","full_name","phone","status"]),
        ("rpt_subjects", "تقرير المواد", "admin", &["id","name"]),
        ("rpt_courses", "تقرير الدورات", "admin", &["id","name","status","fee_minor"]),
        ("rpt_enrollments", "تقرير التسجيل", "admin", &["id","student_id","course_id","status","net_fee_minor"]),
        ("rpt_students_in_course", "الطلاب في كل دورة", "admin", &["course_id","student_id","status"]),
        ("rpt_schedules", "تقرير الجداول", "admin", &["id","course_id","weekday","start_time","end_time"]),
        ("rpt_rooms", "تقرير القاعات", "admin", &["id","name","capacity"]),
        ("rpt_room_occupancy", "إشغال القاعات", "admin", &["room_id","sessions"]),
        ("rpt_sessions", "تقرير الحصص", "admin", &["id","course_id","session_date","status"]),
        ("rpt_cancelled_sessions", "الحصص الملغاة والمؤجلة", "admin", &["id","status","reason"]),
        ("rpt_attendance", "الحضور والغياب", "admin", &["session_id","student_id","status"]),
        ("rpt_attendance_rate", "نسب الالتزام", "admin", &["course_id","rate"]),
        ("rpt_teacher_performance", "أداء المدرسين", "admin", &["teacher_id","sessions"]),
        ("rpt_withdrawn_frozen", "المنسحبون والمجمّدون", "admin", &["student_id","status"]),
        ("rpt_revenue", "تقرير الإيرادات", "financial", &["period","amount_minor"]),
        ("rpt_collections", "تقرير التحصيل", "financial", &["period","collected_minor"]),
        ("rpt_receivables", "تقرير الذمم", "financial", &["student_id","receivable_minor"]),
        ("rpt_student_statement", "كشف حساب الطالب", "financial", &["entry_date","debit_minor","credit_minor"]),
        ("rpt_teacher_statement", "كشف حساب المدرس", "financial", &["entry_date","debit_minor","credit_minor"]),
        ("rpt_teacher_entitlements", "مستحقات المدرسين", "financial", &["teacher_id","amount_minor"]),
        ("rpt_teacher_payments", "المدفوعات للمدرسين", "financial", &["teacher_id","amount_minor"]),
        ("rpt_center_share", "حصة المركز", "financial", &["period","amount_minor"]),
        ("rpt_shortfalls", "تقرير العجز", "financial", &["enrollment_id","uncollected_minor"]),
        ("rpt_shortfall_decisions", "قرارات تحمل العجز", "financial", &["id","decision","center_share_minor","teacher_share_minor"]),
        ("rpt_refunds", "تقرير الاستردادات", "financial", &["id","amount_minor","status"]),
        ("rpt_expenses", "تقرير المصروفات", "financial", &["id","category_id","amount_minor"]),
        ("rpt_cash_register", "تقرير الصندوق", "financial", &["register_id","balance_minor"]),
        ("rpt_cash_closing", "إغلاق الصندوق", "financial", &["closing_id","variance_minor"]),
        ("rpt_receipt_vouchers", "سندات القبض", "financial", &["id","amount_minor"]),
        ("rpt_payment_vouchers", "سندات الصرف", "financial", &["id","amount_minor"]),
        ("rpt_settlements", "تقرير التسويات", "financial", &["id","kind","status"]),
        ("rpt_account_movements", "حركة الحسابات", "financial", &["account_id","debit_minor","credit_minor"]),
        ("rpt_profit_loss", "الأرباح والخسائر", "financial", &["revenue_minor","expense_minor"]),
        ("rpt_balances", "تقرير الأرصدة", "financial", &["account_id","balance_minor"]),
        ("rpt_journal_entries", "تقرير القيود", "financial", &["id","entry_date","balanced"]),
        ("rpt_audit_users", "سجل نشاط المستخدمين", "control", &["user_id","action","created_at"]),
        ("rpt_cancelled_ops", "العمليات الملغاة", "control", &["entity_type","id"]),
        ("rpt_reversals", "القيود العكسية", "control", &["entry_id"]),
        ("rpt_permissions", "تقرير الصلاحيات", "control", &["user_id","permission"]),
        ("rpt_pending_ops", "العمليات المعلقة", "control", &["entity_type","id"]),
        ("rpt_unsettled_courses", "الدورات غير المسواة", "control", &["course_id"]),
        ("rpt_late_payments", "الدفعات المتأخرة", "control", &["enrollment_id","due_date"]),
        ("rpt_cash_variance", "فروقات الصندوق", "control", &["closing_id","variance_minor"]),
        // ---------- added in the UI/report refresh ----------
        ("rpt_course_summary", "ملخّص الدورات", "admin", &["course_id","course_name","enrolled","capacity","fee_minor"]),
        ("rpt_attendance_summary", "ملخّص حضور كل حصة", "admin", &["session_id","present","absent","late"]),
        ("rpt_teacher_workload", "حجم عمل المدرسين", "admin", &["teacher_id","full_name","courses","sessions"]),
        ("rpt_new_students", "الطلاب الجدد شهرياً", "admin", &["period","students"]),
        ("rpt_students_by_status", "الطلاب حسب الحالة", "admin", &["status","students"]),
        ("rpt_expenses_by_category", "المصروفات حسب البند", "financial", &["category","amount_minor","count"]),
        ("rpt_expenses_by_method", "المصروفات حسب طريقة الدفع", "financial", &["method","amount_minor","count"]),
        ("rpt_daily_cash_flow", "التدفق النقدي اليومي", "financial", &["date","in_minor","out_minor"]),
        ("rpt_income_statement", "قائمة الدخل الشهرية", "financial", &["period","revenue_minor","expense_minor","net_minor"]),
        ("rpt_installments_due", "الأقساط المستحقة", "financial", &["due_date","amount_minor","status"]),
        ("rpt_student_balances", "أرصدة الطلاب (مستحق ومحصّل)", "financial", &["student_id","due_minor","paid_minor"]),
        ("rpt_top_courses", "الدورات الأعلى دخلاً", "financial", &["course_id","students","revenue_minor"]),
    ];
    r.into_iter().map(|(c,t,cat,cols)| ReportDefinition { code: c, title: t, category: cat, columns: cols }).collect()
}

pub fn report_count() -> usize { report_registry().len() }

pub fn report_by_code(code: &str) -> Option<ReportDefinition> {
    report_registry().into_iter().find(|r| r.code == code)
}

/// Safe report runner: returns row count placeholder (no raw SQL from UI).
/// Static, allow-listed SQL body for a report code.
///
/// The UI never supplies SQL (ADR: no raw SQL from the frontend) — it only
/// picks a code from the registry, and every column produced here matches the
/// registry definition for that code exactly.
pub fn report_source(code: &str) -> Option<&'static str> {
    let s = match code {
        // ---------- admin / people ----------
        "rpt_students" =>
            "SELECT id, full_name, phone, status FROM students ORDER BY id DESC",
        "rpt_teachers" =>
            "SELECT id, full_name, phone, status FROM teachers ORDER BY id DESC",
        "rpt_subjects" => "SELECT id, name FROM subjects ORDER BY id",
        "rpt_rooms" => "SELECT id, name, capacity FROM rooms ORDER BY id",
        "rpt_courses" =>
            "SELECT id, name, status, fee_minor FROM courses ORDER BY id DESC",
        "rpt_course_summary" =>
            "SELECT c.id AS course_id, c.name AS course_name,
                    COUNT(e.id) AS enrolled, c.max_students AS capacity,
                    c.fee_minor AS fee_minor
             FROM courses c LEFT JOIN enrollments e
               ON e.course_id = c.id AND e.status = 'active'
             GROUP BY c.id ORDER BY c.id DESC",
        "rpt_room_occupancy" =>
            "SELECT room_id, COUNT(*) AS sessions FROM sessions
             WHERE room_id IS NOT NULL GROUP BY room_id ORDER BY sessions DESC",

        // ---------- education ----------
        "rpt_enrollments" =>
            "SELECT id, student_id, course_id, status, net_fee_minor
             FROM enrollments ORDER BY id DESC",
        "rpt_students_in_course" =>
            "SELECT course_id, student_id, status FROM enrollments
             ORDER BY course_id, id",
        "rpt_schedules" =>
            "SELECT id, course_id, weekday, start_time, end_time
             FROM course_schedules ORDER BY course_id, weekday",
        "rpt_sessions" =>
            "SELECT id, course_id, session_date, status FROM sessions
             ORDER BY session_date DESC, id DESC",
        "rpt_cancelled_sessions" =>
            "SELECT id, status, reason FROM sessions
             WHERE status IN ('cancelled','postponed')
             ORDER BY id DESC",
        "rpt_attendance" =>
            "SELECT session_id, student_id, status FROM student_attendance
             ORDER BY id DESC",
        "rpt_attendance_rate" =>
            "SELECT s.course_id AS course_id,
                    ROUND(100.0 * SUM(CASE WHEN a.status='present' THEN 1 ELSE 0 END)
                          / NULLIF(COUNT(*),0), 1) AS rate
             FROM student_attendance a
             JOIN sessions s ON s.id = a.session_id
             GROUP BY s.course_id ORDER BY rate DESC",
        "rpt_attendance_summary" =>
            "SELECT session_id,
                    SUM(CASE WHEN status='present' THEN 1 ELSE 0 END) AS present,
                    SUM(CASE WHEN status='absent'  THEN 1 ELSE 0 END) AS absent,
                    SUM(CASE WHEN status='late'    THEN 1 ELSE 0 END) AS late
             FROM student_attendance GROUP BY session_id ORDER BY session_id DESC",
        "rpt_teacher_performance" =>
            "SELECT teacher_id, COUNT(*) AS sessions FROM sessions
             WHERE teacher_id IS NOT NULL GROUP BY teacher_id
             ORDER BY sessions DESC",
        "rpt_teacher_workload" =>
            "SELECT t.id AS teacher_id, t.full_name AS full_name,
                    COUNT(DISTINCT s.course_id) AS courses,
                    COUNT(s.id) AS sessions
             FROM teachers t LEFT JOIN sessions s ON s.teacher_id = t.id
             GROUP BY t.id ORDER BY sessions DESC",
        "rpt_withdrawn_frozen" =>
            "SELECT student_id, status FROM enrollments
             WHERE status IN ('withdrawn','frozen') ORDER BY id DESC",
        "rpt_new_students" =>
            "SELECT substr(registered_at,1,7) AS period, COUNT(*) AS students
             FROM students GROUP BY period ORDER BY period DESC",
        "rpt_students_by_status" =>
            "SELECT status, COUNT(*) AS students FROM students
             GROUP BY status ORDER BY students DESC",

        // ---------- finance ----------
        "rpt_revenue" =>
            "SELECT substr(je.entry_date,1,7) AS period,
                    SUM(jl.debit_minor) AS amount_minor
             FROM journal_lines jl
             JOIN journal_entries je ON je.id = jl.entry_id
             JOIN accounts a ON a.id = jl.account_id
             WHERE a.kind='revenue' AND je.status='posted'
             GROUP BY period ORDER BY period DESC",
        "rpt_collections" =>
            "SELECT substr(voucher_date,1,7) AS period,
                    SUM(amount_minor) AS collected_minor
             FROM receipt_vouchers WHERE status='posted'
             GROUP BY period ORDER BY period DESC",
        "rpt_receivables" =>
            "SELECT student_id, SUM(net_fee_minor) AS receivable_minor
             FROM enrollments WHERE status='active'
             GROUP BY student_id ORDER BY receivable_minor DESC",
        "rpt_student_statement" =>
            "SELECT je.entry_date AS entry_date,
                    jl.debit_minor AS debit_minor,
                    jl.credit_minor AS credit_minor
             FROM journal_lines jl
             JOIN journal_entries je ON je.id = jl.entry_id
             WHERE je.source_type='payment'
             ORDER BY je.entry_date DESC, jl.id",
        "rpt_teacher_statement" =>
            "SELECT je.entry_date AS entry_date,
                    jl.debit_minor AS debit_minor,
                    jl.credit_minor AS credit_minor
             FROM journal_lines jl
             JOIN journal_entries je ON je.id = jl.entry_id
             JOIN accounts a ON a.id = jl.account_id
             WHERE a.kind='liability'
             ORDER BY je.entry_date DESC, jl.id",
        "rpt_teacher_entitlements" =>
            "SELECT teacher_id, SUM(total_minor) AS amount_minor
             FROM settlements WHERE status='approved'
             GROUP BY teacher_id ORDER BY amount_minor DESC",
        "rpt_teacher_payments" =>
            "SELECT teacher_id, SUM(amount_minor) AS amount_minor
             FROM payment_vouchers
             WHERE beneficiary_type='teacher' AND status='paid'
             GROUP BY teacher_id ORDER BY amount_minor DESC",
        "rpt_center_share" =>
            "SELECT substr(period_from,1,7) AS period,
                    SUM(total_minor) AS amount_minor
             FROM settlements GROUP BY period ORDER BY period DESC",
        "rpt_shortfalls" =>
            "SELECT enrollment_id, uncollected_minor FROM shortfall_decisions
             ORDER BY id DESC",
        "rpt_shortfall_decisions" =>
            "SELECT id, decision, center_share_minor, teacher_share_minor
             FROM shortfall_decisions ORDER BY id DESC",
        "rpt_refunds" =>
            "SELECT id, refundable_minor AS amount_minor, status
             FROM refund_requests ORDER BY id DESC",
        "rpt_expenses" =>
            "SELECT id, category_id, amount_minor FROM expenses
             ORDER BY id DESC",
        "rpt_expenses_by_category" =>
            "SELECT COALESCE(ec.name,'غير مصنّف') AS category,
                    SUM(e.amount_minor) AS amount_minor, COUNT(*) AS count
             FROM expenses e LEFT JOIN expense_categories ec ON ec.id = e.category_id
             GROUP BY ec.name ORDER BY amount_minor DESC",
        "rpt_expenses_by_method" =>
            "SELECT method, SUM(amount_minor) AS amount_minor, COUNT(*) AS count
             FROM expenses GROUP BY method ORDER BY amount_minor DESC",

        // ---------- cash & vouchers ----------
        "rpt_cash_register" =>
            "SELECT register_id, balance_after_minor AS balance_minor
             FROM (SELECT register_id, balance_after_minor,
                          ROW_NUMBER() OVER (PARTITION BY register_id ORDER BY id DESC) rn
                   FROM cash_movements)
             WHERE rn = 1 ORDER BY register_id",
        "rpt_cash_closing" =>
            "SELECT id AS closing_id, variance_minor FROM register_closings
             ORDER BY id DESC",
        "rpt_cash_variance" =>
            "SELECT id AS closing_id, variance_minor FROM register_closings
             WHERE variance_minor <> 0 ORDER BY id DESC",
        "rpt_daily_cash_flow" =>
            "SELECT substr(occurred_at,1,10) AS date,
                    SUM(CASE WHEN direction='in'  THEN amount_minor ELSE 0 END) AS in_minor,
                    SUM(CASE WHEN direction='out' THEN amount_minor ELSE 0 END) AS out_minor
             FROM cash_movements GROUP BY date ORDER BY date DESC",
        "rpt_receipt_vouchers" =>
            "SELECT id, amount_minor FROM receipt_vouchers ORDER BY id DESC",
        "rpt_payment_vouchers" =>
            "SELECT id, amount_minor FROM payment_vouchers ORDER BY id DESC",
        "rpt_settlements" =>
            "SELECT id, kind, status FROM settlements ORDER BY id DESC",

        // ---------- ledger ----------
        "rpt_account_movements" =>
            "SELECT account_id, debit_minor, credit_minor FROM journal_lines
             ORDER BY id DESC",
        "rpt_profit_loss" =>
            "SELECT
               SUM(CASE WHEN a.kind='revenue' THEN jl.credit_minor - jl.debit_minor ELSE 0 END) AS revenue_minor,
               SUM(CASE WHEN a.kind='expense' THEN jl.debit_minor - jl.credit_minor ELSE 0 END) AS expense_minor
             FROM journal_lines jl
             JOIN accounts a ON a.id = jl.account_id
             JOIN journal_entries je ON je.id = jl.entry_id
             WHERE je.status='posted'",
        "rpt_income_statement" =>
            "SELECT substr(je.entry_date,1,7) AS period,
                    SUM(CASE WHEN a.kind='revenue' THEN jl.credit_minor - jl.debit_minor ELSE 0 END) AS revenue_minor,
                    SUM(CASE WHEN a.kind='expense' THEN jl.debit_minor - jl.credit_minor ELSE 0 END) AS expense_minor,
                    SUM(CASE WHEN a.kind='revenue' THEN jl.credit_minor - jl.debit_minor ELSE 0 END)
                  - SUM(CASE WHEN a.kind='expense' THEN jl.debit_minor - jl.credit_minor ELSE 0 END) AS net_minor
             FROM journal_lines jl
             JOIN accounts a ON a.id = jl.account_id
             JOIN journal_entries je ON je.id = jl.entry_id
             WHERE je.status='posted'
             GROUP BY period ORDER BY period DESC",
        "rpt_balances" =>
            "SELECT account_id, SUM(debit_minor) - SUM(credit_minor) AS balance_minor
             FROM journal_lines GROUP BY account_id ORDER BY account_id",
        "rpt_journal_entries" =>
            "SELECT id, entry_date,
                    CASE WHEN total_debit_minor = total_credit_minor THEN 1 ELSE 0 END AS balanced
             FROM journal_entries ORDER BY id DESC",
        "rpt_reversals" =>
            "SELECT reversal_of_id AS entry_id FROM journal_entries
             WHERE reversal_of_id IS NOT NULL ORDER BY id DESC",

        // ---------- control / audit ----------
        "rpt_audit_users" =>
            "SELECT user_id, action, logged_at AS created_at FROM audit_log
             ORDER BY id DESC",
        "rpt_cancelled_ops" =>
            "SELECT entity_type, entity_id AS id FROM audit_log
             WHERE action LIKE '%cancel%' ORDER BY id DESC",
        "rpt_permissions" =>
            "SELECT ur.user_id AS user_id, p.code AS permission
             FROM user_roles ur
             JOIN role_permissions rp ON rp.role_id = ur.role_id
             JOIN permissions p ON p.id = rp.permission_id
             ORDER BY ur.user_id, p.code",
        "rpt_pending_ops" =>
            "SELECT 'expenses' AS entity_type, id FROM expenses WHERE status='draft'
             UNION ALL
             SELECT 'refund_requests', id FROM refund_requests
               WHERE status IN ('draft','submitted')
             UNION ALL
             SELECT 'payment_vouchers', id FROM payment_vouchers
               WHERE status IN ('draft','pending_approval','approved')
             ORDER BY 1, 2",
        "rpt_unsettled_courses" =>
            "SELECT c.id AS course_id FROM courses c
             WHERE c.status = 'completed'
               AND NOT EXISTS (SELECT 1 FROM settlements s WHERE s.course_id = c.id)
             ORDER BY c.id",
        "rpt_late_payments" =>
            "SELECT pp.enrollment_id AS enrollment_id, i.due_date AS due_date
             FROM installments i
             JOIN payment_plans pp ON pp.id = i.payment_plan_id
             WHERE i.status <> 'paid' AND i.due_date < date('now')
             ORDER BY i.due_date",
        "rpt_installments_due" =>
            "SELECT i.due_date AS due_date, i.amount_minor AS amount_minor,
                    i.status AS status
             FROM installments i WHERE i.status <> 'paid'
             ORDER BY i.due_date",
        "rpt_student_balances" =>
            "SELECT e.student_id AS student_id,
                    SUM(e.net_fee_minor) AS due_minor,
                    COALESCE((SELECT SUM(rv.amount_minor) FROM receipt_vouchers rv
                              WHERE rv.student_id = e.student_id AND rv.status='posted'),0)
                      AS paid_minor
             FROM enrollments e WHERE e.status='active'
             GROUP BY e.student_id ORDER BY due_minor DESC",
        "rpt_top_courses" =>
            "SELECT e.course_id AS course_id, COUNT(*) AS students,
                    SUM(e.net_fee_minor) AS revenue_minor
             FROM enrollments e WHERE e.status='active'
             GROUP BY e.course_id ORDER BY revenue_minor DESC",
        // Unknown code: the caller must fall through to the registry lookup.
        _ => return None,
    };
    Some(s)
}

/// One executed report: metadata + real rows straight from SQLite.
#[derive(Debug, Clone, Serialize)]
pub struct ReportResult {
    pub code: String,
    pub title: String,
    pub category: String,
    pub columns: Vec<String>,
    pub rows: Vec<serde_json::Value>,
    pub row_count: i64,
}

/// Execute a registered report and return its rows as JSON objects.
///
/// The row set is produced by SQLite itself (`json_group_array(json_object(..))`)
/// so integer/real values stay numbers in the frontend instead of becoming
/// strings — money formatting and charting depend on that.
pub async fn execute_report(
    pool: &SqlitePool,
    code: &str,
) -> Result<ReportResult, crate::error::AppError> {
    let def = report_by_code(code).ok_or_else(|| {
        crate::error::AppError::NotFound("التقرير غير موجود".into())
    })?;
    let src = report_source(code).ok_or_else(|| {
        crate::error::AppError::NotFound("التقرير غير معرّف".into())
    })?;
    let fields = def
        .columns
        .iter()
        .map(|c| format!("'{c}', {c}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT COALESCE(json_group_array(json_object({fields})), '[]') FROM ({src}) src"
    );
    let json: String = sqlx::query_scalar(sql.as_str()).fetch_one(pool).await?;
    let rows: Vec<serde_json::Value> = serde_json::from_str(&json).map_err(|e| {
        crate::error::AppError::storage(format!("تعذّر قراءة نتيجة {code}: {e}"))
    })?;
    let row_count = rows.len() as i64;
    Ok(ReportResult {
        code: def.code.to_string(),
        title: def.title.to_string(),
        category: def.category.to_string(),
        columns: def.columns.iter().map(|c| c.to_string()).collect(),
        rows,
        row_count,
    })
}

/// Row count of a registered report (kept for compatibility with Phase 10).
pub async fn run_report(pool: &SqlitePool, code: &str) -> Result<i64, sqlx::Error> {
    if report_by_code(code).is_none() {
        return Err(sqlx::Error::RowNotFound);
    }
    let src = report_source(code).ok_or(sqlx::Error::RowNotFound)?;
    let sql = format!("SELECT COUNT(*) FROM ({src}) src");
    let n: i64 = sqlx::query_scalar(sql.as_str()).fetch_one(pool).await?;
    Ok(n)
}

pub fn report_codes() -> Vec<&'static str> {
    report_registry().iter().map(|r| r.code).collect()
}

pub fn list_report_definitions() -> Vec<ReportDefinition> { report_registry() }
pub fn all_reports() -> Vec<ReportDefinition> { report_registry() }

/// Export any registered report to an .xlsx byte blob (header + rows).
///
/// Values keep their real type: numbers stay numbers so Excel can total them.
pub async fn export_excel_report(
    pool: &SqlitePool,
    code: &str,
) -> Result<Vec<u8>, crate::error::AppError> {
    let res = execute_report(pool, code).await?;
    let tmp = std::env::temp_dir().join(format!("ec_report_{code}.xlsx"));
    let mut wb = Workbook::new(tmp.to_str().ok_or_else(|| {
        crate::error::AppError::internal("تعذّر إنشاء ملف مؤقت للتصدير")
    })?);
    let ws = wb.add_worksheet();
    for (c, col) in res.columns.iter().enumerate() {
        ws.write_string_only(0, c as u16, col).map_err(|e| {
            crate::error::AppError::storage(e.to_string())
        })?;
    }
    for (r, row) in res.rows.iter().enumerate() {
        let obj = row.as_object().cloned().unwrap_or_default();
        for (c, col) in res.columns.iter().enumerate() {
            let v = obj.get(col);
            let cell = (r + 1) as u32;
            match v {
                Some(serde_json::Value::Number(n)) => {
                    let f = n.as_f64().unwrap_or(0.0);
                    ws.write_number_only(cell, c as u16, f).ok();
                }
                Some(serde_json::Value::String(s)) => {
                    ws.write_string_only(cell, c as u16, s).ok();
                }
                Some(serde_json::Value::Bool(b)) => {
                    ws.write_string_only(cell, c as u16, if *b { "true" } else { "false" }).ok();
                }
                _ => {}
            }
        }
    }
    wb.close()
        .map_err(|e| crate::error::AppError::storage(e.to_string()))?;
    let bytes = std::fs::read(&tmp)
        .map_err(|e| crate::error::AppError::storage(e.to_string()))?;
    let _ = std::fs::remove_file(&tmp);
    Ok(bytes)
}
