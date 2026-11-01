//! Money type: integer minor units + currency metadata.
//!
//! Rules (docs/DECISIONS.md ADR-002/003):
//! - money is ALWAYS integer minor units; floats are forbidden
//! - percentages/ratios are integer basis points (10000 bp = 100 %)
//! - rounding happens once, half-up, at the final computed amount
//!
//! Currency code + decimals live in `center_settings`; this type keeps its
//! own (code, decimals) pair so computations never depend on hidden state.

use crate::error::AppError;

/// A monetary amount in the smallest currency unit.
#[derive(Clone, PartialEq, Eq)]
pub struct Money {
    minor: i64,
    decimals: u8,
    currency: String,
}

impl std::fmt::Display for Money {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.formatted())
    }
}

impl std::fmt::Debug for Money {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Money({} {})", self.formatted(), self.currency)
    }
}

impl Money {
    /// Construct from minor units with a currency code + decimal count.
    pub fn new(minor: i64, currency: &str, decimals: u8) -> Self {
        debug_assert!(decimals <= 8, "unreasonable decimals");
        Self {
            minor,
            decimals,
            currency: currency.to_uppercase(),
        }
    }

    /// Zero amount in a currency.
    pub fn zero(currency: &str, decimals: u8) -> Self {
        Self::new(0, currency, decimals)
    }

    /// From a decimal string like "100.50" (exact decimal arithmetic, no
    /// float). Accepts '.' or ',' as the decimal separator and strips
    /// thousands separators: when both separators appear, the LAST one is
    /// the decimal point; a lone ',' followed by exactly ≤decimals digits
    /// at the end is also treated as a decimal point, otherwise it is a
    /// thousands separator (e.g. "1,000.05" / "1.000,05" / "1,000").
    pub fn parse(text: &str, currency: &str, decimals: u8) -> Result<Self, AppError> {
        let t = text.trim();
        if t.is_empty() {
            return Err(AppError::validation("amount is empty"));
        }
        let negative = t.starts_with('-');
        let s = t.trim_start_matches(['-', '+']);
        if s.is_empty() {
            return Err(AppError::validation(format!("invalid amount: {t}")));
        }

        // positions of separators
        let dots: Vec<usize> = s.match_indices('.').map(|(i, _)| i).collect();
        let commas: Vec<usize> = s.match_indices(',').map(|(i, _)| i).collect();
        let last_dot = dots.last().copied();
        let last_comma = commas.last().copied();

        // decimal point = the last separator that actually looks like one
        let dec_pos: Option<usize> = match (last_dot, last_comma) {
            (Some(d), Some(c)) => Some(d.max(c)),
            (Some(d), None) => Some(d),
            (None, Some(c)) => {
                // lone comma: decimal only if followed by ≤ decimals digits
                let after = &s[c + 1..];
                if after.len() <= decimals as usize && after.chars().all(|ch| ch.is_ascii_digit()) && !after.is_empty() {
                    Some(c)
                } else {
                    None
                }
            }
            (None, None) => None,
        };

        // validate + remove non-decimal separators
        let mut main = String::new();
        let mut frac = String::new();
        for (i, ch) in s.char_indices() {
            match ch {
                '0'..='9' => {
                    if dec_pos.is_some_and(|p| i > p) {
                        frac.push(ch);
                    } else {
                        main.push(ch);
                    }
                }
                '.' | ',' => {
                    if dec_pos != Some(i) {
                        // thousands separator: must be followed by exactly
                        // 3 digits (e.g. 1,000.05 / 1.000,05)
                        let nxt: Vec<char> = s[i + 1..].chars().take(3).collect();
                        if nxt.len() != 3 || !nxt.iter().all(|c| c.is_ascii_digit()) {
                            return Err(AppError::validation(format!(
                                "invalid amount: {t}"
                            )));
                        }
                        continue;
                    }
                }
                _ => return Err(AppError::validation(format!("invalid amount: {t}"))),
            }
        }
        // pad/trim fraction to `decimals`, rounding half-up on overflow digits
        let frac = match frac.len().cmp(&(decimals as usize)) {
            std::cmp::Ordering::Less => format!("{frac:<width$}", width = decimals as usize)
                .replace(' ', "0"),
            std::cmp::Ordering::Equal => frac,
            std::cmp::Ordering::Greater => {
                let keep: &str = &frac[..decimals as usize];
                let rest: u32 = src_rest_digit(&frac[decimals as usize..]);
                let mut kept: u64 = keep.parse().unwrap_or(0);
                if rest >= 5 {
                    kept += 1;
                }
                format!("{kept:0width$}", width = decimals as usize)
            }
        };
        let main_val: i64 = if main.is_empty() {
            0
        } else {
            main.parse::<i64>()
                .map_err(|_| AppError::validation(format!("amount too large: {t}")))?
        };
        let frac_val: i64 = if decimals == 0 {
            0
        } else {
            frac.parse::<i64>().unwrap_or(0)
        };
        let scale = 10i64.pow(decimals as u32);
        let mut minor = main_val
            .checked_mul(scale)
            .and_then(|m| m.checked_add(frac_val))
            .ok_or_else(|| AppError::validation(format!("amount too large: {t}")))?;
        if negative {
            minor = -minor;
        }
        Ok(Self::new(minor, currency, decimals))
    }

