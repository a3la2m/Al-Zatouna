//! ec-app — thin Tauri 2 shell (Phase 2 + Phase 3 auth commands).
//!
//! Every command handler may only: deserialize input, call ec-core
//! services, and serialize the result. Business logic lives in `ec-core`
//! (ADR-008: permission enforcement and money math never live here).

mod telegram;

use sqlx::Row;
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
use ec_core::education::courses::{
    create_course, list_courses, NewCourse,
};
use ec_core::education::sessions::{
    create_session, list_sessions, set_session_status, Session,
};
use ec_core::education::attendance::{
    record_student_attendance,
};
use ec_core::vouchers::cash::{close_register, open_register, record_cash_movement};
use ec_core::vouchers::payment_vouchers::{approve_payment_voucher, create_payment_voucher, pay_payment_voucher, PaymentVoucher};
use ec_core::finance::payments::{record_payment, PaymentAllocation};
use ec_core::vouchers::expenses::{approve_expense, create_expense};
use ec_core::reports::{
    execute_report, export_excel_report, list_report_definitions, list_reports,
    save_report, ReportDefinition, ReportResult,
};


/// Global app handle, set once during `setup`. Background loops (session
/// reminders, the Telegram bot) need it to raise desktop notifications.
static APP_HANDLE: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

/// The global app handle, if the app is running.
pub(crate) fn app_handle() -> Option<&'static tauri::AppHandle> {
    APP_HANDLE.get()
}

/// Show a native desktop notification (best effort).
pub(crate) fn desktop_notify(app: &tauri::AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    app.notification().builder().title(title).body(body).show().ok();
}

/// Desktop notification from code that has no handle at hand (background tasks).
pub(crate) fn desktop_notify_global(title: &str, body: &str) {
    if let Some(app) = app_handle() {
        desktop_notify(app, title, body);
    }
}

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

#[tauri::command(rename_all = "snake_case")]
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
#[tauri::command(rename_all = "snake_case")]
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
#[tauri::command(rename_all = "snake_case")]
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
#[tauri::command(rename_all = "snake_case")]
async fn auth_login(
    state: tauri::State<'_, AppState>,
    username: String,
    password: String,
    remember: Option<bool>,
) -> Result<LoginResult, AppErrorDto> {
    let auth = state.auth.lock().await;
    let res = auth.login(&username, &password).await?;
    if remember.unwrap_or(false) {
        auth.remember_token(&res.token, res.user.user_id).await?;
    }
    Ok(res)
}

/// Generate one-time recovery codes (self, or any user for admins).
#[tauri::command(rename_all = "snake_case")]
async fn auth_generate_recovery_codes(
    state: tauri::State<'_, AppState>,
    token: String,
    user_id: i64,
    count: Option<i64>,
) -> Result<Vec<String>, AppErrorDto> {
    let auth = state.auth.lock().await;
    Ok(auth.generate_recovery_codes(&token, user_id, count.unwrap_or(6)).await?)
}

/// Self-service password recovery using a one-time code (no login required).
#[tauri::command(rename_all = "snake_case")]
async fn auth_recover_password(
    state: tauri::State<'_, AppState>,
    username: String,
    code: String,
    new_password: String,
) -> Result<(), AppErrorDto> {
    let auth = state.auth.lock().await;
    Ok(auth.recover_password(&username, &code, &new_password).await?)
}

/// End the current session.
#[tauri::command(rename_all = "snake_case")]
async fn auth_logout(state: tauri::State<'_, AppState>, token: String) -> Result<(), AppErrorDto> {
    let auth = state.auth.lock().await;
    Ok(auth.logout(&token).await?)
}

/// Current session summary (window focus / inactivity re-check in UI).
#[tauri::command(rename_all = "snake_case")]
async fn auth_me(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<SessionUserDto, AppErrorDto> {
    let auth = state.auth.lock().await;
    let su = auth.session_user(&token).await?;
    Ok(SessionUserDto::from(&su))
}

/// Self-service password change.
#[tauri::command(rename_all = "snake_case")]
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

#[tauri::command(rename_all = "snake_case")]
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

#[tauri::command(rename_all = "snake_case")]
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

#[tauri::command(rename_all = "snake_case")]
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

#[tauri::command(rename_all = "snake_case")]
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

#[tauri::command(rename_all = "snake_case")]
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

#[tauri::command(rename_all = "snake_case")]
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

#[tauri::command(rename_all = "snake_case")]
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

#[tauri::command(rename_all = "snake_case")]
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

#[tauri::command(rename_all = "snake_case")]
async fn subjects_list(
    state: tauri::State<'_, AppState>,
    token: String,
    active_only: bool,
) -> Result<Vec<ec_core::people::teachers::Subject>, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(list_subjects(&pool, &actor, active_only).await?)
}

#[tauri::command(rename_all = "snake_case")]
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

#[tauri::command(rename_all = "snake_case")]
async fn rooms_list(
    state: tauri::State<'_, AppState>,
    token: String,
    branch_id: i64,
) -> Result<Vec<ec_core::people::teachers::Room>, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(list_rooms(&pool, &actor, branch_id).await?)
}


#[tauri::command(rename_all = "snake_case")]
async fn courses_list(
    state: tauri::State<'_, AppState>,
    token: String,
    branch_id: i64,
    status: Option<String>,
) -> Result<Vec<ec_core::education::courses::Course>, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(list_courses(&pool, &actor, branch_id, status.as_deref()).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn courses_create(
    state: tauri::State<'_, AppState>,
    token: String,
    branch_id: i64,
    name: String,
    subject_id: i64,
    teacher_id: i64,
    room_id: i64,
    fee_minor: i64,
    is_private: Option<bool>,
) -> Result<ec_core::education::courses::Course, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(create_course(&pool, &actor, branch_id, &NewCourse {
        name, subject_id, teacher_id, room_id, fee_minor, is_private: is_private.unwrap_or(false), ..Default::default()
    }).await?)
}


