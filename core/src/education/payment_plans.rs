//! Payment plans & installments (Phase 12.1).

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::AppResult;
use crate::people::context::{log_audit, Actor};

#[derive(Debug, Serialize, Clone)]
pub struct Installment {
    pub id: i64,
    pub payment_plan_id: i64,
    pub seq: i64,
    pub due_date: String,
    pub amount_minor: i64,
    pub status: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct PaymentPlan {
    pub id: i64,
    pub enrollment_id: i64,
    pub total_minor: i64,
    pub installment_count: i64,
    pub installments: Vec<Installment>,
}

pub async fn create_payment_plan(
    pool: &SqlitePool,
    actor: &Actor,
    enrollment_id: i64,
    total_minor: i64,
    installment_count: i64,
    first_due_date: &str,
    interval_days: i64,
) -> AppResult<PaymentPlan> {
    actor.require("enrollments.create")?;
    if total_minor < 0 || installment_count < 1 {
        return Err(AppError::validation("بيانات غير صالحة"));
    }
    // split total into installments (last absorbs remainder)
    let base = total_minor / installment_count;
    let remainder = total_minor % installment_count;

    let mut tx = pool.begin().await?;
    let res = sqlx::query(
        "INSERT INTO payment_plans (enrollment_id, total_minor, installment_count) VALUES (?, ?, ?)",
    )
    .bind(enrollment_id)
    .bind(total_minor)
    .bind(installment_count)
    .execute(&mut *tx)
    .await?;
    let plan_id = res.last_insert_rowid();

    let mut installments = Vec::new();
    for i in 0..installment_count {
        let amt = if i == installment_count - 1 {
            base + remainder
        } else {
            base
        };
        // due date = first + i*interval days (simple: store as string offset; use date arithmetic via sql)
        let due = if i == 0 {
            first_due_date.to_string()
        } else {
            // compute by adding days using sqlite date()
            sqlx::query_scalar::<_, String>("SELECT date(?, ?)")
                .bind(first_due_date)
                .bind(format!("+{} days", i * interval_days))
                .fetch_one(&mut *tx)
                .await
                .unwrap_or_else(|_| first_due_date.to_string())
        };
        let r = sqlx::query(
            "INSERT INTO installments (payment_plan_id, seq, due_date, amount_minor) VALUES (?, ?, ?, ?)",
        )
        .bind(plan_id)
        .bind(i + 1)
        .bind(&due)
        .bind(amt)
        .execute(&mut *tx)
        .await?;
        installments.push(Installment {
            id: r.last_insert_rowid(),
            payment_plan_id: plan_id,
            seq: i + 1,
            due_date: due,
            amount_minor: amt,
            status: "open".into(),
        });
    }
    tx.commit().await?;

    log_audit(pool, actor.user_id, "enrollments.create", "payment_plans", Some(plan_id), None, None, "").await;
    Ok(PaymentPlan { id: plan_id, enrollment_id, total_minor, installment_count, installments })
}

pub async fn list_payment_plan(pool: &SqlitePool, enrollment_id: i64) -> AppResult<Option<PaymentPlan>> {
    let plan_row = sqlx::query("SELECT id, enrollment_id, total_minor, installment_count FROM payment_plans WHERE enrollment_id = ?")
        .bind(enrollment_id)
        .fetch_optional(pool)
        .await?;
    let Some(pr) = plan_row else { return Ok(None); };
    use sqlx::Row;
    let plan_id: i64 = pr.get("id");
    let total: i64 = pr.get("total_minor");
    let count: i64 = pr.get("installment_count");
    let enr: i64 = pr.get("enrollment_id");
    let rows = sqlx::query("SELECT id, seq, due_date, amount_minor, status FROM installments WHERE payment_plan_id = ? ORDER BY seq")
        .bind(plan_id)
        .fetch_all(pool)
        .await?;
    let installments = rows.iter().map(|r| Installment {
        id: r.get("id"),
        payment_plan_id: plan_id,
        seq: r.get("seq"),
        due_date: r.get("due_date"),
        amount_minor: r.get("amount_minor"),
        status: r.get("status"),
    }).collect();
    Ok(Some(PaymentPlan { id: plan_id, enrollment_id: enr, total_minor: total, installment_count: count, installments }))
}

pub async fn record_installment_payment(
    pool: &SqlitePool,
    actor: &Actor,
    installment_id: i64,
    amount_minor: i64,
) -> AppResult<Installment> {
    actor.require("payments.record")?;
    let (plan_id, amt, status): (i64, i64, String) = sqlx::query_as(
        "SELECT payment_plan_id, amount_minor, status FROM installments WHERE id = ?",
    )
    .bind(installment_id)
    .fetch_one(pool)
    .await?;
    if status == "paid" {
        return Err(AppError::Conflict("القسط مدفوع بالكامل".into()));
    }
    // partial payment tracking: if amount >= remaining, mark paid; else partially_paid
    // simplified: if amount >= amount_minor -> paid else partially_paid
    let new_status = if amount_minor >= amt { "paid" } else { "partially_paid" };
    sqlx::query("UPDATE installments SET status = ? WHERE id = ?")
        .bind(new_status)
        .bind(installment_id)
        .execute(pool)
        .await?;
    let r = sqlx::query("SELECT id, payment_plan_id, seq, due_date, amount_minor, status FROM installments WHERE id = ?")
        .bind(installment_id)
        .fetch_one(pool)
        .await?;
    use sqlx::Row;
    Ok(Installment {
        id: r.get("id"),
        payment_plan_id: plan_id,
        seq: r.get("seq"),
        due_date: r.get("due_date"),
        amount_minor: r.get("amount_minor"),
        status: r.get("status"),
    })
}