    /// Minor-unit representation (the storage format).
    pub fn minor(&self) -> i64 {
        self.minor
    }

    pub fn currency(&self) -> &str {
        &self.currency
    }

    pub fn decimals(&self) -> u8 {
        self.decimals
    }

    pub fn is_zero(&self) -> bool {
        self.minor == 0
    }

    pub fn is_negative(&self) -> bool {
        self.minor < 0
    }

    /// Add two amounts of the same currency.
    pub fn add(&self, other: &Money) -> Result<Money, AppError> {
        self.ensure_same(other)?;
        self.minor
            .checked_add(other.minor)
            .map(|m| Money::new(m, &self.currency, self.decimals))
            .ok_or_else(|| AppError::validation("amount overflow"))
    }

    /// Subtract `other` from `self` (same currency required).
    pub fn sub(&self, other: &Money) -> Result<Money, AppError> {
        self.ensure_same(other)?;
        self.minor
            .checked_sub(other.minor)
            .map(|m| Money::new(m, &self.currency, self.decimals))
            .ok_or_else(|| AppError::validation("amount overflow"))
    }

    /// Multiply by a positive ratio expressed in basis points (10000 = 100%).
    /// Rounds half-up once, per project rule.
    pub fn mul_bp(&self, bp: i64) -> Result<Money, AppError> {
        if bp < 0 {
            return Err(AppError::validation("negative basis points"));
        }
        // minor × bp / 10000 with half-up single rounding
        let total = self
            .minor
            .checked_mul(bp)
            .ok_or_else(|| AppError::validation("amount overflow"))?;
        let q = total / 10_000;
        let r = total % 10_000;
        let rounded = if r.abs() * 2 >= 10_000 { q + q.signum() } else { q };
        Ok(Money::new(rounded, &self.currency, self.decimals))
    }

    /// Exact integer division among parts; the *last* part absorbs the
    /// remainder so parts always add up to the original (no money created
    /// or destroyed). `bps` must be positive and sum to 10000.
    pub fn split_bp(&self, bps: &[i64]) -> Result<Vec<Money>, AppError> {
        if bps.is_empty() {
            return Err(AppError::validation("empty split"));
        }
        let sum: i64 = bps.iter().sum();
        if sum != 10_000 {
            return Err(AppError::validation(format!(
                "split must total 10000 bp, got {sum}"
            )));
        }
        let mut parts = Vec::with_capacity(bps.len());
        let mut allocated = 0i64;
        for (i, bp) in bps.iter().enumerate() {
            if *bp < 0 {
                return Err(AppError::validation("negative basis points"));
            }
            let part = if i + 1 == bps.len() {
                self.minor - allocated // last part absorbs the remainder
            } else {
                let p = (*bp as i128 * self.minor as i128 / 10_000) as i64;
                allocated += p;
                p
            };
            parts.push(Money::new(part, &self.currency, self.decimals));
        }
        Ok(parts)
    }

