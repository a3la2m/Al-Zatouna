//! Shareable data bundles (Phase 24).
//!
//! A bundle is a plain `.zip` containing:
//! - `database/education_center.db` — a consistent `VACUUM INTO` snapshot
//! - `settings.json` — the app settings file (data dir is replaced on import)
//! - `assets/*` — centre logo and any other printable assets
//! - `manifest.json` — version + creation time + checksums
//!
//! Import is defensive: entries are only extracted below the destination
//! folder (no `..`, no absolute paths) and the presence of a database file is
//! required before the directory can be adopted.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::AppError;
use crate::{backup, AppResult};

fn sha256_file(path: &Path) -> AppResult<String> {
    use std::io::Read;
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn zip_err(e: zip::result::ZipError) -> AppError {
    AppError::storage(format!("archive: {e}"))
}

fn add_path(zip: &mut ZipWriter<File>, base: &Path, path: &Path) -> AppResult<()> {
    let rel = path
        .strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let meta = fs::metadata(path)?;
    let opts = FileOptions::default().compression_method(CompressionMethod::Deflated);
    if meta.is_dir() {
        zip.add_directory(format!("{rel}/"), opts).map_err(zip_err)?;
        for entry in fs::read_dir(path)? {
            add_path(zip, base, &entry?.path())?;
        }
    } else if meta.is_file() {
        zip.start_file(rel, opts).map_err(zip_err)?;
        let mut f = File::open(path)?;
        io::copy(&mut f, zip)?;
    }
    Ok(())
}

/// Export a compressed bundle to `dest` (a `.zip` path). The database snapshot
/// is taken with `VACUUM INTO`, so the app may keep running while copying.
pub async fn export_bundle(
    pool: &SqlitePool,
    data_dir: &Path,
    dest: &Path,
) -> AppResult<PathBuf> {
    backup::checkpoint(pool).await?;
    let check = backup::integrity_check(pool).await?;
    if check != "ok" {
        return Err(AppError::storage(format!("integrity check failed: {check}")));
    }

    let dest = if dest.extension().and_then(|e| e.to_str()) == Some("zip") {
        dest.to_path_buf()
    } else {
        dest.with_extension("zip")
    };
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp_dir = data_dir.join("tmp");
    fs::create_dir_all(&tmp_dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let snapshot = tmp_dir.join(format!("bundle-{stamp}.db"));
    backup::create_snapshot(pool, &snapshot).await?;

    let tmp_zip = dest.with_extension("zip.part");
    let result = (|| -> AppResult<()> {
        let file = File::create(&tmp_zip)?;
        let mut zip = ZipWriter::new(file);
        let opts = FileOptions::default().compression_method(CompressionMethod::Deflated);

        zip.start_file("database/education_center.db", opts)
            .map_err(zip_err)?;
        let mut db = File::open(&snapshot)?;
        io::copy(&mut db, &mut zip)?;

        if let Some(sp) = crate::config::settings_path() {
            if sp.is_file() {
                zip.start_file("settings.json", opts).map_err(zip_err)?;
                let mut f = File::open(&sp)?;
                io::copy(&mut f, &mut zip)?;
            }
        }

        let assets = data_dir.join("assets");
        if assets.is_dir() {
            add_path(&mut zip, data_dir, &assets)?;
        }

        let manifest = serde_json::json!({
            "app": crate::APP_NAME,
            "app_version": env!("CARGO_PKG_VERSION"),
            "created_at": chrono::Utc::now().to_rfc3339(),
            "database_sha256": sha256_file(&snapshot)?,
            "database_bytes": fs::metadata(&snapshot)?.len(),
        });
        zip.start_file("manifest.json", opts).map_err(zip_err)?;
        use std::io::Write;
        zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;

        zip.finish().map_err(zip_err)?;
        Ok(())
    })();
    let _ = fs::remove_file(&snapshot);
    result?;
    fs::rename(&tmp_zip, &dest)?;
    Ok(dest)
}

/// Extract a `.zip` bundle (or copy a raw `.db`) into `dest_dir` and verify a
/// database is present. Returns the path of the extracted database.
pub fn import_bundle(src: &Path, dest_dir: &Path) -> AppResult<PathBuf> {
    if !src.exists() {
        return Err(AppError::validation("الملف المحدد غير موجود"));
    }
    fs::create_dir_all(dest_dir)?;
    let db_out = dest_dir.join("database").join("education_center.db");
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "db" || ext == "sqlite" || ext == "sqlite3" {
        fs::create_dir_all(db_out.parent().unwrap())?;
        fs::copy(src, &db_out)?;
    } else {
        let file = File::open(src)?;
        let mut zip = ZipArchive::new(file)
            .map_err(|e| AppError::validation(format!("أرشيف غير صالح: {e}")))?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(zip_err)?;
            let name = entry.name().replace('\\', "/");
            if name.starts_with('/') || name.split('/').any(|c| c == "..") {
                return Err(AppError::validation("أرشيف يحتوي على مسار غير آمن"));
            }
            let out = dest_dir.join(&name);
            if entry.is_dir() {
                fs::create_dir_all(&out)?;
                continue;
            }
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut f = File::create(&out)?;
            io::copy(&mut entry, &mut f)?;
        }
    }

    if !db_out.exists() {
        return Err(AppError::validation(
            "الأرشيف لا يحتوي على قاعدة بيانات (database/education_center.db)",
        ));
    }
    // keep a copy of the manifest for auditing, when present
    let manifest = dest_dir.join("manifest.json");
    if manifest.is_file() {
        if let Ok(raw) = fs::read_to_string(&manifest) {
            let _: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
        }
    }
    Ok(db_out)
}

/// A human friendly suggested file name for the share dialog.
pub fn suggested_name() -> String {
    format!("الزيتونة-{}.zip", chrono::Local::now().format("%Y%m%d-%H%M"))
}

/// Last-modified helper used by the UI listing (best effort).
pub fn modified_label(path: &Path) -> String {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| {
            chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                .map(|dt| dt.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default()
}