#[tauri::command(rename_all = "snake_case")]
async fn cash_open_register(
    state: tauri::State<'_, AppState>,
    token: String,
    branch_id: i64,
    code: String,
) -> Result<i64, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(open_register(&pool, &actor, branch_id, &code).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn cash_record_movement(
    state: tauri::State<'_, AppState>,
    token: String,
    register_id: i64,
    kind: String,
    direction: String,
    amount_minor: i64,
    ref_type: String,
) -> Result<ec_core::vouchers::cash::CashMovement, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(record_cash_movement(&pool, &actor, register_id, &kind, &direction, amount_minor, &ref_type, None).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn expenses_create(
    state: tauri::State<'_, AppState>,
    token: String,
    branch_id: i64,
    category_id: i64,
    expense_date: String,
    amount_minor: i64,
    beneficiary: String,
    description: String,
    method: String,
    account_id: i64,
) -> Result<ec_core::vouchers::expenses::Expense, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(create_expense(&pool, &actor, branch_id, category_id, &expense_date, amount_minor, &beneficiary, &description, &method, account_id).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn expenses_approve(
    state: tauri::State<'_, AppState>,
    token: String,
    id: i64,
) -> Result<ec_core::vouchers::expenses::Expense, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(approve_expense(&pool, &actor, id).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn reports_list(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<Vec<ec_core::reports::SavedReport>, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(list_reports(&pool).await.map_err(|e| ec_core::error::AppError::storage(e.to_string()))?)
}

#[tauri::command(rename_all = "snake_case")]
async fn reports_save(
    state: tauri::State<'_, AppState>,
    token: String,
    name: String,
    definition_json: String,
) -> Result<i64, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(save_report(&pool, &name, &definition_json).await.map_err(|e| ec_core::error::AppError::storage(e.to_string()))?)
}


#[tauri::command(rename_all = "snake_case")]
async fn sessions_list(
    state: tauri::State<'_, AppState>,
    token: String,
    course_id: Option<i64>,
) -> Result<Vec<Session>, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(list_sessions(&pool, &actor, course_id, None, None).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn sessions_create(
    state: tauri::State<'_, AppState>,
    token: String,
    course_id: i64,
    session_date: String,
    start_time: String,
    end_time: String,
    room_id: i64,
    teacher_id: i64,
    seq_in_course: i64,
) -> Result<Session, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(create_session(&pool, &actor, course_id, &session_date, &start_time, &end_time, room_id, teacher_id, None, seq_in_course, None).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn sessions_set_status(
    state: tauri::State<'_, AppState>,
    token: String,
    id: i64,
    status: String,
    reason: String,
) -> Result<Session, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(set_session_status(&pool, &actor, id, &status, &reason, None, None).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn attendance_record_student(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    token: String,
    session_id: i64,
    student_id: i64,
    status: String,
    late_minutes: i64,
    notes: Option<String>,
) -> Result<ec_core::education::attendance::StudentAttendance, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    let result = record_student_attendance(&pool, &actor, session_id, student_id, &status, late_minutes, notes.as_deref().unwrap_or("")).await?;
    let label = match status.as_str() { "present"=>"حاضر", "absent"=>"غائب", "late"=>"متأخر", "excused"=>"معذور", _=>&status };
    // in-app notification for managers
    let nbody = format!("تم تسجيل حالة الطالب {} — {}", student_id, label);
    ec_core::notifications::push(&pool, "attendance", "تسجيل حضور", &nbody, "", None, "", None).await.ok();
    desktop_notify(&app, "تسجيل حضور", &nbody);
    // notify the student's linked chat (if any) and the managers
    telegram::notify(&pool, "attendance", "تسجيل حضور",
        &format!("📋 تم تسجيل حالة حضورك: {} في الحصة رقم {}", label, session_id),
        Some("student"), Some(student_id)).await.ok();
    telegram::notify(&pool, "attendance", "تسجيل حضور",
        &format!("تم تسجيل حالة الطالب {} — {}", student_id, label),
        Some("manager"), None).await.ok();
    Ok(result)
}

#[tauri::command(rename_all = "snake_case")]
async fn attendance_record_teacher(
    state: tauri::State<'_, AppState>,
    token: String,
    session_id: i64,
    teacher_id: i64,
    status: String,
    notes: Option<String>,
) -> Result<ec_core::education::attendance::TeacherAttendance, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::education::attendance::record_teacher_attendance(
        &pool, &actor, session_id, teacher_id, &status, notes.as_deref().unwrap_or(""),
    ).await?)
}


#[tauri::command(rename_all = "snake_case")]
async fn payments_record(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    token: String,
    enrollment_id: i64,
    amount_minor: i64,
    idempotency_key: String,
) -> Result<ec_core::finance::payments::PaymentResult, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    let cash: i64 = sqlx::query_scalar("SELECT id FROM accounts LIMIT 1").fetch_one(&pool).await.map_err(ec_core::error::AppError::from)?;
    let recv: i64 = sqlx::query_scalar("SELECT id FROM accounts LIMIT 1 OFFSET 1").fetch_one(&pool).await.map_err(ec_core::error::AppError::from)?;
    let result = record_payment(&pool, &actor, 1, cash, recv, &idempotency_key, &[PaymentAllocation { enrollment_id, amount_minor }], "payment").await?;
    let nbody = format!("تم استلام دفعة بقيمة {:.2} — سند رقم {}", amount_minor as f64/100.0, result.voucher_id);
    ec_core::notifications::push(&pool, "payment", "سند قبض جديد", &nbody, "receipt_voucher", Some(result.voucher_id), "", None).await.ok();
    desktop_notify(&app, "سند قبض جديد", &nbody);
    telegram::notify(&pool, "payment", "سند قبض جديد",
        &format!("تم استلام دفعة بقيمة {:.2} — سند رقم {}", amount_minor as f64/100.0, result.voucher_id),
        None, None).await.ok();
    Ok(result)
}


#[tauri::command(rename_all = "snake_case")]
async fn vouchers_list_receipts(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<Vec<serde_json::Value>, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    let rows = sqlx::query("SELECT id, voucher_no, amount_minor, status, voucher_date FROM receipt_vouchers ORDER BY id DESC")
        .fetch_all(&pool).await.map_err(ec_core::error::AppError::from)?;
    use sqlx::Row;
    Ok(rows.iter().map(|r| serde_json::json!({
        "id": r.get::<i64,_>("id"),
        "voucher_no": r.get::<i64,_>("voucher_no"),
        "amount_minor": r.get::<i64,_>("amount_minor"),
        "status": r.get::<String,_>("status"),
        "voucher_date": r.get::<String,_>("voucher_date"),
    })).collect())
}


#[tauri::command(rename_all = "snake_case")]
async fn auth_disable_user(
    state: tauri::State<'_, AppState>,
    token: String,
    target_user_id: i64,
    reason: String,
) -> Result<(), AppErrorDto> {
    let auth = state.auth.lock().await;
    Ok(auth.disable_user(&token, target_user_id, &reason).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn finance_accrue_enrollment(
    state: tauri::State<'_, AppState>,
    token: String,
    enrollment_id: i64,
    amount_minor: i64,
    receivable_account: i64,
    deferred_account: i64,
) -> Result<i64, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::finance::accrue_enrollment(&pool, &actor, enrollment_id, amount_minor, receivable_account, deferred_account).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn finance_recognize_revenue(
    state: tauri::State<'_, AppState>,
    token: String,
    enrollment_id: i64,
    amount_minor: i64,
    deferred_account: i64,
    revenue_account: i64,
) -> Result<i64, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::finance::recognize_revenue(&pool, &actor, enrollment_id, amount_minor, deferred_account, revenue_account).await?)
}


#[tauri::command(rename_all = "snake_case")]
async fn payment_vouchers_create(
    state: tauri::State<'_, AppState>,
    token: String,
    branch_id: i64,
    beneficiary_type: String,
    teacher_id: Option<i64>,
    purpose: String,
    amount_minor: i64,
    method: String,
    account_id: i64,
    source_type: String,
    source_id: Option<i64>,
    idempotency_key: String,
) -> Result<PaymentVoucher, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(create_payment_voucher(&pool, &actor, branch_id, &beneficiary_type, teacher_id, &purpose, amount_minor, &method, account_id, &source_type, source_id, &idempotency_key).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn payment_vouchers_approve(
    state: tauri::State<'_, AppState>,
    token: String,
    id: i64,
) -> Result<PaymentVoucher, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    let maker: i64 = sqlx::query_scalar("SELECT created_by FROM payment_vouchers WHERE id = ?").bind(id).fetch_one(&pool).await.map_err(ec_core::error::AppError::from)?;
    Ok(approve_payment_voucher(&pool, &actor, id, maker).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn payment_vouchers_pay(
    state: tauri::State<'_, AppState>,
    token: String,
    id: i64,
    payable_account: i64,
    cash_account: i64,
) -> Result<PaymentVoucher, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(pay_payment_voucher(&pool, &actor, id, payable_account, cash_account).await?)
}


// ---------- Phase 16: dashboard, reports, users, settings, cash ----------

fn storage_err(e: impl std::fmt::Display) -> AppErrorDto {
    ec_core::error::AppError::storage(e.to_string()).into()
}

/// Dashboard KPI cards (counts + money, all auto-calculated).
#[tauri::command(rename_all = "snake_case")]
async fn dashboard_kpis(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<ec_core::dashboard::DashboardKpis, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    ec_core::dashboard::compute_kpis(&pool).await.map_err(storage_err)
}

/// Dashboard chart series (collections, expenses, statuses, cash flow).
#[tauri::command(rename_all = "snake_case")]
async fn dashboard_charts(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<ec_core::dashboard::DashboardCharts, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    ec_core::dashboard::compute_charts(&pool).await.map_err(storage_err)
}

/// Full report catalogue (code, title, category, columns) for the picker.
#[tauri::command(rename_all = "snake_case")]
async fn reports_definitions(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<Vec<ReportDefinition>, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    Ok(list_report_definitions())
}

/// Execute one registered report and return its rows as JSON objects.
#[tauri::command(rename_all = "snake_case")]
async fn reports_run(
    state: tauri::State<'_, AppState>,
    token: String,
    code: String,
) -> Result<ReportResult, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(execute_report(&pool, &code).await?)
}

/// Export a registered report to `<data_dir>/exports/…xlsx`, return the path.
#[tauri::command(rename_all = "snake_case")]
async fn reports_export(
    state: tauri::State<'_, AppState>,
    token: String,
    code: String,
) -> Result<String, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    let bytes = export_excel_report(&pool, &code).await?;
    let dir = state.data_dir.read().unwrap().root().join("exports");
    std::fs::create_dir_all(&dir).map_err(storage_err)?;
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let path = dir.join(format!("{code}_{stamp}.xlsx"));
    std::fs::write(&path, bytes).map_err(storage_err)?;
    Ok(path.to_string_lossy().to_string())
}

// ---------- user management ----------

#[tauri::command(rename_all = "snake_case")]
async fn auth_list_users(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<Vec<ec_core::auth::service::UserSummary>, AppErrorDto> {
    let auth = state.auth.lock().await;
    Ok(auth.list_users(&token).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn auth_create_user(
    state: tauri::State<'_, AppState>,
    token: String,
    username: String,
    full_name: String,
    password: String,
    roles: Vec<String>,
) -> Result<i64, AppErrorDto> {
    let auth = state.auth.lock().await;
    let refs: Vec<&str> = roles.iter().map(|s| s.as_str()).collect();
    Ok(auth.create_user_admin(&token, &username, &full_name, &password, &refs).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn auth_reset_password(
    state: tauri::State<'_, AppState>,
    token: String,
    target_user_id: i64,
    new_password: String,
    reason: String,
) -> Result<(), AppErrorDto> {
    let auth = state.auth.lock().await;
    Ok(auth.admin_reset_password(&token, target_user_id, &new_password, &reason).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn auth_enable_user(
    state: tauri::State<'_, AppState>,
    token: String,
    target_user_id: i64,
) -> Result<(), AppErrorDto> {
    let auth = state.auth.lock().await;
    Ok(auth.enable_user(&token, target_user_id).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn auth_list_roles(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<Vec<String>, AppErrorDto> {
    let auth = state.auth.lock().await;
    Ok(auth.list_roles(&token).await?)
}

/// Move the whole data directory (DB + files) to a new root.
/// Requires an authenticated admin; sessions are dropped afterwards.
#[tauri::command(rename_all = "snake_case")]
async fn settings_change_data_dir(
    state: tauri::State<'_, AppState>,
    token: String,
    path: String,
) -> Result<serde_json::Value, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    let new_root = std::path::PathBuf::from(path.trim());
    let current = state.data_dir.read().unwrap().root().to_path_buf();
    if new_root == current {
        return Ok(serde_json::json!({
            "data_dir": current.to_string_lossy(),
            "changed": false,
        }));
    }
    let (pool, dd) = ec_core::config::change_data_dir(&new_root).await?;
    {
        let mut auth = state.auth.lock().await;
        auth.pool = pool.clone();
        auth.sessions.clear().await;
    }
    *state.pool.lock().await = pool;
    *state.data_dir.write().unwrap() = dd;
    *state.settings.write().unwrap() = ec_core::config::load();
    Ok(serde_json::json!({
        "data_dir": state.data_dir.read().unwrap().root().to_string_lossy(),
        "changed": true,
    }))
}

// ---------- lists that the screens were missing ----------

#[tauri::command(rename_all = "snake_case")]
async fn enrollments_list(
    state: tauri::State<'_, AppState>,
    token: String,
    course_id: Option<i64>,
    student_id: Option<i64>,
) -> Result<Vec<ec_core::education::enrollments::Enrollment>, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::education::enrollments::list_enrollments(&pool, &actor, course_id, student_id).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn expenses_list(
    state: tauri::State<'_, AppState>,
    token: String,
    status: Option<String>,
    limit: i64,
) -> Result<Vec<ec_core::vouchers::expenses::Expense>, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::vouchers::expenses::list_expenses(&pool, &actor, status.as_deref(), limit).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn expense_categories_list(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<Vec<ec_core::vouchers::expenses::ExpenseCategory>, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::vouchers::expenses::list_expense_categories(&pool).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn accounts_list(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<Vec<ec_core::finance::ledger::AccountOption>, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    ec_core::finance::ledger::list_accounts(&pool)
        .await
        .map_err(storage_err)
}

// ---------- cash register: auto-calculated balances + reports ----------

#[tauri::command(rename_all = "snake_case")]
async fn cash_registers_list(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<Vec<ec_core::vouchers::cash::CashRegisterRow>, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::vouchers::cash::list_registers(&pool).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn cash_summary(
    state: tauri::State<'_, AppState>,
    token: String,
    register_id: i64,
) -> Result<ec_core::vouchers::cash::CashRegisterSummary, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::vouchers::cash::register_summary(&pool, register_id).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn cash_list_movements(
    state: tauri::State<'_, AppState>,
    token: String,
    register_id: i64,
    limit: i64,
) -> Result<Vec<ec_core::vouchers::cash::CashMovementRow>, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::vouchers::cash::list_movements(&pool, register_id, limit).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn cash_close_register(
    state: tauri::State<'_, AppState>,
    token: String,
    register_id: i64,
    opening_balance_minor: i64,
    counted_balance_minor: i64,
    cash_variance_account: i64,
    cash_account: i64,
) -> Result<i64, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(close_register(
        &pool,
        &actor,
        register_id,
        opening_balance_minor,
        counted_balance_minor,
        cash_variance_account,
        cash_account,
    ).await?)
}


// ---------- attendance sheet + payment vouchers (Phase 16) ----------

/// Enrolled students of a session's course, left-joined with whatever has
/// been recorded — the attendance screen fills this in student by student.
#[tauri::command(rename_all = "snake_case")]
async fn attendance_sheet(
    state: tauri::State<'_, AppState>,
    token: String,
    session_id: i64,
) -> Result<Vec<ec_core::education::attendance::AttendanceSheetRow>, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::education::attendance::attendance_sheet(&pool, &actor, session_id).await?)
}

/// Payment vouchers, newest first (optional status filter).
#[tauri::command(rename_all = "snake_case")]
async fn payment_vouchers_list(
    state: tauri::State<'_, AppState>,
    token: String,
    status: Option<String>,
    limit: i64,
) -> Result<Vec<ec_core::vouchers::payment_vouchers::PaymentVoucherRow>, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::vouchers::payment_vouchers::list_payment_vouchers(
        &pool, &actor, status.as_deref(), limit,
    ).await?)
}

// ---------- course status + enrollments (Phase 16) ----------

#[tauri::command(rename_all = "snake_case")]
async fn courses_set_status(
    state: tauri::State<'_, AppState>,
    token: String,
    id: i64,
    new_status: String,
    reason: String,
) -> Result<ec_core::education::courses::Course, AppErrorDto> {
    // If activating, ensure an active financial agreement exists; if not, create
    // a default 100%-to-center one and approve it with an override reason so a
    // single administrator can complete the flow.
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    if new_status == "active" {
        let has: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM financial_agreements WHERE course_id = ? AND status = 'active'",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .map_err(|e| ec_core::error::AppErrorDto::from(ec_core::AppError::from(e)))?;
        if has == 0 {
            let agr = ec_core::education::agreements::create_agreement_draft(
                &pool, &actor, id, "percent_of_agreed", Some(10000), Some(10000),
                None, None, "{}", "{}",
            ).await.map_err(AppErrorDto::from)?;
            ec_core::education::agreements::approve_agreement(
                &pool, &actor, agr.id, actor.user_id,
                Some("اعتماد ذاتي — مدير النظام"),
            ).await.map_err(AppErrorDto::from)?;
        }
    }
    Ok(ec_core::education::courses::set_course_status(&pool, &actor, id, &new_status, &reason).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn enrollments_enroll(
    state: tauri::State<'_, AppState>,
    token: String,
    student_id: i64,
    course_id: i64,
    base_fee_minor: i64,
    discount_minor: i64,
    waiver_minor: i64,
    enrolled_date: String,
    notes: String,
) -> Result<ec_core::education::enrollments::Enrollment, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::education::enrollments::enroll(
        &pool, &actor, 1, student_id, course_id, base_fee_minor,
        discount_minor, waiver_minor, &enrolled_date, &notes,
    ).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn enrollments_set_status(
    state: tauri::State<'_, AppState>,
    token: String,
    id: i64,
    new_status: String,
    reason: String,
) -> Result<ec_core::education::enrollments::Enrollment, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::education::enrollments::set_enrollment_status(&pool, &actor, id, &new_status, &reason).await?)
}


// ---------- detail views (Phase 17) ----------

#[tauri::command(rename_all = "snake_case")]
async fn details_student(
    state: tauri::State<'_, AppState>,
    token: String,
    student_id: i64,
) -> Result<ec_core::details::StudentDetail, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::details::student_detail(&pool, &actor, student_id).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn details_teacher(
    state: tauri::State<'_, AppState>,
    token: String,
    teacher_id: i64,
) -> Result<ec_core::details::TeacherDetail, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::details::teacher_detail(&pool, &actor, teacher_id).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn details_course(
    state: tauri::State<'_, AppState>,
    token: String,
    course_id: i64,
) -> Result<ec_core::details::CourseDetail, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::details::course_detail(&pool, &actor, course_id).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn details_room(
    state: tauri::State<'_, AppState>,
    token: String,
    room_id: i64,
) -> Result<ec_core::details::RoomDetail, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::details::room_detail(&pool, &actor, room_id).await?)
}


#[tauri::command(rename_all = "snake_case")]
async fn expenses_create_teacher(
    state: tauri::State<'_, AppState>,
    token: String,
    branch_id: i64,
    category_id: i64,
    teacher_id: i64,
    expense_date: String,
    amount_minor: i64,
    description: String,
    method: String,
    account_id: i64,
) -> Result<ec_core::vouchers::expenses::Expense, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::vouchers::expenses::create_teacher_expense(
        &pool, &actor, branch_id, category_id, teacher_id, &expense_date,
        amount_minor, &description, &method, account_id,
    ).await?)
}



// ---------- tiny dependency-free encoders (for inline PDF preview / checksum) ----------

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b = (chunk[0] as u32) << 16
            | (chunk.get(1).copied().unwrap_or(0) as u32) << 8
            | chunk.get(2).copied().unwrap_or(0) as u32;
        out.push(T[((b >> 18) & 63) as usize] as char);
        out.push(T[((b >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { T[((b >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(b & 63) as usize] as char } else { '=' });
    }
    out
}
fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'), b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52), b'+' => Some(62), b'/' => Some(63),
            b'=' => None, _ => None,
        }
    };
    let b = s.as_bytes();
    if b.len() % 4 != 0 { return None; }
    let mut out = Vec::with_capacity(b.len() / 4 * 3);
    let mut i = 0;
    while i < b.len() {
        let a = val(b[i])? as u32;
        let c = val(b[i+1])? as u32;
        let d = if b[i+2] == b'=' { 0 } else { val(b[i+2])? as u32 };
        let e = if b[i+3] == b'=' { 0 } else { val(b[i+3])? as u32 };
        let n = a << 18 | c << 12 | d << 6 | e;
        out.push((n >> 16) as u8);
        if b[i+2] != b'=' { out.push((n >> 8) as u8); }
        if b[i+3] != b'=' { out.push(n as u8); }
        i += 4;
    }
    Some(out)
}
fn fnv1a(data: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}
fn hex_enc(data: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(data.len() * 2);
    for &b in data {
        out.push(H[(b >> 4) as usize] as char);
        out.push(H[(b & 15) as usize] as char);
    }
    out
}


#[tauri::command(rename_all = "snake_case")]
async fn settings_upload_logo(
    state: tauri::State<'_, AppState>,
    token: String,
    filename: String,
    data_base64: String,
) -> Result<String, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    let dir = state.data_dir.read().unwrap().root().to_path_buf();
    let assets = dir.join("assets");
    std::fs::create_dir_all(&assets).map_err(|e| ec_core::AppError::storage(e.to_string()))?;
    let ext = std::path::Path::new(&filename).extension().and_then(|e| e.to_str()).unwrap_or("png");
    let name = format!("logo.{}", ext.to_lowercase());
    let bytes = b64_decode(&data_base64)
        .ok_or_else(|| ec_core::AppError::validation("بيانات الصورة غير صالحة"))?;
    if bytes.is_empty() || bytes.len() > 5 * 1024 * 1024 {
        return Err(ec_core::AppError::validation("حجم الصورة يجب أن يكون أقل من 5MB").into());
    }
    std::fs::write(assets.join(&name), &bytes)
        .map_err(|e| ec_core::AppError::storage(e.to_string()))?;
    Ok(name)
}

// ---------- center settings + printing (Phase 17) ----------

#[tauri::command(rename_all = "snake_case")]
async fn settings_center_get(state: tauri::State<'_, AppState>) -> Result<ec_core::settings_center::CenterSettings, AppErrorDto> {
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::settings_center::get(&pool).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn settings_center_update(
    state: tauri::State<'_, AppState>,
    token: String,
    patch: ec_core::settings_center::CenterSettingsPatch,
) -> Result<ec_core::settings_center::CenterSettings, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::settings_center::update(&pool, &actor, &patch).await?)
}

/// Render a document to PDF and return it as base64 for inline preview.
#[tauri::command(rename_all = "snake_case")]
async fn print_preview(
    state: tauri::State<'_, AppState>,
    token: String,
    document_type: String,
    id: i64,
) -> Result<String, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    let dir = state.data_dir.read().unwrap().root().to_path_buf();
    let doc = match document_type.as_str() {
        "receipt" | "receipt_voucher" => ec_core::documents::build_voucher(&pool, &dir, "receipt", id).await?,
        "payment" | "payment_voucher" => ec_core::documents::build_voucher(&pool, &dir, "payment", id).await?,
        "enrollment" | "enrollment_receipt" => ec_core::documents::build_enrollment_receipt(&pool, &dir, id).await?,
        "teacher_schedule" => ec_core::documents::build_schedule(&pool, &dir, "teacher", id).await?,
        "room_schedule" => ec_core::documents::build_schedule(&pool, &dir, "room", id).await?,
        other => return Err(ec_core::AppError::validation(format!("نوع مستند غير معروف: {}", other)).into()),
    };
    let assets: Vec<(&str, Vec<u8>)> = doc.assets.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
    let png = ec_core::printing::render_png_with_assets(&doc.template, &assets, 1.0)
        .map_err(ec_core::AppError::internal)?;
    Ok(b64(&png))
}

