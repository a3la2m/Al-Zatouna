//! ec-app — thin Tauri 2 shell (Phase 2 + Phase 3 auth commands).
//!
//! Every command handler may only: deserialize input, call ec-core
//! services, and serialize the result. Business logic lives in `ec-core`
//! (ADR-008: permission enforcement and money math never live here).

use tauri::Manager;
use tokio::sync::Mutex;

use ec_core::auth::service::{AuthService, LoginResult, SessionUserDto};
use ec_core::config::{data_dir::DataDir, AppSettings};
use ec_core::error::AppErrorDto;

/// Managed app state — pool + data directory handle + settings cache.
/// All swappable pieces sit behind interior mutability so the first-run
/// wizard can rebind them without re-managing state.
pub struct AppState {
    pool: Mutex<sqlx::SqlitePool>,
    data_dir: std::sync::RwLock<DataDir>,
    settings: std::sync::RwLock<AppSettings>,
    /// Auth service (owns the session store). Rebound by the wizard.
    auth: Mutex<AuthService>,
}

impl AppState {
    fn new(pool: sqlx::SqlitePool, dd: DataDir) -> Self {
        Self {
            pool: Mutex::new(pool.clone()),
            data_dir: std::sync::RwLock::new(dd),
            settings: std::sync::RwLock::new(ec_core::config::load()),
            auth: Mutex::new(AuthService::new(pool)),
        }
    }
}

async fn app_ready_state() -> AppState {
    // Normal startup; when first-run has not completed yet we hand back a
    // transient in-memory state and the UI wizard binds the real one.
    match ec_core::config::open_data().await {
        Ok((pool, dd)) => AppState::new(pool, dd),
        Err(_) => {
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_lazy("sqlite::memory:")
                .expect("lazy pool");
            let dd = DataDir::from_root(std::env::temp_dir().join("ec-not-initialized"));
            AppState::new(pool, dd)
        }
    }
}

#[tauri::command]
async fn app_info(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let pool = state.pool.lock().await.clone();
    let accounts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);
    Ok(serde_json::json!({
        "app": ec_core::APP_NAME,
        "version": env!("CARGO_PKG_VERSION"),
        "data_dir": state.data_dir.read().unwrap().root().to_string_lossy(),
        "accounts_seeded": accounts,
        "users": users,
    }))
}

/// First-run wizard step: pick a data directory and bootstrap everything
/// (tree, DB, migrations, settings). Rebinds pool/auth/data-dir in place.
#[tauri::command]
async fn wizard_choose_data_dir(
    state: tauri::State<'_, AppState>,
    path: Option<String>,
) -> Result<serde_json::Value, AppErrorDto> {
    let chosen = path.map(std::path::PathBuf::from);
    let (pool, dd) =
        ec_core::config::bootstrap_first_run(chosen.as_deref().map(std::path::Path::new))
            .await?;
    // swap every live handle to the new database
    let mut auth = state.auth.lock().await;
    auth.pool = pool.clone();
    // a new data dir means fresh sessions (memory store is per-pool era)
    auth.sessions.clear().await;
    drop(auth);
    *state.pool.lock().await = pool;
    *state.data_dir.write().unwrap() = dd;
    *state.settings.write().unwrap() = ec_core::config::load();
    Ok(serde_json::json!({
        "data_dir": state.data_dir.read().unwrap().root().to_string_lossy()
    }))
}

// ---------- Phase 3 auth commands ----------

/// Create the first admin (only valid while no users exist — wizard step 2).
#[tauri::command]
async fn auth_create_admin(
    state: tauri::State<'_, AppState>,
    username: String,
    full_name: String,
    password: String,
) -> Result<serde_json::Value, AppErrorDto> {
    let auth = state.auth.lock().await;
    let id = auth.create_first_admin(&username, &full_name, &password).await?;
    Ok(serde_json::json!({ "user_id": id }))
}

/// Login with username/password → session token + permission set.
#[tauri::command]
async fn auth_login(
    state: tauri::State<'_, AppState>,
    username: String,
    password: String,
) -> Result<LoginResult, AppErrorDto> {
    let auth = state.auth.lock().await;
    Ok(auth.login(&username, &password).await?)
}

/// End the current session.
#[tauri::command]
async fn auth_logout(state: tauri::State<'_, AppState>, token: String) -> Result<(), AppErrorDto> {
    let auth = state.auth.lock().await;
    Ok(auth.logout(&token).await?)
}

/// Current session summary (window focus / inactivity re-check in UI).
#[tauri::command]
async fn auth_me(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<SessionUserDto, AppErrorDto> {
    let auth = state.auth.lock().await;
    let su = auth.session_user(&token).await?;
    Ok(SessionUserDto::from(&su))
}

/// Self-service password change.
#[tauri::command]
async fn auth_change_password(
    state: tauri::State<'_, AppState>,
    token: String,
    current: String,
    new_password: String,
) -> Result<(), AppErrorDto> {
    let auth = state.auth.lock().await;
    Ok(auth.change_password(&token, &current, &new_password).await?)
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let state = tauri::async_runtime::block_on(app_ready_state());
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            wizard_choose_data_dir,
            auth_create_admin,
            auth_login,
            auth_logout,
            auth_me,
            auth_change_password
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
