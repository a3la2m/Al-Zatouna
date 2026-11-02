//! Arabic amount-in-words (Phase 7, vch008).
//!
//! Converts an integer minor amount to Arabic words. Handles 0..1e12.

const ONES: &[&str] = &[
    "", "واحد", "اثنان", "ثلاثة", "أربعة", "خمسة", "ستة", "سبعة", "ثمانية", "تسعة",
    "عشرة", "أحد عشر", "اثنا عشر", "ثلاثة عشر", "أربعة عشر", "خمسة عشر",
    "ستة عشر", "سبعة عشر", "ثمانية عشر", "تسعة عشر",
];
const TENS: &[&str] = &[
    "", "", "عشرون", "ثلاثون", "أربعون", "خمسون", "ستون", "سبعون", "ثمانون", "تسعون",
];
const HUNDREDS: &[&str] = &[
    "", "مائة", "مئتان", "ثلاثمائة", "أربعمائة", "خمسمائة", "ستمائة", "سبعمائة", "ثمانمائة", "تسعمائة",
];

fn under_thousand(n: i64) -> String {
    if n == 0 {
        return String::new();
    }
    let h = (n / 100) as usize;
    let r = n % 100;
    let mut parts = Vec::new();
    if h > 0 {
        parts.push(HUNDREDS[h].to_string());
    }
    if r > 0 {
        if r < 20 {
            parts.push(ONES[r as usize].to_string());
        } else {
            let t = (r / 10) as usize;
            let o = r % 10;
            if o > 0 {
                parts.push(format!("{} و{}", ONES[o as usize], TENS[t]));
            } else {
                parts.push(TENS[t].to_string());
            }
        }
    }
    parts.join(" و")
}

/// Convert an integer (major units) to Arabic words.
pub fn amount_to_words_major(n: i64) -> String {
    if n == 0 {
        return "صفر".to_string();
    }
    let negative = n < 0;
    let n = n.unsigned_abs() as i64;

    let billions = n / 1_000_000_000;
    let millions = (n / 1_000_000) % 1_000;
    let thousands = (n / 1_000) % 1_000;
    let rest = n % 1_000;

    let mut parts: Vec<String> = Vec::new();
    if billions > 0 {
        if billions == 1 {
            parts.push("مليار".to_string());
        } else if billions == 2 {
            parts.push("ملياران".to_string());
        } else if billions <= 10 {
            parts.push(format!("{} مليارات", under_thousand(billions)));
        } else {
            parts.push(format!("{} مليار", under_thousand(billions)));
        }
    }
    if millions > 0 {
        if millions == 1 {
            parts.push("مليون".to_string());
        } else if millions == 2 {
            parts.push("مليونان".to_string());
        } else if millions <= 10 {
            parts.push(format!("{} ملايين", under_thousand(millions)));
        } else {
            parts.push(format!("{} مليون", under_thousand(millions)));
        }
    }
    if thousands > 0 {
        if thousands == 1 {
            parts.push("ألف".to_string());
        } else if thousands == 2 {
            parts.push("ألفان".to_string());
        } else if thousands <= 10 {
            parts.push(format!("{} آلاف", under_thousand(thousands)));
        } else {
            parts.push(format!("{} ألف", under_thousand(thousands)));
        }
    }
    if rest > 0 {
        parts.push(under_thousand(rest));
    }
    let mut s = parts.join(" و");
    if negative {
        s = format!("ناقص {}", s);
    }
    s
}

/// Format a minor amount as words including currency units.
pub fn amount_to_words_minor(minor: i64, decimals: u8) -> String {
    let scale = 10i64.pow(decimals as u32);
    let major = minor / scale;
    let frac = minor.rem_euclid(scale);
    let mut s = amount_to_words_major(major);
    if frac > 0 {
        s = format!("{} و {} جزء", s, amount_to_words_major(frac));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero() {
        assert_eq!(amount_to_words_major(0), "صفر");
    }

    #[test]
    fn small() {
        assert_eq!(amount_to_words_major(5), "خمسة");
        assert_eq!(amount_to_words_major(21), "واحد وعشرون");
    }

    #[test]
    fn hundreds() {
        assert_eq!(amount_to_words_major(100), "مائة");
        assert_eq!(amount_to_words_major(250), "مئتان وخمسون");
    }

    #[test]
    fn thousands() {
        assert_eq!(amount_to_words_major(1000), "ألف");
        assert_eq!(amount_to_words_major(2000), "ألفان");
    }

    #[test]
    fn millions() {
        assert_eq!(amount_to_words_major(1_000_000), "مليون");
    }

    #[test]
    fn round_trip_no_panic_large() {
        let s = amount_to_words_major(999_999_999_999);
        assert!(!s.is_empty());
    }
}