/// Render to PDF, save under <data_dir>/prints, bump print_count and log it.
#[tauri::command(rename_all = "snake_case")]
async fn print_execute(
    state: tauri::State<'_, AppState>,
    token: String,
    document_type: String,
    id: i64,
    copies: i64,
) -> Result<serde_json::Value, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    let dir = state.data_dir.read().unwrap().root().to_path_buf();
    let doc = match document_type.as_str() {
        "receipt" | "receipt_voucher" => ec_core::documents::build_voucher(&pool, &dir, "receipt", id).await?,
        "payment" | "payment_voucher" => ec_core::documents::build_voucher(&pool, &dir, "payment", id).await?,
        "enrollment" | "enrollment_receipt" => ec_core::documents::build_enrollment_receipt(&pool, &dir, id).await?,
        "teacher_schedule" => ec_core::documents::build_schedule(&pool, &dir, "teacher", id).await?,
        "room_schedule" => ec_core::documents::build_schedule(&pool, &dir, "room", id).await?,
        other => return Err(ec_core::AppError::validation(format!("نوع مستند غير معروف: {}", other)).into()),
    };
    let assets: Vec<(&str, Vec<u8>)> = doc.assets.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
    let pdf = ec_core::printing::render_pdf_with_assets(&doc.template, &assets)
        .map_err(ec_core::AppError::internal)?;
    let prints_dir = dir.join("prints");
    std::fs::create_dir_all(&prints_dir).ok();
    let name = format!("{}-{}-{}.pdf", document_type.replace('_', "-"), id, chrono::Utc::now().format("%Y%m%d%H%M%S"));
    let path = prints_dir.join(&name);
    std::fs::write(&path, &pdf).map_err(|e| ec_core::AppError::internal(e.to_string()))?;
    // bump print_count on the voucher (best effort)
    if document_type == "teacher_schedule" || document_type == "room_schedule" {
        // schedule documents are not vouchers — nothing to bump
    } else if document_type.contains("receipt") || document_type == "enrollment" {
        sqlx::query("UPDATE receipt_vouchers SET print_count=print_count+?, last_printed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?")
            .bind(copies).bind(id).execute(&pool).await.ok();
    } else {
        sqlx::query("UPDATE payment_vouchers SET print_count=print_count+1 WHERE id=?")
            .bind(id).execute(&pool).await.ok();
    }
    let checksum = hex_enc(&fnv1a(&pdf).to_le_bytes());
    let code = match document_type.as_str() {
        "payment" | "payment_voucher" => "voucher.payment",
        "enrollment" | "enrollment_receipt" => "enrollment.receipt",
        "teacher_schedule" | "room_schedule" => "schedule.study",
        _ => "voucher.receipt",
    };
    ec_core::printing::log_print(&pool, actor.user_id, &document_type, id, code, "1", "print",
        path.to_string_lossy().as_ref(), &checksum).await.map_err(|e| ec_core::AppError::storage(e.to_string()))?;
    // open the PDF with the system's default viewer
    open_in_os(&path.to_string_lossy()).ok();
    Ok(serde_json::json!({ "path": path.to_string_lossy(), "pages": ec_core::printing::page_count(&doc.template).unwrap_or(1) }))
}

