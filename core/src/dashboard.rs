//! Dashboard KPIs (Phase 10).

use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Serialize, Clone)]
pub struct DashboardKpis {
    pub students_total: i64,
    pub students_active: i64,
    pub courses_total: i64,
    pub courses_active: i64,
    pub enrollments_active: i64,
    pub collected_minor: i64,
    pub receivable_minor: i64,
    pub today_sessions: i64,
    pub pending_refunds: i64,
    pub pending_shortfalls: i64,
    pub cash_balance_minor: i64,
    pub teacher_payables_minor: i64,
    pub due_fees_minor: i64,
    pub courses_needing_settlement: i64,
    /// Cash taken in today (auto-calculated from posted receipts).
    pub collected_today_minor: i64,
    /// Cash taken in during the current calendar month.
    pub collected_month_minor: i64,
    /// Expenses booked in the current calendar month.
    pub expenses_month_minor: i64,
    /// Students registered during the current calendar month.
    pub students_new_month: i64,
    /// Receipts still in draft / awaiting posting.
    pub receipts_pending: i64,
}

pub async fn compute_kpis(pool: &SqlitePool) -> Result<DashboardKpis, sqlx::Error> {
    let students_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students").fetch_one(pool).await?;
    let students_active: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE status='active'").fetch_one(pool).await?;
    let courses_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM courses").fetch_one(pool).await?;
    let courses_active: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM courses WHERE status IN ('registration_open','active')").fetch_one(pool).await?;
    let enrollments_active: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM enrollments WHERE status='active'").fetch_one(pool).await?;
    let collected_minor: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(amount_minor),0) FROM receipt_vouchers WHERE status='posted'").fetch_one(pool).await?;
    let receivable_minor: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(net_fee_minor),0) FROM enrollments WHERE status='active'").fetch_one(pool).await?;
    let today_sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE session_date = date('now')").fetch_one(pool).await?;
    let pending_refunds: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM refund_requests WHERE status IN ('submitted','approved')").fetch_one(pool).await.unwrap_or(0);
    let pending_shortfalls: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM shortfall_decisions WHERE status IN ('pending','open')").fetch_one(pool).await.unwrap_or(0);
    let cash_balance_minor: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(CASE WHEN direction='in' THEN amount_minor ELSE -amount_minor END),0) FROM cash_movements").fetch_one(pool).await.unwrap_or(0);
    let teacher_payables_minor: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(credit_minor - debit_minor),0) FROM journal_lines").fetch_one(pool).await.unwrap_or(0);
    let due_fees_minor: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(net_fee_minor),0) FROM enrollments WHERE status='active'").fetch_one(pool).await.unwrap_or(0);
    let courses_needing_settlement: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM courses WHERE status='completed'").fetch_one(pool).await.unwrap_or(0);
    let collected_today_minor: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor),0) FROM receipt_vouchers
         WHERE status='posted' AND voucher_date = date('now')",
    ).fetch_one(pool).await.unwrap_or(0);
    let collected_month_minor: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor),0) FROM receipt_vouchers
         WHERE status='posted' AND strftime('%Y-%m', voucher_date) = strftime('%Y-%m','now')",
    ).fetch_one(pool).await.unwrap_or(0);
    let expenses_month_minor: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor),0) FROM expenses
         WHERE status IN ('approved','paid')
           AND strftime('%Y-%m', expense_date) = strftime('%Y-%m','now')",
    ).fetch_one(pool).await.unwrap_or(0);
    let students_new_month: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM students
         WHERE strftime('%Y-%m', registered_at) = strftime('%Y-%m','now')",
    ).fetch_one(pool).await.unwrap_or(0);
    let receipts_pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM receipt_vouchers WHERE status = 'draft'",
    ).fetch_one(pool).await.unwrap_or(0);

    Ok(DashboardKpis {
        students_total,
        students_active,
        courses_total,
        courses_active,
        enrollments_active,
        collected_minor,
        receivable_minor,
        today_sessions,
        pending_refunds,
        pending_shortfalls,
        cash_balance_minor,
        teacher_payables_minor,
        due_fees_minor,
        courses_needing_settlement,
        collected_today_minor,
        collected_month_minor,
        expenses_month_minor,
        students_new_month,
        receipts_pending,
    })
}

// ---------- charts (Phase 16) ------------------------------------------------

/// One series point: a label plus a numeric value (minor units or a count).
#[derive(Debug, Serialize, Clone)]
pub struct ChartPoint {
    pub label: String,
    pub value: i64,
}

/// A cash-flow point: money in, money out and the resulting balance that day.
#[derive(Debug, Serialize, Clone)]
pub struct CashFlowPoint {
    pub date: String,
    pub in_minor: i64,
    pub out_minor: i64,
    pub balance_minor: i64,
}

