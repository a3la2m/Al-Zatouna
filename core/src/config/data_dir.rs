//! Data directory management (Phase 2): resolution, tree creation,
//! manifest, permission testing. Portable mode + relocation arrive with
//! the wizard UI in Phase 3 but the primitives live here.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{AppError, Result};

/// Concrete sub-tree of the data directory (docs/BACKUP_RESTORE.md §1).
const TREE: &[&str] = &[
    "database",
    "config/branding",
    "attachments/students",
    "attachments/teachers",
    "attachments/receipts",
    "attachments/documents",
    "templates/receipts",
    "templates/reports",
    "templates/certificates",
    "backups/automatic",
    "backups/manual",
    "exports/pdf",
    "exports/excel",
    "logs",
];

pub const PORTABLE_MARKER: &str = "portable.marker";

/// Default OS path per platform (FR-1.1). Windows: %APPDATA%/EducationCenter,
/// Linux: ~/.local/share/education-center, macOS: .../EducationCenter.
pub fn default_os_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        dirs::data_dir().map(|d| d.join("EducationCenter"))
    }
    #[cfg(target_os = "macos")]
    {
        dirs::data_dir().map(|d| d.join("EducationCenter"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        dirs::data_dir().map(|d| d.join("education-center"))
    }
}

/// Directory of the running executable (portable mode anchor).
pub fn executable_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.to_path_buf())
}

fn portable_marker_path() -> Option<PathBuf> {
    executable_dir().map(|dir| dir.join(PORTABLE_MARKER))
}

pub fn is_portable_mode() -> bool {
    portable_marker_path().map(|p| p.exists()).unwrap_or(false)
}

/// All module code receives paths via this handle; modules must never build
/// absolute paths by guesswork (path traversal rule, docs/SECURITY.md §4).
#[derive(Debug, Clone)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    pub fn from_root(root: impl Into<PathBuf>) -> Self {
        let root: PathBuf = root.into();
        let canonical = root.canonicalize().unwrap_or(root);
        Self { root: canonical }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// A path inside the data directory; rejects traversal attempts.
    /// Lexical validation of the RELATIVE component: no absolute prefixes,
    /// no escaping above the data root (`..` beyond depth 0 is rejected).
    pub fn join_checked(&self, rel: &str) -> Result<PathBuf> {
        let rel_path = std::path::Path::new(rel);
        if rel.is_empty() {
            return Ok(self.root.clone());
        }
        // absolute path or Windows drive prefix is never acceptable
        if rel_path.is_absolute()
            || (rel.len() >= 2 && rel.as_bytes()[1] == b':')
            || rel.starts_with('/')
            || rel.starts_with('\\')
        {
            return Err(AppError::validation(format!(
                "absolute paths are not allowed: {rel}"
            )));
        }
        let mut depth: i64 = 0;
        for comp in rel_path.components() {
            match comp {
                std::path::Component::Normal(_) => depth += 1,
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    depth -= 1;
                    if depth < 0 {
                        return Err(AppError::validation(format!(
                            "path escapes data directory: {rel}"
                        )));
                    }
                }
                _ => return Err(AppError::validation(format!(
                    "unsupported path component in: {rel}"
                ))),
            }
        }
        Ok(self.root.join(rel_path))
    }

    /// Create the full folder skeleton + manifest (idempotent).
    pub fn create_tree(&self) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        for rel in TREE {
            fs::create_dir_all(self.root.join(rel))?;
        }
        self.ensure_manifest()?;
        Ok(())
    }

    pub fn db_path(&self) -> PathBuf {
        self.root.join("database").join("education_center.db")
    }

    fn manifest_path(&self) -> PathBuf {
        self.root.join("manifest.json")
    }

    fn ensure_manifest(&self) -> Result<()> {
        let path = self.manifest_path();
        if path.exists() {
            return Ok(());
        }
        let manifest = DataManifest {
            manifest_version: 1,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            database_schema_version: None,
        };
        let json = serde_json::to_string_pretty(&manifest)?;
        fs::write(&path, json)?;
        Ok(())
    }

    pub fn read_manifest(&self) -> Result<Option<DataManifest>> {
        let path = self.manifest_path();
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_str(&fs::read_to_string(&path)?)?))
    }

    /// Write/read/delete a temp file to prove write permissions (FR-1.1).
    pub fn test_write_permissions(&self) -> Result<()> {
        let probe = self.root.join(".permission_probe");
        fs::write(&probe, b"ok")?;
        let content = fs::read(&probe)?;
        if content.as_slice() != b"ok" {
            return Err(AppError::storage("permission probe mismatch"));
        }
        fs::remove_file(&probe)?;
        Ok(())
    }

    /// SHA-256 of any file within this data dir (used by backup, phase 9).
    pub fn sha256_file(&self, rel: &str) -> Result<String> {
        let path = self.join_checked(rel)?;
        let bytes = fs::read(path)?;
        let h = Sha256::digest(&bytes);
        Ok(format!("{:x}", h))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataManifest {
    pub manifest_version: u64,
    pub app_version: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_schema_version: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_tree_and_probes_permissions() {
        let tmp = tempfile::tempdir().unwrap();
        let dd = DataDir::from_root(tmp.path());
        dd.create_tree().unwrap();
        for rel in TREE {
            assert!(tmp.path().join(rel).exists(), "missing {rel}");
        }
        dd.test_write_permissions().unwrap();
        let m = dd.read_manifest().unwrap().expect("manifest");
        assert_eq!(m.manifest_version, 1);
    }

    #[test]
    fn idempotent_tree_creation() {
        let tmp = tempfile::tempdir().unwrap();
        let dd = DataDir::from_root(tmp.path());
        dd.create_tree().unwrap();
        dd.create_tree().unwrap(); // second run succeeds
    }

    #[test]
    fn rejects_path_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        let dd = DataDir::from_root(tmp.path());
        assert!(dd.join_checked("../../etc/passwd").is_err());
        assert!(dd.join_checked("database/education_center.db").is_ok());
    }

    #[test]
    fn portable_marker_detection() {
        // no marker next to this test binary → not portable
        assert!(!is_portable_mode());
    }
}