#[tauri::command(rename_all = "snake_case")]
async fn print_profiles_list(state: tauri::State<'_, AppState>) -> Result<Vec<serde_json::Value>, AppErrorDto> {
    let pool = state.pool.lock().await.clone();
    let rows = sqlx::query("SELECT code, name, doc_type, paper, orientation, font_family, font_size_pt, show_header, show_footer, watermark, copies, is_default FROM print_profiles ORDER BY code")
        .fetch_all(&pool).await.map_err(|e| ec_core::AppError::storage(e.to_string()))?;
    use sqlx::Row;
    Ok(rows.iter().map(|r| serde_json::json!({
        "code": r.get::<String,_>("code"), "name": r.get::<String,_>("name"),
        "doc_type": r.try_get::<String,_>("doc_type").unwrap_or_default(),
        "show_header": r.get::<i64,_>("show_header"), "show_footer": r.get::<i64,_>("show_footer"),
        "font_size_pt": r.get::<i64,_>("font_size_pt"), "copies": r.get::<i64,_>("copies"),
    })).collect())
}


// ---------- weekly schedule + session generation (Phase 18) ----------

#[tauri::command(rename_all = "snake_case")]
async fn schedule_add(
    state: tauri::State<'_, AppState>, token: String, course_id: i64, weekday: i64,
    start_time: String, end_time: String, room_id: i64, teacher_id: i64,
    effective_from: String, effective_to: String,
) -> Result<ec_core::education::schedule::ScheduleRule, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::education::schedule::add_rule(&pool, &actor, course_id, weekday,
        &start_time, &end_time, room_id, teacher_id, &effective_from, &effective_to).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn schedule_list(
    state: tauri::State<'_, AppState>, token: String, course_id: Option<i64>,
) -> Result<Vec<ec_core::education::schedule::ScheduleRule>, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::education::schedule::list_rules(&pool, &actor, course_id).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn schedule_delete(state: tauri::State<'_, AppState>, token: String, id: i64) -> Result<(), AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::education::schedule::delete_rule(&pool, &actor, id).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn schedule_check_clashes(
    state: tauri::State<'_, AppState>, token: String, course_id: i64, weekday: i64,
    start_time: String, end_time: String, room_id: i64, teacher_id: i64,
) -> Result<Vec<ec_core::education::schedule::Clash>, AppErrorDto> {
    let _actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::education::schedule::check_rule_clashes(&pool, course_id, weekday,
        &start_time, &end_time, room_id, teacher_id).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn sessions_generate(
    state: tauri::State<'_, AppState>, token: String, course_id: i64,
    from: String, to: String, commit: bool,
) -> Result<Vec<ec_core::education::schedule::GeneratedSession>, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::education::schedule::generate_sessions(&pool, &actor, course_id,
        &from, &to, commit).await?)
}




