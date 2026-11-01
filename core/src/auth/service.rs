//! Authentication service: first-run admin, login/logout, session tokens,
//! lockout, password change/reset (docs/SECURITY.md §2–§3, API §4).
//!
//! Sessions live in Rust memory (die at app exit — desktop app semantics);
//! login state, lockout counters and audit remain durable in SQLite.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::Serialize;
use sqlx::{SqlitePool, Transaction};
use tokio::sync::Mutex;

use crate::auth::{audit, password};
use crate::error::AppError;
use crate::AppResult;

/// Default lockout: 5 failed attempts → 5 minutes locked.
pub const FAILED_ATTEMPTS_LIMIT: i64 = 5;
pub const LOCKOUT_MINUTES: i64 = 5;
/// Auto-inactivity lock default: 20 minutes (configurable later).
pub const INACTIVITY_TIMEOUT: Duration = Duration::from_secs(20 * 60);

/// A live, verified session.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionUser {
    pub user_id: i64,
    pub username: String,
    pub full_name: String,
    #[serde(skip)]
    pub permissions: crate::auth::permissions::PermissionSet,
    #[serde(skip)]
    pub last_activity: Instant,
}

/// Rust-side session store keyed by opaque token.
#[derive(Default)]
pub struct SessionStore {
    sessions: Mutex<HashMap<String, SessionUser>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    async fn put(&self, token: String, user: SessionUser) {
        self.sessions.lock().await.insert(token, user);
    }

    /// Latest activity (in-place refresh) view of a session.
    pub async fn verify(&self, token: &str) -> Option<SessionUser> {
        self.verify_with_timeout(token, INACTIVITY_TIMEOUT).await
    }

    /// Same with an explicit timeout (tests use tiny values).
    pub async fn verify_with_timeout(&self, token: &str, timeout: Duration) -> Option<SessionUser> {
        let mut map = self.sessions.lock().await;
        let user = map.get_mut(token)?;
        // re-authentication required after inactivity (desktop auto-lock)
        if user.last_activity.elapsed() > timeout {
            map.remove(token);
            return None;
        }
        user.last_activity = Instant::now();
        Some(user.clone())
    }

    pub async fn remove(&self, token: &str) -> bool {
        self.sessions.lock().await.remove(token).is_some()
    }

    pub async fn len(&self) -> usize {
        self.sessions.lock().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.sessions.lock().await.is_empty()
    }

    /// Test-only: backdate a session's last activity to force expiry.
    #[cfg(any(test, feature = "test-utils"))]
    pub async fn backdate(&self, token: &str, ago: Duration) {
        let mut map = self.sessions.lock().await;
        if let Some(u) = map.get_mut(token) {
            u.last_activity = Instant::now()
                .checked_sub(ago)
                .unwrap_or_else(Instant::now);
        }
    }
}

pub struct AuthService {
    pub pool: SqlitePool,
    pub sessions: Arc<SessionStore>,
}

/// Everything the UI needs after a successful login.
#[derive(Debug, Serialize)]
pub struct LoginResult {
    pub token: String,
    pub user: SessionUserDto,
    pub must_change_password: bool,
}

/// Public shape of the session user sent to the UI.
#[derive(Debug, Serialize)]
pub struct SessionUserDto {
    pub user_id: i64,
    pub username: String,
    pub full_name: String,
    pub permissions: Vec<String>,
}

impl From<&SessionUser> for SessionUserDto {
    fn from(s: &SessionUser) -> Self {
        let mut perms: Vec<String> = s.permissions.permissions.iter().cloned().collect();
        perms.sort();
        Self {
            user_id: s.user_id,
            username: s.username.clone(),
            full_name: s.full_name.clone(),
            permissions: perms,
        }
    }
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn minutes_from_now_iso(minutes: i64) -> String {
    (Utc::now() + chrono::Duration::minutes(minutes)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn parse_iso(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
}

impl AuthService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool, sessions: Arc::new(SessionStore::new()) }
    }

    // ---------- first-run admin ----------

    /// Create the first administrator. Only valid while the users table is
    /// empty (no users exist ⇒ wizard phase).
    pub async fn create_first_admin(
        &self,
        username: &str,
        full_name: &str,
        pass: &str,
    ) -> AppResult<i64> {
        let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        if existing > 0 {
            return Err(AppError::Conflict(
                "يوجد مستخدمون بالفعل — إنشاء المدير الأول متاح فقط في التهيئة الأولى".into(),
            ));
        }
        let mut tx = self.pool.begin().await?;
        let id = create_user_in_tx(&mut tx, username, full_name, pass, true).await?;
        // grant system_admin
        sqlx::query(
            "INSERT INTO user_roles (user_id, role_id)
             SELECT ?, id FROM roles WHERE code = 'system_admin'",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        audit::log(
            &self.pool,
            audit::AuditEvent {
                user_id: Some(id),
                action: "user_created",
                entity_type: "users",
                entity_id: Some(id),
                old: None,
                new: None,
                reason: "first-run admin creation",
                ctx: None,
            },
        )
        .await
        .ok();
        Ok(id)
    }