    /// Human display like "100.50" (decimal digits per currency decimals).
    pub fn formatted(&self) -> String {
        let scale = 10i64.pow(self.decimals as u32);
        let abs = self.minor.abs();
        let main = abs / scale;
        let frac = abs.rem_euclid(scale);
        let body = if self.decimals == 0 {
            format!("{main}")
        } else {
            format!("{main}.{:0width$}", frac, width = self.decimals as usize)
        };
        if self.minor < 0 {
            format!("-{body}")
        } else {
            body
        }
    }

    fn ensure_same(&self, other: &Money) -> Result<(), AppError> {
        if self.currency != other.currency || self.decimals != other.decimals {
            Err(AppError::validation(format!(
                "currency mismatch: {} ({}/{}) vs {} ({}/{})",
                self.currency, self.currency, self.decimals,
                other.currency, other.currency, other.decimals
            )))
        } else {
            Ok(())
        }
    }
}

fn src_rest_digit(s: &str) -> u32 {
    // first remaining digit decides half-up rounding
    s.chars().next().and_then(|c| c.to_digit(10)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ils(v: &str) -> Money {
        Money::parse(v, "ILS", 2).unwrap()
    }

    #[test]
    fn parse_basic() {
        assert_eq!(ils("100.50").minor(), 10050);
        assert_eq!(ils("100.5").minor(), 10050);
        assert_eq!(ils("100").minor(), 10000);
        assert_eq!(ils("0.99").minor(), 99);
        assert_eq!(ils("-77.25").minor(), -7725);
        assert_eq!(ils("1,000.05").minor(), 100005);
        assert_eq!(ils("00012.3").minor(), 1230);
    }

    #[test]
    fn parse_rounds_half_up() {
        assert_eq!(ils("0.005").minor(), 1);
        assert_eq!(ils("0.004").minor(), 0);
        assert_eq!(ils("2.999").minor(), 300);
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(Money::parse("12abc", "ILS", 2).is_err());
        assert!(Money::parse("", "ILS", 2).is_err());
        assert!(Money::parse("1.2.3", "ILS", 2).is_err());
    }
    #[test]
    fn arithmetic_and_match() {
        let a = ils("10.25");
        let b = ils("5.75");
        assert_eq!(a.add(&b).unwrap().minor(), 1600);
        assert_eq!(a.sub(&b).unwrap().minor(), 450);
        assert_eq!(a.sub(&a).unwrap().minor(), 0);
    }

    #[test]
    fn currency_must_match() {
        let ils = Money::parse("5.00", "ILS", 2).unwrap();
        let usd = Money::parse("5.00", "USD", 2).unwrap();
        assert!(ils.add(&usd).is_err());
    }

    #[test]
    fn basis_point_multiplication_half_up_once() {
        let fee = ils("1000.00");
        // 3333 bp of 100000 minor = 33330.0 -> 33330
        assert_eq!(fee.mul_bp(3333).unwrap().minor(), 33330);
        // 5 bp of 10010 minor = 5.005 -> 5
        assert_eq!(ils("100.10").mul_bp(5).unwrap().minor(), 5);
        // 155 bp of 10000 minor = 155
        assert_eq!(ils("100.00").mul_bp(155).unwrap().minor(), 155);
    }

    #[test]
    fn split_absorbs_remainder_in_last_part() {
        // 100.01 split 50/50 → 5000 + 5001 = 10001 (last absorbs)
        let parts = ils("100.01").split_bp(&[5000, 5000]).unwrap();
        assert_eq!(parts[0].minor(), 5000);
        assert_eq!(parts[1].minor(), 5001);
        assert_eq!(
            parts[0].add(&parts[1]).unwrap(),
            ils("100.01")
        );
        // 1 minor split 33/33/34 must keep the total exactly
        let p = Money::new(1, "ILS", 2).split_bp(&[3334, 3333, 3333]).unwrap();
        let total: i64 = p.iter().map(|m| m.minor()).sum();
        assert_eq!(total, 1);
    }

    #[test]
    fn display_uses_currency_decimals() {
        assert_eq!(ils("100.50").formatted(), "100.50");
        assert_eq!(ils("-0.05").formatted(), "-0.05");
        let jod = Money::parse("12.345", "JOD", 3).unwrap();
        assert_eq!(jod.formatted(), "12.345");
    }
}