#[tauri::command(rename_all = "snake_case")]
async fn payments_record_service(
    state: tauri::State<'_, AppState>, app: tauri::AppHandle, token: String, payer_name: String,
    description: String, amount_minor: i64, method: String, idempotency_key: String,
) -> Result<i64, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    let cash = sqlx::query_scalar("SELECT id FROM accounts WHERE code='1000' LIMIT 1")
        .fetch_one(&pool).await.map_err(ec_core::error::AppError::from)?;
    let revenue = sqlx::query_scalar("SELECT id FROM accounts WHERE code='4000' LIMIT 1")
        .fetch_one(&pool).await.map_err(ec_core::error::AppError::from)?;
    let id = ec_core::finance::payments::record_service_sale(
        &pool, &actor, 1, cash, revenue, &payer_name, &description, amount_minor,
        &method, &idempotency_key,
    ).await?;
    let nbody = format!("{} — {:.2} من {}", description, amount_minor as f64/100.0, payer_name);
    ec_core::notifications::push(&pool, "payment", "قبض خدمة إضافية", &nbody, "receipt_voucher", Some(id), "", None).await.ok();
    desktop_notify(&app, "قبض خدمة إضافية", &nbody);
    telegram::notify(&pool, "payment", "قبض خدمة إضافية",
        &format!("تم استلام {:.2} مقابل {} — سند {}", amount_minor as f64/100.0, description, id),
        None, None).await.ok();
    Ok(id)
}





