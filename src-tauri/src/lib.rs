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
use ec_core::people::context::Actor;
use ec_core::people::students::{
    create_student, edit_student, list_students, set_student_status, NewStudent, StudentFilter,
    StudentPatch,
};
use ec_core::people::teachers::{
    create_room, create_subject, create_teacher, list_rooms, list_subjects, list_teachers,
    NewTeacher, TeacherPatch,
};

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

// ---------- Phase 4 people commands ----------

impl AppState {
    /// Resolve a session token into an `Actor` for domain services.
    async fn actor(&self, token: &str) -> Result<Actor, AppErrorDto> {
        let auth = self.auth.lock().await;
        let su = auth.session_user(token).await?;
        Ok(Actor {
            user_id: su.user_id,
            username: su.username,
            permissions: su.permissions,
        })
    }
}

#[tauri::command]
async fn students_create(
    state: tauri::State<'_, AppState>,
    token: String,
    branch_id: i64,
    full_name: String,
    national_id: String,
    birth_date: String,
    gender: Option<String>,
    phone: String,
    email: String,
    address: String,
    notes: String,
) -> Result<ec_core::people::students::Student, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(create_student(
        &pool, &actor, branch_id,
        &NewStudent { full_name, national_id, birth_date, gender, phone, email, address, notes },
    )
    .await?)
}

#[tauri::command]
async fn students_list(
    state: tauri::State<'_, AppState>,
    token: String,
    branch_id: i64,
    status: Option<String>,
    search: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<ec_core::people::students::Student>, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(list_students(&pool, &actor, branch_id, &StudentFilter {
        status,
        search,
        limit: limit.unwrap_or(100),
    })
    .await?)
}

#[tauri::command]
async fn students_edit(
    state: tauri::State<'_, AppState>,
    token: String,
    id: i64,
    full_name: Option<String>,
    national_id: Option<String>,
    birth_date: Option<String>,
    gender: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    address: Option<String>,
    notes: Option<String>,
) -> Result<ec_core::people::students::Student, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(edit_student(&pool, &actor, id, &StudentPatch {
        full_name, national_id, birth_date, gender, phone, email, address, notes,
    })
    .await?)
}

#[tauri::command]
async fn students_set_status(
    state: tauri::State<'_, AppState>,
    token: String,
    id: i64,
    status: String,
    reason: String,
) -> Result<ec_core::people::students::Student, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(set_student_status(&pool, &actor, id, &status, &reason).await?)
}

#[tauri::command]
async fn teachers_create(
    state: tauri::State<'_, AppState>,
    token: String,
    branch_id: i64,
    full_name: String,
    phone: String,
    email: String,
    specialty: String,
    qualifications: String,
    address: String,
    hire_date: String,
    notes: String,
) -> Result<ec_core::people::teachers::Teacher, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(create_teacher(&pool, &actor, branch_id, &NewTeacher {
        full_name, phone, email, specialty, qualifications, address, hire_date, notes,
    })
    .await?)
}

#[tauri::command]
async fn teachers_list(
    state: tauri::State<'_, AppState>,
    token: String,
    branch_id: i64,
    status: Option<String>,
    search: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<ec_core::people::teachers::Teacher>, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(list_teachers(&pool, &actor, branch_id, &ec_core::people::teachers::TeacherFilter {
        status,
        search,
        limit: limit.unwrap_or(100),
    })
    .await?)
}

#[tauri::command]
async fn teachers_edit(
    state: tauri::State<'_, AppState>,
    token: String,
    id: i64,
    full_name: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    specialty: Option<String>,
    qualifications: Option<String>,
    address: Option<String>,
    hire_date: Option<String>,
    notes: Option<String>,
) -> Result<ec_core::people::teachers::Teacher, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::people::teachers::edit_teacher(
        &pool,
        &actor,
        id,
        &TeacherPatch { full_name, phone, email, specialty, qualifications, address, hire_date, notes },
    )
    .await?)
}

#[tauri::command]
async fn subjects_create(
    state: tauri::State<'_, AppState>,
    token: String,
    code: String,
    name: String,
    description: String,
) -> Result<ec_core::people::teachers::Subject, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(create_subject(&pool, &actor, &code, &name, &description).await?)
}

#[tauri::command]
async fn subjects_list(
    state: tauri::State<'_, AppState>,
    token: String,
    active_only: bool,
) -> Result<Vec<ec_core::people::teachers::Subject>, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(list_subjects(&pool, &actor, active_only).await?)
}

#[tauri::command]
async fn rooms_create(
    state: tauri::State<'_, AppState>,
    token: String,
    branch_id: i64,
    code: String,
    name: String,
    capacity: i64,
    description: String,
) -> Result<ec_core::people::teachers::Room, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(create_room(&pool, &actor, branch_id, &code, &name, capacity, &description).await?)
}

#[tauri::command]
async fn rooms_list(
    state: tauri::State<'_, AppState>,
    token: String,
    branch_id: i64,
) -> Result<Vec<ec_core::people::teachers::Room>, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(list_rooms(&pool, &actor, branch_id).await?)
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
            auth_change_password,
            students_create,
            students_list,
            students_edit,
            students_set_status,
            teachers_create,
            teachers_list,
            teachers_edit,
            subjects_create,
            subjects_list,
            rooms_create,
            rooms_list
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
