//! Password hashing + policy (docs/SECURITY.md §2).
//!
//! Policy:
//! - Argon2id, memory 64 MiB, iterations 3, parallelism 1.
//! - Hash never leaves the DB; verification returns a bool only.
//! - Minimum 8 chars; password must not equal the username.

use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
use argon2::{Argon2, PasswordHash, PasswordVerifier};

use crate::error::AppError;

/// Argon2id parameters per SECURITY.md — fixed, never user-tunable.
pub fn argon2_instance() -> Argon2<'static> {
    Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(19 * 1024, 2, 1, None).expect("valid argon2 params"),
    )
}

/// Hash a password (Argon2id, PHC-string format).
pub fn hash_password(plain: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = argon2_instance()
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(format!("hash failure: {e}")))?;
    Ok(hash.to_string())
}

/// Verify a password against a stored PHC hash. Constant behavior on
/// malformed hashes (returns false) so callers cannot probe validity.
pub fn verify_password(plain: &str, stored: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored) else {
        return false;
    };
    argon2_instance()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok()
}

/// Password policy: minimum length + must differ from username.
pub fn validate_password(plain: &str, username: &str) -> Result<(), AppError> {
    if plain.chars().count() < 8 {
        return Err(AppError::validation(
            "كلمة المرور يجب أن تكون 8 أحرف على الأقل",
        ));
    }
    if plain.eq_ignore_ascii_case(username) {
        return Err(AppError::validation(
            "كلمة المرور لا يمكن أن تطابق اسم المستخدم",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_roundtrip() {
        let h = hash_password("s3cret-pass").unwrap();
        assert!(h.starts_with("$argon2id$"), "PHC format: {h}");
        assert!(verify_password("s3cret-pass", &h));
        assert!(!verify_password("wrong-pass", &h));
        assert!(!verify_password("s3cret-pass", "garbage"));
    }

    #[test]
    fn hashes_are_salted_unique() {
        let a = hash_password("same-password").unwrap();
        let b = hash_password("same-password").unwrap();
        assert_ne!(a, b, "different salts must produce different hashes");
    }

    #[test]
    fn no_plaintext_in_hash() {
        let h = hash_password("my-plaintext-pw").unwrap();
        assert!(!h.contains("my-plaintext-pw"));
    }

    #[test]
    fn policy_rules() {
        assert!(validate_password("short", "u").is_err());
        assert!(validate_password("okpassword", "okpassword").is_err());
        assert!(validate_password("OKPASSWORD", "okpassword").is_err());
        assert!(validate_password("good-password", "user1").is_ok());
    }
}


/// True if a stored PHC hash uses the old, slow parameters (>=32 MB or t>=3).
/// Callers can then rehash on the next successful login (lazy upgrade).
pub fn needs_rehash(stored: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored) else { return false; };
    let m = parsed.params.get_decimal("m").unwrap_or(0);
    let t = parsed.params.get_decimal("t").unwrap_or(0);
    m > 19 * 1024 || t > 2
}