#[tauri::command(rename_all = "snake_case")]
async fn telegram_service_requests(
    state: tauri::State<'_, AppState>, token: String, status: Option<String>,
) -> Result<Vec<telegram::ServiceRequest>, AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(telegram::list_service_requests(&pool, status.as_deref()).await.map_err(ec_core::AppError::internal)?)
}

#[tauri::command(rename_all = "snake_case")]
async fn telegram_respond_service(
    state: tauri::State<'_, AppState>, token: String, id: i64, approve: bool, response: String,
) -> Result<(), AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(telegram::respond_service_request(&pool, id, approve, &response, actor.user_id).await.map_err(ec_core::AppError::internal)?)
}




// ---------- window helpers ----------

/// Reset webview zoom to 100%. Fixes accidental pinch/ctrl zoom getting stuck.
#[tauri::command]
fn window_reset_zoom(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        w.set_zoom(1.0).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ---------- shareable data bundles (Phase 24) ----------

/// Export a compressed bundle (database snapshot + settings + assets).
/// When `dest` is omitted a native "save as" dialog is shown.
#[tauri::command(rename_all = "snake_case")]
async fn backup_export_archive(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    token: String,
    dest: Option<String>,
) -> Result<String, AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    let data_dir = state.data_dir.read().unwrap().root().to_path_buf();

    let target = match dest.filter(|d| !d.trim().is_empty()) {
        Some(d) => std::path::PathBuf::from(d.trim()),
        None => {
            use tauri_plugin_dialog::DialogExt;
            let picker = app
                .dialog()
                .file()
                .set_file_name(ec_core::archive::suggested_name())
                .add_filter("أرشيف الزيتونة", &["zip"]);
            match picker.blocking_save_file() {
                Some(p) => p
                    .into_path()
                    .map_err(|e| ec_core::AppError::internal(e.to_string()))?,
                None => return Err(ec_core::AppError::validation("أُلغي حفظ الأرشيف").into()),
            }
        }
    };

    let out = ec_core::archive::export_bundle(&pool, &data_dir, &target)
        .await
        .map_err(ec_core::AppError::from)?;
    Ok(out.to_string_lossy().to_string())
}

/// Import another database/archive and switch the app to it. Sessions are
/// cleared, so the caller must return to the login screen.
#[tauri::command(rename_all = "snake_case")]
async fn backup_import_archive(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    token: String,
    path: Option<String>,
) -> Result<serde_json::Value, AppErrorDto> {
    state.actor(&token).await?;

    let src = match path.filter(|p| !p.trim().is_empty()) {
        Some(p) => std::path::PathBuf::from(p.trim()),
        None => {
            use tauri_plugin_dialog::DialogExt;
            let picker = app
                .dialog()
                .file()
                .add_filter("أرشيف أو قاعدة بيانات", &["zip", "db", "sqlite", "sqlite3"]);
            match picker.blocking_pick_file() {
                Some(p) => p
                    .into_path()
                    .map_err(|e| ec_core::AppError::internal(e.to_string()))?,
                None => return Err(ec_core::AppError::validation("أُلغي الاستيراد").into()),
            }
        }
    };

    let current = state.data_dir.read().unwrap().root().to_path_buf();
    let parent = current
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(std::env::temp_dir);
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let dest_root = parent.join(format!("zaytouna-import-{stamp}"));

    ec_core::archive::import_bundle(&src, &dest_root).map_err(ec_core::AppError::from)?;
    let (pool, dd) = ec_core::config::adopt_data_dir(&dest_root)
        .await
        .map_err(ec_core::AppError::from)?;

    {
        let mut auth = state.auth.lock().await;
        auth.pool = pool.clone();
        auth.sessions.clear().await;
    }
    *state.pool.lock().await = pool;
    *state.data_dir.write().unwrap() = dd.clone();
    *state.settings.write().unwrap() = ec_core::config::load();

    Ok(serde_json::json!({
        "data_dir": dd.root().to_string_lossy(),
        "imported": true,
    }))
}


fn open_in_os(target: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        std::process::Command::new("cmd")
            .args(["/C", "start", "", target])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(target)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(target)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Reveal a file or folder in the OS file manager.
#[tauri::command(rename_all = "snake_case")]
async fn open_path(
    state: tauri::State<'_, AppState>,
    token: String,
    path: String,
) -> Result<(), AppErrorDto> {
    state.actor(&token).await?;
    let p = std::path::PathBuf::from(path.trim());
    if !p.exists() {
        return Err(ec_core::AppError::validation("المسار غير موجود").into());
    }
    open_in_os(&p.to_string_lossy()).map_err(ec_core::AppError::internal)?;
    Ok(())
}

// ---------- my account: phone + Telegram linking (Phase 23) ----------

#[derive(serde::Serialize)]
struct AccountTelegramStatus {
    user_id: i64,
    full_name: String,
    username: String,
    phone: String,
    bot_username: String,
    bot_enabled: bool,
    linked: bool,
    chat_id: Option<String>,
    linked_at: Option<String>,
    link_active: Option<bool>,
}

#[tauri::command(rename_all = "snake_case")]
async fn account_telegram_status(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<AccountTelegramStatus, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    let row = sqlx::query("SELECT full_name, username, phone FROM users WHERE id=?")
        .bind(actor.user_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| ec_core::AppError::internal(e.to_string()))?;
    let link = telegram::link_for_person(&pool, "manager", actor.user_id)
        .await
        .map_err(ec_core::AppError::internal)?;
    let bot_username = telegram::setting(&pool, "tg_bot_username", "").await;
    let enabled = telegram::setting(&pool, "tg_enabled", "0").await;
    Ok(AccountTelegramStatus {
        user_id: actor.user_id,
        full_name: row.get("full_name"),
        username: row.get("username"),
        phone: row.get("phone"),
        bot_username,
        bot_enabled: enabled == "1" || enabled.eq_ignore_ascii_case("true"),
        linked: link.is_some(),
        chat_id: link.as_ref().map(|l| l.chat_id.clone()),
        linked_at: link.as_ref().map(|l| l.linked_at.clone()),
        link_active: link.as_ref().map(|l| l.active),
    })
}

#[tauri::command(rename_all = "snake_case")]
async fn account_phone_set(
    state: tauri::State<'_, AppState>,
    token: String,
    phone: String,
) -> Result<String, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    let phone = phone.trim().to_string();
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    if !phone.is_empty() && digits.len() < 7 {
        return Err(ec_core::AppError::validation("رقم الجوال غير صحيح").into());
    }
    sqlx::query(
        "UPDATE users SET phone=?, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?",
    )
    .bind(&phone)
    .bind(actor.user_id)
    .execute(&pool)
    .await
    .map_err(|e| ec_core::AppError::internal(e.to_string()))?;
    // keep an existing bot link in sync
    sqlx::query("UPDATE telegram_chats SET phone=? WHERE person_type='manager' AND person_id=?")
        .bind(telegram::normalize_phone(&phone))
        .bind(actor.user_id)
        .execute(&pool)
        .await
        .ok();
    Ok(phone)
}

