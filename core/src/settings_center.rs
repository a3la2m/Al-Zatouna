//! Center identity & print settings (Phase 17).
//! Reads/writes the single `center_settings` row that the printing engine uses
//! for header / footer / logo. All writes require `settings.manage`.

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::people::context::Actor;
use crate::AppResult;

#[derive(Debug, Serialize, Clone)]
pub struct CenterSettings {
    pub id: i64,
    pub name: String,
    pub trade_name: String,
    pub logo_ref: String,
    pub address: String,
    pub phones: String,
    pub email: String,
    pub website: String,
    pub license_no: String,
    pub tax_no: String,
    pub currency_code: String,
    pub currency_decimals: i64,
    pub timezone: String,
    pub academic_year: String,
    pub header_text: String,
    pub footer_text: String,
}

/// A patch from the UI — `None` means "leave unchanged".
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case", default)]
pub struct CenterSettingsPatch {
    pub name: Option<String>,
    pub trade_name: Option<String>,
    pub logo_ref: Option<String>,
    pub address: Option<String>,
    pub phones: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub license_no: Option<String>,
    pub tax_no: Option<String>,
    pub currency_code: Option<String>,
    pub currency_decimals: Option<i64>,
    pub timezone: Option<String>,
    pub academic_year: Option<String>,
    pub header_text: Option<String>,
    pub footer_text: Option<String>,
}

fn s(r: &sqlx::sqlite::SqliteRow, col: &str) -> String {
    r.get::<String, _>(col)
}

/// Accept plain text, a JSON string, `{"text": ...}` or `{"lines": [...]}`.
pub fn json_to_text(raw: &str) -> String {
    let t = raw.trim();
    if t.is_empty() || t == "{}" {
        return String::new();
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(t) {
        match v {
            serde_json::Value::String(s) => return s,
            serde_json::Value::Object(m) => {
                if let Some(x) = m.get("text").and_then(|x| x.as_str()) {
                    return x.to_string();
                }
                if let Some(lines) = m.get("lines").and_then(|x| x.as_array()) {
                    return lines
                        .iter()
                        .filter_map(|x| x.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                }
                return String::new();
            }
            _ => {}
        }
    }
    raw.to_string()
}

fn row_settings(r: &sqlx::sqlite::SqliteRow) -> CenterSettings {
    CenterSettings {
        id: r.get("id"),
        name: s(r, "name"),
        trade_name: s(r, "trade_name"),
        logo_ref: s(r, "logo_ref"),
        address: s(r, "address"),
        phones: s(r, "phones"),
        email: s(r, "email"),
        website: s(r, "website"),
        license_no: s(r, "license_no"),
        tax_no: s(r, "tax_no"),
        currency_code: s(r, "currency_code"),
        currency_decimals: r.get("currency_decimals"),
        timezone: s(r, "timezone"),
        academic_year: s(r, "academic_year"),
        header_text: json_to_text(&s(r, "header_json")),
        footer_text: json_to_text(&s(r, "footer_json")),
    }
}

pub async fn get(pool: &SqlitePool) -> AppResult<CenterSettings> {
    let r = sqlx::query("SELECT * FROM center_settings WHERE id = 1")
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("إعدادات المركز غير موجودة"))?;
    Ok(row_settings(&r))
}

pub async fn update(
    pool: &SqlitePool,
    actor: &Actor,
    patch: &CenterSettingsPatch,
) -> AppResult<CenterSettings> {
    actor.require("settings.manage")?;
    if let Some(cc) = &patch.currency_code {
        if cc.len() != 3 {
            return Err(AppError::validation("رمز العملة يجب أن يكون 3 أحرف"));
        }
    }
    if let Some(d) = patch.currency_decimals {
        if !(0..=8).contains(&d) {
            return Err(AppError::validation("عدد الخانات العشرية غير صالح"));
        }
    }
    // Fetch current values, then apply the patch — simple, avoids dynamic SQL.
    let cur = get(pool).await?;
    let name = patch.name.clone().unwrap_or(cur.name);
    if name.trim().is_empty() {
        return Err(AppError::validation("اسم المركز مطلوب"));
    }
    let v = |new: &Option<String>, old: String| new.clone().unwrap_or(old);
    sqlx::query(
        "UPDATE center_settings SET
           name=?, trade_name=?, logo_ref=?, address=?, phones=?, email=?, website=?,
           license_no=?, tax_no=?, currency_code=?, currency_decimals=?, timezone=?,
           academic_year=?, header_json=?, footer_json=?,
           updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id=1",
    )
    .bind(name)
    .bind(v(&patch.trade_name, cur.trade_name))
    .bind(v(&patch.logo_ref, cur.logo_ref))
    .bind(v(&patch.address, cur.address))
    .bind(v(&patch.phones, cur.phones))
    .bind(v(&patch.email, cur.email))
    .bind(v(&patch.website, cur.website))
    .bind(v(&patch.license_no, cur.license_no))
    .bind(v(&patch.tax_no, cur.tax_no))
    .bind(patch.currency_code.clone().unwrap_or(cur.currency_code))
    .bind(patch.currency_decimals.unwrap_or(cur.currency_decimals))
    .bind(patch.timezone.clone().unwrap_or(cur.timezone))
    .bind(v(&patch.academic_year, cur.academic_year))
    .bind(serde_json::json!({ "text": v(&patch.header_text, cur.header_text) }).to_string())
    .bind(serde_json::json!({ "text": v(&patch.footer_text, cur.footer_text) }).to_string())
    .execute(pool)
    .await?;
    get(pool).await
}
