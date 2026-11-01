//! Shared primitives: Money, currency, dates, sequence allocation helpers.

pub mod money;
pub mod seq;

pub use money::Money;
pub use seq::{allocate_next_no, seed_sequence, SequenceKind};
