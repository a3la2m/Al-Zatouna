//! Authentication domain: Argon2id passwords, sessions/lockout, permission
//! evaluation and the append-only audit trail. See docs/SECURITY.md.

pub mod audit;
pub mod password;
pub mod permissions;
pub mod service;

pub use permissions::{load_for_user, PermissionSet};
pub use service::{AuthService, LoginResult, SessionStore, SessionUser, SessionUserDto};
