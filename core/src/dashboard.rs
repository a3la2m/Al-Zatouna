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
}

pub async fn compute_kpis(pool: &SqlitePool) -> Result<DashboardKpis, sqlx::Error> {
    let students_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students").fetch_one(pool).await?;
    let students_active: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE status='active'").fetch_one(pool).await?;
    let courses_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM courses").fetch_one(pool).await?;
    let courses_active: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM courses WHERE status IN ('registration_open','active')").fetch_one(pool).await?;
    let enrollments_active: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM enrollments WHERE status='active'").fetch_one(pool).await?;
    let collected_minor: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(amount_minor),0) FROM receipt_vouchers WHERE status='posted'").fetch_one(pool).await?;
    let receivable_minor: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(net_fee_minor),0) FROM enrollments WHERE status='active'").fetch_one(pool).await?;
    Ok(DashboardKpis {
        students_total,
        students_active,
        courses_total,
        courses_active,
        enrollments_active,
        collected_minor,
        receivable_minor,
    })
}
