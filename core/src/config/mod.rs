//! App settings stored at the OS-standard config location — the pointer to
//! the current data directory lives here, NEVER inside the install folder
//! (FR-1.6).

pub mod data_dir;

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::data_dir::{default_os_path, is_portable_mode, DataDir};
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_schema_version")]
    pub schema_version: u64,
    /// Chosen data directory root. None = first run not completed yet.
    #[serde(default)]
    pub data_directory: Option<String>,
    #[serde(default)]
    pub portable_mode: bool,
    #[serde(default)]
    pub first_run_completed: bool,
    #[serde(default)]
    pub last_app_version: String,
}

fn default_schema_version() -> u64 {
    1
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: default_schema_version(),
            data_directory: None,
            portable_mode: false,
            first_run_completed: false,
            last_app_version: String::new(),
        }
    }
}

fn settings_file_path() -> Result<PathBuf> {
    // TEST isolation: allow overriding the settings location (used by tests
    // and portable debugging); production path stays the OS-standard one.
    if let Ok(override_path) = std::env::var("EC_SETTINGS_PATH") {
        return Ok(PathBuf::from(override_path));
    }
    dirs::config_dir()
        .map(|c| c.join("education-center").join("settings.json"))
        .ok_or_else(|| AppError::internal("no OS config directory available"))
}

/// Absolute path of the OS settings file (None when no config dir exists).
pub fn settings_path() -> Option<PathBuf> {
    settings_file_path().ok()
}

pub fn load() -> AppSettings {
    let path = match settings_file_path() {
        Ok(p) => p,
        Err(_) => return AppSettings::default(),
    };
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<AppSettings>(&s).ok())
        .unwrap_or_default()
}

pub fn save(settings: &AppSettings) -> Result<()> {
    let path = settings_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings)?;
    fs::write(&path, json)?;
    Ok(())
}

/// Resolution order (docs/ARCHITECTURE.md §6):
/// 1) portable marker next to the executable
/// 2) path recorded in settings
/// 3) default OS path
pub fn resolve_data_dir(settings: &AppSettings) -> Result<(DataDir, bool)> {
    if is_portable_mode() {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|e| Some(e.parent()?.to_path_buf()))
            .ok_or_else(|| AppError::internal("cannot locate executable dir"))?;
        return Ok((DataDir::from_root(exe_dir.join("data")), true));
    }
    if let Some(dir) = &settings.data_directory {
        return Ok((DataDir::from_root(PathBuf::from(dir)), false));
    }
    let def = default_os_path()
        .ok_or_else(|| AppError::internal("no default data directory available"))?;
    Ok((DataDir::from_root(def), false))
}

/// Complete first-run bootstrap: resolve, create tree, open DB, migrate,
/// mark settings. Returns the opened pool. (Wizard calls this after the
/// user picked the path; here it powers the headless test AND the CLI.)
pub async fn bootstrap_first_run(
    chosen_root: Option<&std::path::Path>,
) -> Result<(sqlx::SqlitePool, DataDir)> {
    let mut settings = load();
    let data_dir = match chosen_root {
        Some(p) => DataDir::from_root(p),
        None => resolve_data_dir(&settings)?.0,
    };
    data_dir.create_tree()?;
    data_dir.test_write_permissions()?;

    let pool = crate::database::open_pool(&data_dir.db_path()).await?;
    crate::database::run_migrations(&pool, env!("CARGO_PKG_VERSION")).await?;

    settings.data_directory = Some(data_dir.root().to_string_lossy().to_string());
    settings.portable_mode = is_portable_mode();
    settings.first_run_completed = true;
    settings.last_app_version = env!("CARGO_PKG_VERSION").to_string();
    save(&settings)?;
    Ok((pool, data_dir))
}

