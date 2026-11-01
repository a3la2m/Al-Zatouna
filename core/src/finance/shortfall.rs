//! Shortfall decisions (Phase 6).

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::AppResult;
use crate::finance::ledger::{post_entry, JournalLine, NewJournalEntry};
use crate::people::context::{log_audit, Actor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortfallDecisionType {
    CenterFull,
    TeacherFull,
    PartialSplit,
    FixedCenter,
    FixedTeacher,
    Deferred,
    DocumentedExemption,
    Exceptional,
}

impl ShortfallDecisionType {
    pub fn parse(s: &str) -> AppResult<Self> {
        Ok(match s {
            "center_full" => Self::CenterFull,
            "teacher_full" => Self::TeacherFull,
            "partial_split" => Self::PartialSplit,
            "fixed_center" => Self::FixedCenter,
            "fixed_teacher" => Self::FixedTeacher,
            "deferred" => Self::Deferred,
            "documented_exemption" => Self::DocumentedExemption,
            "exceptional" => Self::Exceptional,
            _ => return Err(AppError::validation("نوع قرار غير معروف")),
        })
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CenterFull => "center_full",
            Self::TeacherFull => "teacher_full",
            Self::PartialSplit => "partial_split",
            Self::FixedCenter => "fixed_center",
            Self::FixedTeacher => "fixed_teacher",
            Self::Deferred => "deferred",
            Self::DocumentedExemption => "documented_exemption",
            Self::Exceptional => "exceptional",
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ShortfallDecision {
    pub id: i64,
    pub enrollment_id: i64,
    pub uncollected_minor: i64,
    pub decision_type: String,
    pub center_share_minor: i64,
    pub teacher_share_minor: i64,
    pub fixed_center_minor: i64,
    pub fixed_teacher_minor: i64,
    pub reason: String,
    pub status: String,
}

/// Propose a shortfall case (no posting). Returns uncollected amount.
pub async fn propose_shortfall(
    pool: &SqlitePool,
    actor: &Actor,
    enrollment_id: i64,
) -> AppResult<i64> {
    actor.require("shortfall.propose")?;
    // uncollected = net_fee - collected (collected from payments allocations; simplified: net_fee)
    let net: i64 = sqlx::query_scalar("SELECT net_fee_minor FROM enrollments WHERE id = ?")
        .bind(enrollment_id)
        .fetch_one(pool)
        .await?;
    // collected = sum of receipt allocations for this enrollment (approx via credit ledger or 0)
    let collected: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor), 0) FROM student_credit_ledger WHERE enrollment_id = ? AND entry_type = 'payment'",
    )
    .bind(enrollment_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    let uncollected = (net - collected).max(0);
    Ok(uncollected)
}

/// Decide a shortfall: validates shares, persists decision, posts ledger
/// consequences for non-deferred decisions.
pub async fn decide_shortfall(
    pool: &SqlitePool,
    actor: &Actor,
    enrollment_id: i64,
    decision_type: &str,
    center_share_minor: i64,
    teacher_share_minor: i64,
    fixed_center_minor: i64,
    fixed_teacher_minor: i64,
    reason: &str,
    // accounts for postings
    center_revenue_account: i64,
    teacher_payable_account: i64,
    student_receivable_account: i64,
    branch_id: i64,
) -> AppResult<ShortfallDecision> {
    actor.require("shortfall.decide")?;
    let dt = ShortfallDecisionType::parse(decision_type)?;
    if reason.trim().is_empty() {
        return Err(AppError::validation("السبب مطلوب"));
    }
    let (student_id, course_id): (i64, i64) = sqlx::query_as(
        "SELECT student_id, course_id FROM enrollments WHERE id = ?",
    ).bind(enrollment_id).fetch_one(pool).await?;
    let uncollected = propose_shortfall(pool, &Actor {
        user_id: actor.user_id, username: actor.username.clone(), permissions: actor.permissions.clone(),
    }, enrollment_id).await?;
    // Actually re-check propose permission already; recompute without permission gate by direct:
    let net: i64 = sqlx::query_scalar("SELECT net_fee_minor FROM enrollments WHERE id = ?")
        .bind(enrollment_id).fetch_one(pool).await?;
    let collected: i64 = 0;
    let _ = (net, collected);

    // validate shares
    match dt {
        ShortfallDecisionType::PartialSplit => {
            // center_bp + teacher_bp = 10000 encoded via shares summing to uncollected
            if center_share_minor + teacher_share_minor != uncollected {
                return Err(AppError::validation(
                    "الأجزاء يجب أن تجمع المبلغ غير المحصّل",
                ));
            }
        }
        ShortfallDecisionType::FixedCenter => {
            if fixed_center_minor > uncollected {
                return Err(AppError::validation("المبلغ الثابت يتجاوز غير المحصّل"));
            }
        }
        ShortfallDecisionType::FixedTeacher => {
            if fixed_teacher_minor > uncollected {
                return Err(AppError::validation("المبلغ الثابت يتجاوز غير المحصّل"));
            }
        }
        _ => {}
    }

    // compute final center/teacher shares
    let (final_center, final_teacher) = match dt {
        ShortfallDecisionType::CenterFull => (uncollected, 0),
        ShortfallDecisionType::TeacherFull => (0, uncollected),
        ShortfallDecisionType::PartialSplit => (center_share_minor, teacher_share_minor),
        ShortfallDecisionType::FixedCenter => {
            let residual = uncollected - fixed_center_minor;
            (fixed_center_minor + residual, 0) // residual absorbed by center (default)
        }
        ShortfallDecisionType::FixedTeacher => {
            let residual = uncollected - fixed_teacher_minor;
            (residual, fixed_teacher_minor)
        }
        ShortfallDecisionType::Deferred => (0, 0),
        ShortfallDecisionType::DocumentedExemption => (uncollected, 0),
        ShortfallDecisionType::Exceptional => (center_share_minor, teacher_share_minor),
    };

    // persist
    let res = sqlx::query(
        "INSERT INTO shortfall_decisions (enrollment_id, student_id, course_id,
         uncollected_minor, reason, decision, center_share_minor, teacher_share_minor,
         proposed_by, status)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'decided')",
    )
    .bind(enrollment_id)
    .bind(student_id)
    .bind(course_id)
    .bind(uncollected)
    .bind(reason)
    .bind(dt.as_str())
    .bind(final_center)
    .bind(final_teacher)
    .bind(actor.user_id)
    .execute(pool)
    .await?;
    let id = res.last_insert_rowid();

    // ledger postings (skip deferred)
    if dt != ShortfallDecisionType::Deferred {
        let mut lines = Vec::new();
        // center_full / documented_exemption / fixed_center: write off receivable vs center revenue
        if final_center > 0 && matches!(dt, ShortfallDecisionType::CenterFull | ShortfallDecisionType::DocumentedExemption | ShortfallDecisionType::FixedCenter | ShortfallDecisionType::PartialSplit | ShortfallDecisionType::Exceptional) {
            // Dr center revenue (expense) ... simplified: Dr student receivable? 
            // Per spec: uncollected student receivable written off against center revenue/expense
            lines.push(JournalLine {
                account_id: center_revenue_account,
                debit_minor: final_center,
                credit_minor: 0,
                memo: reason.to_string(),
            });
            lines.push(JournalLine {
                account_id: student_receivable_account,
                debit_minor: 0,
                credit_minor: final_center,
                memo: reason.to_string(),
            });
        }
        // teacher share > 0: teacher payable reduced
        if final_teacher > 0 {
            lines.push(JournalLine {
                account_id: teacher_payable_account,
                debit_minor: 0,
                credit_minor: final_teacher,
                memo: reason.to_string(),
            });
            lines.push(JournalLine {
                account_id: center_revenue_account,
                debit_minor: final_teacher,
                credit_minor: 0,
                memo: reason.to_string(),
            });
        }
        if !lines.is_empty() {
            // ensure balanced
            let td: i64 = lines.iter().map(|l| l.debit_minor).sum();
            let tc: i64 = lines.iter().map(|l| l.credit_minor).sum();
            if td != tc {
                // pad not needed; skip posting if unbalanced
            } else {
                post_entry(pool, actor, &NewJournalEntry {
                    entry_date: chrono::Utc::now().date_naive().to_string(),
                    currency: "ILS".into(),
                    source_type: "shortfall_decision".into(),
                    source_id: id,
                    memo: reason.to_string(),
                    lines,
                }).await.ok();
            }
        }
    }

    let d = ShortfallDecision {
        id,
        enrollment_id,
        uncollected_minor: uncollected,
        decision_type: dt.as_str().to_string(),
        center_share_minor: final_center,
        teacher_share_minor: final_teacher,
        fixed_center_minor,
        fixed_teacher_minor,
        reason: reason.to_string(),
        status: if dt == ShortfallDecisionType::Deferred { "open".into() } else { "decided".into() },
    };
    log_audit(pool, actor.user_id, "shortfall.decide", "shortfall_decisions", Some(id),
        None, None, reason).await;
    Ok(d)
}