/// Everything the dashboard draws, in one round-trip.
#[derive(Debug, Serialize, Clone)]
pub struct DashboardCharts {
    /// Last 6 months, oldest first, zero-filled — safe to plot directly.
    pub collections_by_month: Vec<ChartPoint>,
    /// Last 6 months, oldest first, zero-filled.
    pub expenses_by_month: Vec<ChartPoint>,
    pub students_by_status: Vec<ChartPoint>,
    pub enrollments_by_status: Vec<ChartPoint>,
    pub attendance_by_status: Vec<ChartPoint>,
    pub top_courses: Vec<ChartPoint>,
    /// Last 14 days of cash movement, oldest first, with a running balance.
    pub cash_flow: Vec<CashFlowPoint>,
}

/// Last `n` YYYY-MM months, oldest first.
fn last_months(n: usize) -> Vec<String> {
    let now = chrono::Utc::now();
    (0..n)
        .rev()
        .map(|i| {
            let m = now - chrono::Duration::days(30 * i as i64);
            m.format("%Y-%m").to_string()
        })
        .collect()
}

async fn sum_by_month(
    pool: &SqlitePool,
    table: &str,
    date_col: &str,
    amount_col: &str,
    where_extra: &str,
    months: &[String],
) -> Result<Vec<ChartPoint>, sqlx::Error> {
    let sql = format!(
        "SELECT strftime('%Y-%m', {date_col}) AS m, COALESCE(SUM({amount_col}),0)
         FROM {table} WHERE {where_extra} GROUP BY m"
    );
    let rows: Vec<(String, i64)> = sqlx::query_as(&sql).fetch_all(pool).await?;
    let map: std::collections::HashMap<String, i64> = rows.into_iter().collect();
    Ok(months
        .iter()
        .map(|m| ChartPoint {
            label: m.clone(),
            value: map.get(m).copied().unwrap_or(0),
        })
        .collect())
}

async fn count_grouped(
    pool: &SqlitePool,
    sql: &str,
) -> Result<Vec<ChartPoint>, sqlx::Error> {
    let rows: Vec<(String, i64)> = sqlx::query_as(sql).fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|(label, value)| ChartPoint { label, value })
        .collect())
}

/// Build every dashboard chart payload.
pub async fn compute_charts(pool: &SqlitePool) -> Result<DashboardCharts, sqlx::Error> {
    let months = last_months(6);

    let collections_by_month = sum_by_month(
        pool,
        "receipt_vouchers",
        "voucher_date",
        "amount_minor",
        "status = 'posted'",
        &months,
    )
    .await?;

    let expenses_by_month = sum_by_month(
        pool,
        "expenses",
        "expense_date",
        "amount_minor",
        "status IN ('approved','paid')",
        &months,
    )
    .await?;

    let students_by_status = count_grouped(
        pool,
        "SELECT status, COUNT(*) FROM students GROUP BY status ORDER BY COUNT(*) DESC",
    )
    .await?;

    let enrollments_by_status = count_grouped(
        pool,
        "SELECT status, COUNT(*) FROM enrollments GROUP BY status ORDER BY COUNT(*) DESC",
    )
    .await?;

    let attendance_by_status = count_grouped(
        pool,
        "SELECT status, COUNT(*) FROM student_attendance GROUP BY status ORDER BY COUNT(*) DESC",
    )
    .await?;

    let top_courses = count_grouped(
        pool,
        "SELECT c.name, COUNT(e.id)
         FROM courses c JOIN enrollments e ON e.course_id = c.id AND e.status = 'active'
         GROUP BY c.id ORDER BY COUNT(e.id) DESC LIMIT 6",
    )
    .await?;

    let flow_rows: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        "SELECT d.day,
                COALESCE(SUM(CASE WHEN m.direction='in'  THEN m.amount_minor ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN m.direction='out' THEN m.amount_minor ELSE 0 END),0),
                COALESCE((SELECT SUM(CASE WHEN direction='in' THEN amount_minor ELSE -amount_minor END)
                          FROM cash_movements
                          WHERE date(occurred_at) <= d.day), 0)
         FROM (SELECT date('now', '-' || n || ' days') AS day
               FROM (WITH RECURSIVE seq(n) AS (SELECT 0 UNION ALL SELECT n+1 FROM seq WHERE n < 13)
                     SELECT n FROM seq)) d
         LEFT JOIN cash_movements m ON date(m.occurred_at) = d.day
         GROUP BY d.day ORDER BY d.day",
    )
    .fetch_all(pool)
    .await?;

    let cash_flow = flow_rows
        .into_iter()
        .map(|(date, in_minor, out_minor, balance_minor)| CashFlowPoint {
            date,
            in_minor,
            out_minor,
            balance_minor,
        })
        .collect();

    Ok(DashboardCharts {
        collections_by_month,
        expenses_by_month,
        students_by_status,
        enrollments_by_status,
        attendance_by_status,
        top_courses,
        cash_flow,
    })
}