#[tauri::command(rename_all = "snake_case")]
async fn account_telegram_link_code(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<String, AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(telegram::create_link_code(&pool, "manager", actor.user_id, 10)
        .await
        .map_err(ec_core::AppError::internal)?)
}

#[tauri::command(rename_all = "snake_case")]
async fn account_telegram_unlink(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<(), AppErrorDto> {
    let actor = state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    let link = telegram::link_for_person(&pool, "manager", actor.user_id)
        .await
        .map_err(ec_core::AppError::internal)?;
    telegram::unlink_person(&pool, "manager", actor.user_id)
        .await
        .map_err(ec_core::AppError::internal)?;
    // drop the chat from the bot-admin list too
    if let Some(l) = link {
        let mg = telegram::setting(&pool, "tg_manager_chats", "").await;
        let kept: Vec<&str> = mg
            .split(',')
            .map(|c| c.trim())
            .filter(|c| !c.is_empty() && *c != l.chat_id)
            .collect();
        telegram::set_setting(&pool, "tg_manager_chats", &kept.join(","))
            .await
            .ok();
    }
    Ok(())
}

/// Open an external URL in the user's browser (used for t.me deep links).
#[tauri::command(rename_all = "snake_case")]
async fn open_external_url(
    state: tauri::State<'_, AppState>,
    token: String,
    url: String,
) -> Result<(), AppErrorDto> {
    state.actor(&token).await?;
    let url = url.trim().to_string();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(ec_core::AppError::validation("رابط غير صالح").into());
    }
    open_in_os(&url).map_err(ec_core::AppError::internal)?;
    Ok(())
}

// ---------- update check (GitHub Releases) ----------

fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts
        .next()
        .unwrap_or("0")
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .unwrap_or("0")
        .parse()
        .ok()?;
    Some((major, minor, patch))
}

fn semver_is_newer(latest: &str, current: &str) -> bool {
    match (parse_semver(latest), parse_semver(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// Check GitHub releases for a newer version. No auth token required.
#[tauri::command(rename_all = "snake_case")]
async fn check_for_update() -> Result<serde_json::Value, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, String> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(format!("Al-Zatouna/{}", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| e.to_string())?;
        let resp = client
            .get("https://api.github.com/repos/a3la2m/Al-Zatouna/releases/latest")
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Ok(serde_json::json!({
                "available": false,
                "current": env!("CARGO_PKG_VERSION"),
                "reason": format!("http_{}", resp.status().as_u16())
            }));
        }
        let body: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
        let tag = body.get("tag_name").and_then(|v| v.as_str()).unwrap_or("");
        let latest = tag.trim_start_matches('v').to_string();
        let url = body
            .get("html_url")
            .and_then(|v| v.as_str())
            .unwrap_or("https://github.com/a3la2m/Al-Zatouna/releases/latest")
            .to_string();
        let notes = body
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .chars()
            .take(500)
            .collect::<String>();
        let published_at = body
            .get("published_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let available = semver_is_newer(&latest, env!("CARGO_PKG_VERSION"));
        Ok(serde_json::json!({
            "available": available,
            "current": env!("CARGO_PKG_VERSION"),
            "latest": latest,
            "tag": tag,
            "url": url,
            "notes": notes,
            "published_at": published_at
        }))
    })
    .await
    .map_err(|e| e.to_string())??;

    let _ = current;
    Ok(result)
}

/// Open a GitHub release page for this app (no session token required).
#[tauri::command(rename_all = "snake_case")]
async fn open_release_page(url: String) -> Result<(), String> {
    let url = url.trim().to_string();
    let allowed = url.starts_with("https://github.com/a3la2m/Al-Zatouna/releases")
        || url == "https://github.com/a3la2m/Al-Zatouna/releases/latest";
    if !allowed {
        return Err("رابط تحديث غير مسموح".to_string());
    }
    open_in_os(&url)
}

// ---------- in-app notifications (Phase 22) ----------

#[tauri::command(rename_all = "snake_case")]
async fn notifications_list(state: tauri::State<'_, AppState>, token: String, limit: Option<i64>) -> Result<Vec<ec_core::notifications::AppNotification>, AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::notifications::list(&pool, limit.unwrap_or(100)).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn notifications_unseen(state: tauri::State<'_, AppState>, token: String) -> Result<i64, AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::notifications::unseen_count(&pool).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn notifications_mark_seen(state: tauri::State<'_, AppState>, token: String, id: i64) -> Result<(), AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::notifications::mark_seen(&pool, id).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn notifications_mark_all(state: tauri::State<'_, AppState>, token: String) -> Result<(), AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::notifications::mark_all_seen(&pool).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn notifications_clear(state: tauri::State<'_, AppState>, token: String) -> Result<(), AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(ec_core::notifications::clear(&pool).await?)
}

// ---------- Telegram bot (Phase 20) ----------

#[tauri::command(rename_all = "snake_case")]
async fn telegram_get_settings(state: tauri::State<'_, AppState>, token: String) -> Result<serde_json::Value, AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    let mut m = serde_json::Map::new();
    let keys = [
        "tg_enabled", "tg_token", "tg_manager_chats", "tg_queries_enabled",
        "tg_notify_payment", "tg_notify_attendance", "tg_notify_session",
        "tg_notify_expense", "tg_notify_other",
    ];
    for k in keys {
        let default = if k == "tg_enabled" { "0" } else { "1" };
        m.insert(k.to_string(), serde_json::json!(telegram::setting(&pool, k, default).await));
    }
    Ok(serde_json::Value::Object(m))
}

#[tauri::command(rename_all = "snake_case")]
async fn telegram_save_settings(
    state: tauri::State<'_, AppState>, token: String, values: serde_json::Value,
) -> Result<(), AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    if let Some(obj) = values.as_object() {
        for (k, v) in obj {
            if k.starts_with("tg_") {
                let val = v.as_str().unwrap_or("");
                telegram::set_setting(&pool, k, val).await.map_err(ec_core::AppError::internal)?;
            }
        }
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn telegram_test(state: tauri::State<'_, AppState>, token: String) -> Result<String, AppErrorDto> {
    state.actor(&token).await?;
    // test uses the saved token (UI saves it first)
    let pool = state.pool.lock().await.clone();
    let bot_token = telegram::setting(&pool, "tg_token", "").await;
    if bot_token.trim().is_empty() {
        return Err(ec_core::AppError::validation("أدخل رمز البوت أولاً").into());
    }
    let result = tauri::async_runtime::spawn_blocking(move || telegram::test_bot(&bot_token)).await;
    match result {
        Ok(Ok(s)) => {
            // remember the bot handle so «حسابي» can build a t.me deep link
            if let Some(rest) = s.split('@').nth(1) {
                let uname: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !uname.is_empty() {
                    telegram::set_setting(&pool, "tg_bot_username", &uname).await.ok();
                }
            }
            Ok(s)
        }
        Ok(Err(e)) => Err(ec_core::AppError::internal(e).into()),
        Err(e) => Err(ec_core::AppError::internal(e.to_string()).into()),
    }
}

#[tauri::command(rename_all = "snake_case")]
async fn telegram_links(state: tauri::State<'_, AppState>, token: String) -> Result<Vec<telegram::ChatLink>, AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(telegram::list_links(&pool).await.map_err(ec_core::AppError::internal)?)
}

#[tauri::command(rename_all = "snake_case")]
async fn telegram_link_manually(
    state: tauri::State<'_, AppState>, token: String, chat_id: String,
    person_type: String, person_id: i64, phone: String,
) -> Result<(), AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    // validate person exists
    let table = match person_type.as_str() {
        "student" => "students", "teacher" => "teachers", "manager" => "users",
        _ => return Err(ec_core::AppError::validation("نوع الشخص غير صالح").into()),
    };
    let exists: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table} WHERE id=?"))
        .bind(person_id).fetch_one(&pool).await
        .map_err(|e| ec_core::AppError::storage(e.to_string()))?;
    if exists == 0 { return Err(ec_core::AppError::not_found("الشخص غير موجود").into()); }
    sqlx::query("DELETE FROM telegram_chats WHERE chat_id=? OR (person_type=? AND person_id=?)")
        .bind(&chat_id).bind(&person_type).bind(person_id).execute(&pool).await
        .map_err(|e| ec_core::AppError::storage(e.to_string()))?;
    sqlx::query("INSERT INTO telegram_chats (chat_id,person_type,person_id,phone) VALUES (?,?,?,?)")
        .bind(&chat_id).bind(&person_type).bind(person_id).bind(&phone)
        .execute(&pool).await.map_err(|e| ec_core::AppError::storage(e.to_string()))?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn telegram_unlink(state: tauri::State<'_, AppState>, token: String, id: i64) -> Result<(), AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(telegram::unlink(&pool, id).await.map_err(ec_core::AppError::internal)?)
}

#[tauri::command(rename_all = "snake_case")]
async fn telegram_set_active(state: tauri::State<'_, AppState>, token: String, id: i64, active: bool) -> Result<(), AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(telegram::set_active(&pool, id, active).await.map_err(ec_core::AppError::internal)?)
}

#[tauri::command(rename_all = "snake_case")]
async fn telegram_messages(
    state: tauri::State<'_, AppState>, token: String, chat: Option<String>,
    kind: Option<String>, search: Option<String>, limit: Option<i64>,
) -> Result<Vec<telegram::ChatMessage>, AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(telegram::list_messages(&pool, chat.as_deref(), kind.as_deref(), search.as_deref(), limit.unwrap_or(300))
        .await.map_err(ec_core::AppError::internal)?)
}

