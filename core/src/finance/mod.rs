//! Financial engine (Phase 6): ledger, entitlements, payments, shortfalls,
//! refunds, settlements. All money as i64 minor units (ADR-002).

pub mod entitlement;
pub mod ledger;
pub mod payments;
pub mod recognition;
pub mod refunds;
pub mod settlements;
pub mod shortfall;

pub use entitlement::{compute_entitlement, EntitlementMethod, EntitlementParams};
pub use ledger::{post_entry, reverse_entry, JournalEntry, JournalLine, NewJournalEntry, ledger_balanced, account_balance};
pub use payments::{record_payment, PaymentAllocation, PaymentResult};
pub use recognition::{accrue_enrollment, recognize_revenue};
pub use refunds::{compute_refund, RefundPolicy, RefundResult};
pub use settlements::{approve_settlement, prepare_settlement, reverse_settlement, Settlement, SettlementKind};
pub use shortfall::{decide_shortfall, propose_shortfall, ShortfallDecision, ShortfallDecisionType};
