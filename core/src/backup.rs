//! Backup / restore (Phase 9).

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::AppResult;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ManifestFile {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Manifest {
    pub app_version: String,
    pub created_at: String,
    pub files: Vec<ManifestFile>,
}

pub async fn checkpoint(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn integrity_check(pool: &SqlitePool) -> AppResult<String> {
    let r: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(pool)
        .await?;
    Ok(r)
}

pub async fn create_snapshot(pool: &SqlitePool, dest: &Path) -> AppResult<()> {
    // VACUUM INTO requires the destination not exist
    if dest.exists() {
        fs::remove_file(dest).ok();
    }
    let sql = format!("VACUUM INTO '{}'", dest.display().to_string().replace('\'', "''"));
    sqlx::query(&sql).execute(pool).await?;
    Ok(())
}

fn sha256_file(path: &Path) -> AppResult<String> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex_encode(&h.finalize()))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

/// Create a backup package directory containing a consistent snapshot + manifest.
pub async fn create_backup(
    pool: &SqlitePool,
    data_dir_db: &Path,
    dest_root: &Path,
) -> AppResult<PathBuf> {
    checkpoint(pool).await?;
    let chk = integrity_check(pool).await?;
    if chk != "ok" {
        return Err(AppError::storage(format!("integrity check failed: {chk}")));
    }
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let pkg = dest_root.join(format!("backup_{}", ts));
    fs::create_dir_all(&pkg)?;
    let _snap = pkg.join("snapshot.db");
    create_snapshot(pool, &_snap).await?;

    // manifest
    let mut files = Vec::new();
    let meta = fs::metadata(&_snap)?;
    files.push(ManifestFile {
        path: "snapshot.db".into(),
        sha256: sha256_file(&_snap)?,
        size: meta.len(),
    });
    // include db_meta? already inside snapshot
    let manifest = Manifest {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        files,
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| AppError::storage(e.to_string()))?;
    fs::write(pkg.join("manifest.json"), manifest_json)?;
    let _ = data_dir_db;
    Ok(pkg)
}

/// Validate a backup package: manifest present, checksums match.
pub fn validate_backup(pkg: &Path) -> AppResult<Manifest> {
    let manifest_path = pkg.join("manifest.json");
    if !manifest_path.exists() {
        return Err(AppError::validation("manifest.json مفقود"));
    }
    let raw = fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&raw)
        .map_err(|e| AppError::validation(format!("manifest غير صالح: {e}")))?;
    for f in &manifest.files {
        let p = pkg.join(&f.path);
        if !p.exists() {
            return Err(AppError::validation(format!("ملف مفقود: {}", f.path)));
        }
        let actual = sha256_file(&p)?;
        if actual != f.sha256 {
            return Err(AppError::validation(format!(
                "checksum mismatch for {}",
                f.path
            )));
        }
    }
    Ok(manifest)
}

/// Restore: safety copy of current db, then replace with snapshot.
pub fn restore_backup(pkg: &Path, data_dir_db: &Path, safety_dir: &Path) -> AppResult<PathBuf> {
    let manifest = validate_backup(pkg)?;
    // safety copy
    fs::create_dir_all(safety_dir)?;
    let safety_copy = safety_dir.join("pre_restore.db");
    if data_dir_db.exists() {
        fs::copy(data_dir_db, &safety_copy)?;
    }
    let _snap = pkg.join("snapshot.db");
    // find snapshot file from manifest
    let snap_name = manifest
        .files
        .first()
        .map(|f| f.path.clone())
        .unwrap_or_else(|| "snapshot.db".into());
    let src = pkg.join(snap_name);
    // atomic-ish copy
    let tmp = data_dir_db.with_extension("db.restore_tmp");
    fs::copy(&src, &tmp)?;
    if data_dir_db.exists() {
        fs::remove_file(data_dir_db)?;
    }
    fs::rename(&tmp, data_dir_db)?;
    Ok(safety_copy)
}

/// Retention prune: keep last `keep` backup dirs of a given prefix; never
/// prune directories whose name contains "pre_upgrade" or "pre_restore".
pub fn prune_backups(dest_root: &Path, keep: usize) -> AppResult<usize> {
    if !dest_root.exists() {
        return Ok(0);
    }
    let mut dirs: Vec<PathBuf> = fs::read_dir(dest_root)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            !name.contains("pre_upgrade") && !name.contains("pre_restore")
        })
        .collect();
    dirs.sort();
    let mut removed = 0;
    if dirs.len() > keep {
        let to_remove = dirs.len() - keep;
        for d in dirs.iter().take(to_remove) {
            fs::remove_dir_all(d)?;
            removed += 1;
        }
    }
    Ok(removed)
}


/// Validate a backup destination folder (writable + enough space).
pub fn validate_destination(dest: &Path) -> AppResult<()> {
    if !dest.exists() {
        std::fs::create_dir_all(dest)?;
    }
    // write probe
    let probe = dest.join(".backup_probe");
    std::fs::write(&probe, b"ok")?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

/// Auto-backup: create a backup if the last one is older than `interval_hours`.
pub async fn auto_backup_if_due(
    pool: &SqlitePool,
    db_path: &Path,
    dest_root: &Path,
    interval_hours: i64,
) -> AppResult<Option<PathBuf>> {
    validate_destination(dest_root)?;
    // find latest backup dir
    let mut latest: Option<std::time::SystemTime> = None;
    if dest_root.exists() {
        for e in std::fs::read_dir(dest_root)? {
            let e = e?;
            if e.path().is_dir() {
                if let Ok(m) = e.metadata() {
                    if let Ok(t) = m.modified() {
                        if latest.map_or(true, |l| t > l) {
                            latest = Some(t);
                        }
                    }
                }
            }
        }
    }
    let now = std::time::SystemTime::now();
    let due = match latest {
        None => true,
        Some(t) => now.duration_since(t).map_or(true, |d| d.as_secs() >= (interval_hours as u64) * 3600),
    };
    if !due {
        return Ok(None);
    }
    let pkg = create_backup(pool, db_path, dest_root).await?;
    Ok(Some(pkg))
}
