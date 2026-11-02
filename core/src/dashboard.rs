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
    })
}