#[tauri::command(rename_all = "snake_case")]
async fn telegram_broadcast(
    state: tauri::State<'_, AppState>, token: String, text: String, target: String,
) -> Result<i64, AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    Ok(telegram::broadcast(&pool, &text, &target).await.map_err(ec_core::AppError::internal)?)
}

#[tauri::command(rename_all = "snake_case")]
async fn telegram_notify_test(
    state: tauri::State<'_, AppState>, token: String, person_type: String, person_id: i64,
) -> Result<i64, AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    let text = "هذه رسالة تجريبية من نظام إدارة المركز التعليمي ✅";
    Ok(telegram::notify(&pool, "test", "رسالة تجريبية", text, Some(&person_type), Some(person_id))
        .await.map_err(ec_core::AppError::internal)?)
}

#[tauri::command(rename_all = "snake_case")]
async fn telegram_stats(state: tauri::State<'_, AppState>, token: String) -> Result<serde_json::Value, AppErrorDto> {
    state.actor(&token).await?;
    let pool = state.pool.lock().await.clone();
    let links: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM telegram_chats WHERE active=1")
        .fetch_one(&pool).await.map_err(|e| ec_core::AppError::storage(e.to_string()))?;
    let messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM telegram_messages")
        .fetch_one(&pool).await.map_err(|e| ec_core::AppError::storage(e.to_string()))?;
    let notifications: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM telegram_notifications")
        .fetch_one(&pool).await.map_err(|e| ec_core::AppError::storage(e.to_string()))?;
    Ok(serde_json::json!({ "links": links, "messages": messages, "notifications": notifications }))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // A second launch should bring the existing (possibly hidden) window forward.
            use tauri::Manager;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // expose the handle to background tasks (reminders, bot alerts)
            let _ = APP_HANDLE.set(app.handle().clone());
            let state = tauri::async_runtime::block_on(app_ready_state());
            // start the Telegram polling bot with a clone of the active pool
            let bot_pool = tauri::async_runtime::block_on(state.pool.lock()).clone();
            telegram::spawn_bot(bot_pool.clone());
            telegram::spawn_reminders(bot_pool);
            app.manage(state);

            // ---- system tray: keep the app alive when the window is closed ----
            use tauri::menu::{Menu, MenuItem};
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder};
            use tauri::Manager;
            let show_i = MenuItem::with_id(app, "show", "فتح الزيتونة", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "خروج من البرنامج", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;
            let tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("الزيتونة — نظام إدارة المركز التعليمي")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;
            app.manage(tray);

            // ---- hide (don't exit) when the window close button is pressed ----
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = w.hide();
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            window_reset_zoom,
            app_info,
            wizard_choose_data_dir,
            auth_create_admin,
            auth_login,
            auth_generate_recovery_codes,
            auth_recover_password,
            auth_logout,
            auth_me,
            auth_change_password,
            auth_disable_user,
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
            rooms_list,
            courses_list,
            courses_create,
            cash_open_register,
            cash_record_movement,
            expenses_create,
            expenses_approve,
            reports_list,
            reports_save,
            sessions_list,
            sessions_create,
            sessions_set_status,
            attendance_record_student,
            attendance_record_teacher,
            payments_record,
            payment_vouchers_create,
            payment_vouchers_approve,
            payment_vouchers_pay,
            finance_accrue_enrollment,
            finance_recognize_revenue,
            vouchers_list_receipts,
            dashboard_kpis,
            dashboard_charts,
            reports_definitions,
            reports_run,
            reports_export,
            auth_list_users,
            auth_create_user,
            auth_reset_password,
            auth_enable_user,
            auth_list_roles,
            settings_change_data_dir,
            enrollments_list,
            enrollments_enroll,
            enrollments_set_status,
            courses_set_status,
            details_student,
            details_teacher,
            details_course,
            details_room,
            expenses_create_teacher,
            settings_upload_logo,
            settings_center_get,
            settings_center_update,
            print_preview,
            print_execute,
            print_profiles_list,
            schedule_add,
            schedule_list,
            schedule_delete,
            schedule_check_clashes,
            sessions_generate,
            expenses_list,
            expense_categories_list,
            cash_registers_list,
            cash_summary,
            cash_list_movements,
            cash_close_register,
            accounts_list,
            attendance_sheet,
            payment_vouchers_list,
            payments_record_service,
            telegram_service_requests, telegram_respond_service,
            backup_export_archive, backup_import_archive, open_path,
            account_telegram_status, account_phone_set, account_telegram_link_code,
            account_telegram_unlink, open_external_url,
            check_for_update, open_release_page,
            notifications_list, notifications_unseen, notifications_mark_seen,
            notifications_mark_all, notifications_clear,
            telegram_get_settings, telegram_save_settings, telegram_test,
            telegram_links, telegram_link_manually, telegram_unlink,
            telegram_set_active, telegram_messages, telegram_broadcast,
            telegram_notify_test, telegram_stats
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
