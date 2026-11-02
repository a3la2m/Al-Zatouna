//! Reports (Phase 10): safe report registry, no raw SQL from UI.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

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
