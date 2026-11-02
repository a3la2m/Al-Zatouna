//! Reports (Phase 10): safe report registry, no raw SQL from UI.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use rust_xlsxwriter::Workbook;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SavedReport {
    pub id: i64,
    pub name: String,
    pub definition_json: String,
}

/// Allowed column codes per entity — the registry rejects unknown codes.
pub fn allowed_columns(entity: &str) -> &'static [&'static str] {
    match entity {
        "students" => &["id", "full_name", "phone", "status", "registered_at"],
        "courses" => &["id", "name", "status", "fee_minor"],
        "enrollments" => &["id", "student_id", "course_id", "status", "net_fee_minor"],
        _ => &[],
    }
}

pub fn is_valid_column(entity: &str, col: &str) -> bool {
    allowed_columns(entity).contains(&col)
}

pub async fn save_report(pool: &SqlitePool, name: &str, definition_json: &str) -> Result<i64, sqlx::Error> {
    // ensure an owner user exists (FK)
    sqlx::query("INSERT OR IGNORE INTO users (id, username, full_name, password_hash) VALUES (0, 'system', 'System', 'x')")
        .execute(pool).await.ok();
    let res = sqlx::query("INSERT INTO saved_reports (report_code, name, owner_id, config_json) VALUES (?, ?, 0, ?)")
        .bind("custom")
        .bind(name)
        .bind(definition_json)
        .execute(pool)
        .await?;
    Ok(res.last_insert_rowid())
}


/// Export a simple report (list of students) to Excel bytes.
pub async fn export_excel_students(pool: &SqlitePool) -> Result<Vec<u8>, sqlx::Error> {
    let rows = sqlx::query("SELECT id, full_name, phone, status FROM students ORDER BY id")
        .fetch_all(pool).await?;
    use sqlx::Row;
    let tmp = std::env::temp_dir().join("ec_export.xlsx");
    let mut wb = Workbook::new(tmp.to_str().unwrap());
    let ws = wb.add_worksheet();
    ws.write_string_only(0, 0, "id").ok();
    ws.write_string_only(0, 1, "full_name").ok();
    ws.write_string_only(0, 2, "phone").ok();
    ws.write_string_only(0, 3, "status").ok();
    for (i, r) in rows.iter().enumerate() {
        ws.write_number_only((i+1) as u32, 0, r.get::<i64,_>("id") as f64).ok();
        ws.write_string_only((i+1) as u32, 1, r.get::<String,_>("full_name").as_str()).ok();
        ws.write_string_only((i+1) as u32, 2, r.get::<String,_>("phone").as_str()).ok();
        ws.write_string_only((i+1) as u32, 3, r.get::<String,_>("status").as_str()).ok();
    }
    wb.close().map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    let bytes = std::fs::read(&tmp).map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    let _ = std::fs::remove_file(&tmp);
    Ok(bytes)
}

pub async fn list_reports(pool: &SqlitePool) -> Result<Vec<SavedReport>, sqlx::Error> {
    let rows = sqlx::query("SELECT id, name, config_json FROM saved_reports")
        .fetch_all(pool)
        .await?;
    use sqlx::Row;
    Ok(rows.iter().map(|r| SavedReport {
        id: r.get("id"),
        name: r.get("name"),
        definition_json: r.get("config_json"),
    }).collect())
}
