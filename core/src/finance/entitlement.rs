//! Teacher entitlement computation (Phase 6, FINANCIAL_ENGINE §1.1).
//!
//! Half-up rounding happens exactly once at the final amount; intermediate
//! sums are kept as integers and only divided at the end.

use crate::error::AppError;
use crate::AppResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntitlementMethod {
    PercentOfAgreed,
    PercentOfCollected,
    PercentOfExecutedSessions,
    FlatCourse,
    FlatSession,
    FlatPlusPercent,
}

impl EntitlementMethod {
    pub fn parse(s: &str) -> AppResult<Self> {
        Ok(match s {
            "percent_of_agreed" => Self::PercentOfAgreed,
            "percent_of_collected" => Self::PercentOfCollected,
            "percent_of_executed_sessions" => Self::PercentOfExecutedSessions,
            "flat_course" => Self::FlatCourse,
            "flat_session" => Self::FlatSession,
            "flat_plus_percent" => Self::FlatPlusPercent,
            _ => return Err(AppError::validation("طريقة استحقاق غير معروفة")),
        })
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PercentOfAgreed => "percent_of_agreed",
            Self::PercentOfCollected => "percent_of_collected",
            Self::PercentOfExecutedSessions => "percent_of_executed_sessions",
            Self::FlatCourse => "flat_course",
            Self::FlatSession => "flat_session",
            Self::FlatPlusPercent => "flat_plus_percent",
        }
    }
}

/// Inputs for one entitlement computation. All amounts in minor units.
#[derive(Debug, Clone)]
pub struct EntitlementParams {
    pub method: EntitlementMethod,
    /// teacher rate in bp (10000 = 100%)
    pub rate_bp: i64,
    /// agreed basis (net fee portion) for percent methods
    pub agreed_basis_minor: i64,
    /// actually collected/allocated for percent_of_collected
    pub collected_minor: i64,
    /// executed session count
    pub executed_sessions: i64,
    /// per-session value (course fee ÷ session count, or explicit)
    pub session_value_minor: i64,
    /// per-session flat amount (flat_session / flat_plus_percent)
    pub per_session_minor: i64,
    /// flat base amount (flat_course / flat_plus_percent)
    pub flat_minor: i64,
}

/// Actually simpler and correct for non-negative inputs:
fn mul_bp_half_up(amount: i64, bp: i64) -> i64 {
    if amount >= 0 && bp >= 0 {
        (amount * bp + 5_000) / 10_000
    } else {
        let total = amount * bp;
        let q = total / 10_000;
        let r = total % 10_000;
        if r.abs() * 2 >= 10_000 {
            q + if total >= 0 { 1 } else { -1 }
        } else {
            q
        }
    }
}

/// Compute the teacher entitlement amount (minor units).
pub fn compute_entitlement(p: &EntitlementParams) -> i64 {
    match p.method {
        EntitlementMethod::PercentOfAgreed => mul_bp_half_up(p.agreed_basis_minor, p.rate_bp),
        EntitlementMethod::PercentOfCollected => mul_bp_half_up(p.collected_minor, p.rate_bp),
        EntitlementMethod::PercentOfExecutedSessions => {
            mul_bp_half_up(p.executed_sessions * p.session_value_minor, p.rate_bp)
        }
        EntitlementMethod::FlatCourse => p.flat_minor,
        EntitlementMethod::FlatSession => p.executed_sessions * p.per_session_minor,
        EntitlementMethod::FlatPlusPercent => {
            p.flat_minor + mul_bp_half_up(p.agreed_basis_minor, p.rate_bp)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_params(m: EntitlementMethod) -> EntitlementParams {
        EntitlementParams {
            method: m,
            rate_bp: 3000, // 30%
            agreed_basis_minor: 100_000,
            collected_minor: 60_000,
            executed_sessions: 8,
            session_value_minor: 12_500,
            per_session_minor: 5_000,
            flat_minor: 10_000,
        }
    }

    #[test]
    fn percent_of_agreed_exact() {
        let p = base_params(EntitlementMethod::PercentOfAgreed);
        // 100000 * 3000 / 10000 = 30000
        assert_eq!(compute_entitlement(&p), 30_000);
    }

    #[test]
    fn percent_of_collected() {
        let p = base_params(EntitlementMethod::PercentOfCollected);
        assert_eq!(compute_entitlement(&p), 18_000); // 60000*0.3
    }

    #[test]
    fn percent_of_executed_sessions() {
        let p = base_params(EntitlementMethod::PercentOfExecutedSessions);
        // 8 * 12500 = 100000 basis * 0.3 = 30000
        assert_eq!(compute_entitlement(&p), 30_000);
    }

    #[test]
    fn flat_course() {
        let p = base_params(EntitlementMethod::FlatCourse);
        assert_eq!(compute_entitlement(&p), 10_000);
    }

    #[test]
    fn flat_session() {
        let p = base_params(EntitlementMethod::FlatSession);
        assert_eq!(compute_entitlement(&p), 8 * 5_000);
    }

    #[test]
    fn flat_plus_percent() {
        let p = base_params(EntitlementMethod::FlatPlusPercent);
        assert_eq!(compute_entitlement(&p), 10_000 + 30_000);
    }

    #[test]
    fn half_up_rounding_once() {
        // 1000 * 3333 / 10000 = 333.3 → 333
        let mut p = base_params(EntitlementMethod::PercentOfAgreed);
        p.agreed_basis_minor = 1000;
        p.rate_bp = 3333;
        assert_eq!(compute_entitlement(&p), 333);
        // 1001 * 3333 / 10000 = 333.6333 → 334
        p.agreed_basis_minor = 1001;
        assert_eq!(compute_entitlement(&p), 334);
    }

    #[test]
    fn rate_bp_validation() {
        assert!(EntitlementMethod::parse("bogus").is_err());
        assert_eq!(EntitlementMethod::parse("flat_course").unwrap().as_str(), "flat_course");
    }
}
