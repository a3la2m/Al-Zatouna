//! ec-app — thin Tauri 2 shell (Phase 2).
//!
//! Every command handler may only: deserialize input, call ec-core
//! services, and serialize the result. Business logic lives in `ec-core`
//! (ADR-008: permission enforcement and money math never live here).

use tauri::{Manager, State};
use tokio::sync::Mutex;

use ec_core::config::{data_dir::DataDir, AppSettings};
use ec_core::error::AppErrorDto;

/// Managed app state — pool + data directory handle + settings cache.
pub struct AppState {
    pub pool: Mutex<sqlx::SqlitePool>,
    pub data_dir: DataDir,
    pub settings: std::sync::RwLock<AppSettings>,
}

#[tauri::command]
async fn app_info(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let pool = state.pool.lock().await.clone();
    let accounts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);
    Ok(serde_json::json!({
        "app": ec_core::APP_NAME,
        "version": env!("CARGO_PKG_VERSION"),
        "data_dir": state.data_dir.root().to_string_lossy(),
        "accounts_seeded": accounts,
    }))
}

/// First-run wizard step: pick a data directory and bootstrap everything
/// (tree, DB, migrations, settings). Returns the final data-dir path.
#[tauri::command]
async fn wizard_choose_data_dir(
    state: tauri::State<'_, AppState>,
    path: Option<String>,
) -> Result<serde_json::Value, AppErrorDto> {
    let chosen = path.map(std::path::PathBuf::from);
    let result =
        ec_core::config::bootstrap_first_run(chosen.as_deref().map(|p| p.as_path()))
            .await;
    match result {
        Ok((pool, dd)) => {
            *state.pool.lock().await = pool;
            *state.settings.write().unwrap() = ec_core::config::load();
            Ok(serde_json::json!({"data_dir": dd.root().to_string_lossy()}))
        }
        Err(e) => Err(e.into()),
    }
}

async fn app_ready_state() -> Option<AppState> {
    // Try normal startup; if no settings/first-run yet the UI wizard will
    // bootstrap via `wizard_choose_data_dir`. A default empty state keeps
    // the window usable either way.
    match ec_core::config::open_data().await {
        Ok((pool, dd)) => Some(AppState {
            pool: Mutex::new(pool),
            data_dir: dd,
            settings: std::sync::RwLock::new(ec_core::config::load()),
        }),
        // First-run not completed: window shows the wizard.
        Err(_) => None,
    }
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let rt = tauri::async_runtime::handle();
            let state = tauri::async_runtime::block_on(app_ready_state());
            app.manage(state as Option<AppState>);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![app_info, wizard_choose_data_dir])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
