//! In-app notification inbox (Phase 22). Durable notifications shown as a bell
//! in the UI and as native desktop notifications. Money-free, simple CRUD.

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::AppResult;

#[derive(Debug, Serialize, Clone)]
pub struct AppNotification {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub ref_type: String,
    pub ref_id: Option<i64>,
    pub person_type: String,
    pub person_id: Option<i64>,
    pub seen: bool,
    pub created_at: String,
}

fn from_row(r: &sqlx::sqlite::SqliteRow) -> AppNotification {
    AppNotification {
        id: r.get("id"), kind: r.get("kind"), title: r.get("title"), body: r.get("body"),
        ref_type: r.get("ref_type"), ref_id: r.get("ref_id"),
        person_type: r.get("person_type"), person_id: r.get("person_id"),
        seen: r.get::<i64, _>("seen") == 1, created_at: r.get("created_at"),
    }
}

pub async fn push(
    pool: &SqlitePool,
    kind: &str,
    title: &str,
    body: &str,
    ref_type: &str,
    ref_id: Option<i64>,
    person_type: &str,
    person_id: Option<i64>,
) -> AppResult<i64> {
    let res = sqlx::query(
        "INSERT INTO app_notifications (kind,title,body,ref_type,ref_id,person_type,person_id)
         VALUES (?,?,?,?,?,?,?)",
    )
    .bind(kind).bind(title).bind(body).bind(ref_type).bind(ref_id)
    .bind(person_type).bind(person_id)
    .execute(pool).await?;
    Ok(res.last_insert_rowid())
}

pub async fn list(pool: &SqlitePool, limit: i64) -> AppResult<Vec<AppNotification>> {
    let limit = limit.clamp(1, 500);
    let rows = sqlx::query("SELECT * FROM app_notifications ORDER BY id DESC LIMIT ?")
        .bind(limit).fetch_all(pool).await?;
    Ok(rows.iter().map(from_row).collect())
}

pub async fn unseen_count(pool: &SqlitePool) -> AppResult<i64> {
    Ok(sqlx::query_scalar("SELECT COUNT(*) FROM app_notifications WHERE seen=0")
        .fetch_one(pool).await?)
}

pub async fn mark_seen(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("UPDATE app_notifications SET seen=1 WHERE id=?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn mark_all_seen(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query("UPDATE app_notifications SET seen=1 WHERE seen=0").execute(pool).await?;
    Ok(())
}

pub async fn clear(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query("DELETE FROM app_notifications").execute(pool).await?;
    Ok(())
}