/// Startup path for non-first runs: resolve + open + migrate (+ quick check).
/// Change the data directory: backup current DB, copy to new location,
/// verify integrity, update settings. Returns the new pool + DataDir.
/// The caller must drop the old pool before calling.
pub async fn change_data_dir(new_root: &std::path::Path) -> Result<(sqlx::SqlitePool, DataDir)> {
    let settings = load();
    let old_dir = settings.data_directory.clone();
    let new_dd = DataDir::from_root(new_root);
    new_dd.create_tree()?;
    new_dd.test_write_permissions()?;
    // if old db exists, copy it
    if let Some(old) = old_dir {
        let old_db = std::path::Path::new(&old).join("database").join("education_center.db");
        if old_db.exists() {
            let new_db = new_dd.db_path();
            std::fs::create_dir_all(new_db.parent().unwrap())?;
            std::fs::copy(&old_db, &new_db)?;
        }
    }
    let pool = crate::database::open_pool(&new_dd.db_path()).await?;
    crate::database::run_migrations(&pool, env!("CARGO_PKG_VERSION")).await?;
    let check = crate::database::quick_check(&pool).await?;
    if check != "ok" {
        return Err(AppError::storage(format!("quick check failed: {check}")));
    }
    let mut new_settings = settings;
    new_settings.data_directory = Some(new_dd.root().to_string_lossy().to_string());
    new_settings.first_run_completed = true;
    save(&new_settings)?;
    Ok((pool, new_dd))
}

/// Open an *existing* data directory (imported database) and make it the
/// active one. Unlike [`change_data_dir`] this never copies the current DB over
/// the target, so the imported data is preserved.
pub async fn adopt_data_dir(new_root: &std::path::Path) -> Result<(sqlx::SqlitePool, DataDir)> {
    let settings = load();
    let new_dd = DataDir::from_root(new_root);
    new_dd.create_tree()?;
    new_dd.test_write_permissions()?;
    if !new_dd.db_path().exists() {
        return Err(AppError::validation("لا توجد قاعدة بيانات في الأرشيف المستورد"));
    }
    let pool = crate::database::open_pool(&new_dd.db_path()).await?;
    crate::database::run_migrations(&pool, env!("CARGO_PKG_VERSION")).await?;
    let check = crate::database::quick_check(&pool).await?;
    if check != "ok" {
        return Err(AppError::storage(format!("quick check failed: {check}")));
    }
    let mut new_settings = settings;
    new_settings.data_directory = Some(new_dd.root().to_string_lossy().to_string());
    new_settings.first_run_completed = true;
    save(&new_settings)?;
    Ok((pool, new_dd))
}

pub async fn open_data() -> Result<(sqlx::SqlitePool, DataDir)> {
    let settings = load();
    let data_dir = resolve_data_dir(&settings)?.0;
    data_dir.create_tree()?;
    let pool = crate::database::open_pool(&data_dir.db_path()).await?;
    crate::database::run_migrations(&pool, env!("CARGO_PKG_VERSION")).await?;
    let check = crate::database::quick_check(&pool).await?;
    if check != "ok" {
        return Err(AppError::storage(format!(
            "database quick check failed: {check}"
        )));
    }
    Ok((pool, data_dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn first_run_bootstrap_end_to_end() {
        let tmp = TempDir::new().unwrap();
        // isolate the settings file for this test
        let settings_tmp = TempDir::new().unwrap();
        unsafe {
            std::env::set_var(
                "EC_SETTINGS_PATH",
                settings_tmp.path().join("settings.json"),
            );
        }
        let (pool, dd) = bootstrap_first_run(Some(tmp.path())).await.unwrap();
        assert!(dd.db_path().exists());
        assert!(fs::read_to_string(dd.root().join("manifest.json")).is_ok());
        // settings recorded
        assert!(load().data_directory.is_some());
        // database has seeded center settings
        let row = sqlx::query("SELECT COUNT(*) c FROM accounts")
            .fetch_one(&pool).await.unwrap();
        let c = sqlx::Row::get::<i64, _>(&row, "c");
        assert!(c > 20, "seeded chart of accounts, got {}", c);
    }
}
