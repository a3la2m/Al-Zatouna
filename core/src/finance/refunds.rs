//! Refunds (Phase 6).

use serde::Serialize;

use crate::AppResult;

#[derive(Debug, Clone, Copy)]
pub enum RefundPolicy {
    BeforeStart { admin_fee_minor: i64 },
    AfterStart { admin_fee_minor: i64, remaining_weight: i64, total_weight: i64 },
}

#[derive(Debug, Serialize, Clone)]
pub struct RefundResult {
    pub amount_minor: i64,
    pub admin_fee_minor: i64,
    pub gross_minor: i64,
}

/// Compute refund amount per policy. Capped by paid_minor (caller enforces).
pub fn compute_refund(policy: RefundPolicy, base_minor: i64, paid_minor: i64, prior_refunds_minor: i64) -> RefundResult {
    let (admin_fee, gross) = match policy {
        RefundPolicy::BeforeStart { admin_fee_minor } => (admin_fee_minor, base_minor),
        RefundPolicy::AfterStart { admin_fee_minor, remaining_weight, total_weight } => {
            let w = if total_weight > 0 { remaining_weight } else { 0 };
            let t = if total_weight > 0 { total_weight } else { 1 };
            let proportional = base_minor * w / t;
            (admin_fee_minor, proportional)
        }
    };
    let mut amount = gross - admin_fee - prior_refunds_minor;
    if amount < 0 {
        amount = 0;
    }
    // cap by paid
    let cap = paid_minor.saturating_sub(prior_refunds_minor);
    if amount > cap {
        amount = cap;
    }
    if amount < 0 { amount = 0; }
    RefundResult {
        amount_minor: amount,
        admin_fee_minor: admin_fee,
        gross_minor: gross,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn before_start_full_minus_fee() {
        let r = compute_refund(RefundPolicy::BeforeStart { admin_fee_minor: 500 }, 10000, 10000, 0);
        assert_eq!(r.amount_minor, 9500);
    }

    #[test]
    fn after_start_proportional() {
        // 50% remaining
        let r = compute_refund(
            RefundPolicy::AfterStart { admin_fee_minor: 0, remaining_weight: 5, total_weight: 10 },
            10000, 10000, 0,
        );
        assert_eq!(r.amount_minor, 5000);
    }

    #[test]
    fn clamped_to_paid_and_nonnegative() {
        let r = compute_refund(RefundPolicy::BeforeStart { admin_fee_minor: 0 }, 10000, 3000, 0);
        assert_eq!(r.amount_minor, 3000);
        let r2 = compute_refund(RefundPolicy::BeforeStart { admin_fee_minor: 20000 }, 10000, 10000, 0);
        assert_eq!(r2.amount_minor, 0);
    }

    #[test]
    fn prior_refunds_deducted() {
        let r = compute_refund(RefundPolicy::BeforeStart { admin_fee_minor: 0 }, 10000, 10000, 4000);
        assert_eq!(r.amount_minor, 6000);
    }
}