    // ---------- login / logout ----------

    /// Verify credentials, enforce lockout, issue a session token.
    pub async fn login(
        &self,
        username: &str,
        pass: &str,
    ) -> AppResult<LoginResult> {
        let row = sqlx::query_as::<_, (i64, String, String, String, bool, i64, Option<String>)>(
            "SELECT id, username, full_name, password_hash, is_active,
                    failed_attempts, locked_until
             FROM users WHERE username = ? COLLATE NOCASE",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        let Some((uid, uname, _full, phash, is_active, failed, locked_until)) = row else {
            self.audit_failure(None, username).await;
            return Err(AppError::PermissionDenied(
                "اسم المستخدم أو كلمة المرور غير صحيحة".into(),
            ));
        };

        if !is_active {
            return Err(AppError::PermissionDenied("الحساب معطّل".into()));
        }

        // locked right now?
        if let Some(lu) = locked_until.as_deref().and_then(parse_iso) {
            if Utc::now() < lu {
                let mins = (lu - Utc::now()).num_minutes() + 1;
                return Err(AppError::PermissionDenied(format!(
                    "الحساب مقفول مؤقتاً — حاول بعد {mins} دقيقة"
                )));
            }
        }

        if !password::verify_password(pass, &phash) {
            let attempt = failed + 1;
            if attempt >= FAILED_ATTEMPTS_LIMIT {
                sqlx::query("UPDATE users SET failed_attempts = 0, locked_until = ? WHERE id = ?")
                    .bind(minutes_from_now_iso(LOCKOUT_MINUTES))
                    .bind(uid)
                    .execute(&self.pool)
                    .await?;
                self.audit_failure(Some(uid), username).await;
                audit::count_action(&self.pool, "login_failed").await.ok();
                return Err(AppError::PermissionDenied(format!(
                    "فشل الدخول {FAILED_ATTEMPTS_LIMIT} مرات — تم قفل الحساب {LOCKOUT_MINUTES} دقائق"
                )));
            }
            sqlx::query("UPDATE users SET failed_attempts = ? WHERE id = ?")
                .bind(attempt)
                .bind(uid)
                .execute(&self.pool)
                .await?;
            self.audit_failure(Some(uid), username).await;
            return Err(AppError::PermissionDenied(
                "اسم المستخدم أو كلمة المرور غير صحيحة".into(),
            ));
        }

        // success — reset counters + stamp last login
        let must_change: bool = sqlx::query_scalar(
            "SELECT must_change_password FROM users WHERE id = ?",
        )
        .bind(uid)
        .fetch_one(&self.pool)
        .await?;
        let now = now_iso();
        sqlx::query(
            "UPDATE users SET failed_attempts = 0, locked_until = NULL, last_login_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(uid)
        .execute(&self.pool)
        .await?;

        let perms = crate::auth::permissions::load_for_user(&self.pool, uid).await?;
        let su = SessionUser {
            user_id: uid,
            username: uname,
            full_name: _full,
            permissions: perms,
            last_activity: Instant::now(),
        };
        let token = new_token();
        self.sessions.put(token.clone(), su.clone()).await;
        // session row duplicated in memory only; log the event
        audit::log(
            &self.pool,
            audit::AuditEvent {
                user_id: Some(uid),
                action: "login",
                entity_type: "users",
                entity_id: Some(uid),
                old: None,
                new: None,
                reason: "",
                ctx: None,
            },
        )
        .await
        .ok();

        Ok(LoginResult {
            token,
            user: SessionUserDto::from(&su),
            must_change_password: must_change,
        })
    }

    /// End a session explicitly (logout event is durable).
    pub async fn logout(&self, token: &str) -> AppResult<()> {
        let user = self.sessions.verify(token).await;
        self.sessions.remove(token).await;
        if let Some(u) = user {
            audit::log(
                &self.pool,
                audit::AuditEvent {
                    user_id: Some(u.user_id),
                    action: "logout",
                    entity_type: "users",
                    entity_id: Some(u.user_id),
                    old: None,
                    new: None,
                    reason: "",
                    ctx: None,
                },
            )
            .await
            .ok();
        }
        Ok(())
    }

    /// Authoritative lookup used by every protected command via the shell.
    pub async fn session_user(&self, token: &str) -> AppResult<SessionUser> {
        self.sessions
            .verify(token)
            .await
            .ok_or_else(|| AppError::PermissionDenied("الجلسة غير سارية — أعد تسجيل الدخول".into()))
    }

    /// Convenience gate for commands: verify the session AND permission.
    pub async fn require(
        &self,
        token: &str,
        permission: &str,
    ) -> AppResult<SessionUser> {
        let su = self.session_user(token).await?;
        if !su.permissions.can(permission) {
            return Err(AppError::PermissionDenied(format!(
                "ليس لديك صلاحية: {permission}"
            )));
        }
        Ok(su)
    }

    // ---------- passwords ----------

    /// Self-service change (old password required).
    pub async fn change_password(
        &self,
        token: &str,
        current: &str,
        new: &str,
    ) -> AppResult<()> {
        let su = self.session_user(token).await?;
        let (phash, uname): (String, String) = sqlx::query_as(
            "SELECT password_hash, username FROM users WHERE id = ?",
        )
        .bind(su.user_id)
        .fetch_one(&self.pool)
        .await?;
        if !password::verify_password(current, &phash) {
            return Err(AppError::PermissionDenied(
                "كلمة المرور الحالية غير صحيحة".into(),
            ));
        }
        set_password(&self.pool, su.user_id, new, &uname, false).await?;
        audit::log(
            &self.pool,
            audit::AuditEvent {
                user_id: Some(su.user_id),
                action: "password_changed",
                entity_type: "users",
                entity_id: Some(su.user_id),
                old: None,
                new: None,
                reason: "",
                ctx: None,
            },
        )
        .await
        .ok();
        Ok(())
    }

    /// Admin reset with `users.password_reset`; forces change at next login.
    pub async fn admin_reset_password(
        &self,
        token: &str,
        target_user_id: i64,
        new: &str,
        reason: &str,
    ) -> AppResult<()> {
        let su = self.require(token, "users.password_reset").await?;
        let uname: String = sqlx::query_scalar("SELECT username FROM users WHERE id = ?")
            .bind(target_user_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("المستخدم غير موجود".into()))?;
        set_password(&self.pool, target_user_id, new, &uname, true).await?;
        audit::log(
            &self.pool,
            audit::AuditEvent {
                user_id: Some(su.user_id),
                action: "password_reset",
                entity_type: "users",
                entity_id: Some(target_user_id),
                old: None,
                new: None,
                reason,
                ctx: None,
            },
        )
        .await
        .ok();
        Ok(())
    }

    async fn audit_failure(&self, user_id: Option<i64>, username: &str) {
        audit::log(
            &self.pool,
            audit::AuditEvent {
                user_id,
                action: "login_failed",
                entity_type: "users",
                entity_id: user_id,
                old: None,
                new: None,
                reason: username,
                ctx: None,
            },
        )
        .await
        .ok();
    }
}

// ---------- helpers ----------

fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

async fn create_user_in_tx(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    username: &str,
    full_name: &str,
    pass: &str,
    must_change: bool,
) -> AppResult<i64> {
    validate_new_user(username, full_name, pass)?;
    let hash = password::hash_password(pass)?;
    let r = sqlx::query(
        "INSERT INTO users (username, full_name, password_hash, must_change_password)
         VALUES (?, ?, ?, ?)",
    )
    .bind(username)
    .bind(full_name)
    .bind(hash)
    .bind(must_change as i64)
    .execute(&mut **tx)
    .await?;
    Ok(r.last_insert_rowid())
}

/// Create an additional user (admin flow, Phase 3 management UI later).
pub async fn create_user(
    pool: &SqlitePool,
    acting_user: i64,
    username: &str,
    full_name: &str,
    pass: &str,
    roles: &[&str],
) -> AppResult<i64> {
    let mut tx = pool.begin().await?;
    let id = create_user_in_tx(&mut tx, username, full_name, pass, true).await?;
    for r in roles {
        sqlx::query(
            "INSERT INTO user_roles (user_id, role_id)
             SELECT ?, id FROM roles WHERE code = ?",
        )
        .bind(id)
        .bind(r)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    audit::log(
        pool,
        audit::AuditEvent {
            user_id: Some(acting_user),
            action: "user_created",
            entity_type: "users",
            entity_id: Some(id),
            old: None,
            new: None,
            reason: "",
            ctx: None,
        },
    )
    .await
    .ok();
    Ok(id)
}

async fn set_password(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    new: &str,
    username: &str,
    must_change: bool,
) -> AppResult<()> {
    password::validate_password(new, username)?;
    let hash = password::hash_password(new)?;
    sqlx::query(
        "UPDATE users SET password_hash = ?, must_change_password = ?,
         failed_attempts = 0, locked_until = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(hash)
    .bind(must_change as i64)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn validate_new_user(username: &str, _full_name: &str, pass: &str) -> AppResult<()> {
    if username.trim().len() < 3 {
        return Err(AppError::Validation("اسم المستخدم قصير جداً (3 أحرف على الأقل)".into()));
    }
    if !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-') {
        return Err(AppError::Validation(
            "اسم المستخدم: حروف لاتينية وأرقام و _ . - فقط".into(),
        ));
    }
    password::validate_password(pass, username)
}
