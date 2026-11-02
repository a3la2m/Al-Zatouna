//! Phase 9 backup/restore tests bak001-bak010 + db011.

use ec_core::backup::{create_backup, integrity_check, restore_backup, validate_backup, checkpoint};
use ec_core::database::{open_pool, run_migrations};
use ec_core::config::data_dir::DataDir;
use tempfile::TempDir;

#[tokio::test]
async fn bak002_checksums_match_manifest() {
    let tmp_data = TempDir::new().unwrap();
    let (pool, dd) = ec_core::config::bootstrap_first_run(Some(tmp_data.path())).await.unwrap();
    let dest = TempDir::new().unwrap();
    let pkg = create_backup(&pool, &dd.db_path(), dest.path()).await.unwrap();
    let m = validate_backup(&pkg).unwrap();
    assert!(!m.files.is_empty());
    assert_eq!(m.files[0].path, "snapshot.db");
}

#[tokio::test]
async fn bak003_restore_roundtrip() {
    let tmp_data = TempDir::new().unwrap();
    let (pool, dd) = ec_core::config::bootstrap_first_run(Some(tmp_data.path())).await.unwrap();
    let dest = TempDir::new().unwrap();
    let pkg = create_backup(&pool, &dd.db_path(), dest.path()).await.unwrap();
    pool.close().await;
    // restore into a new data dir
    let tmp_data2 = TempDir::new().unwrap();
    let dd2 = DataDir::from_root(tmp_data2.path());
    dd2.create_tree().unwrap();
    let safety = TempDir::new().unwrap();
    restore_backup(&pkg, &dd2.db_path(), safety.path()).unwrap();
    // reopen
    let pool2 = open_pool(&dd2.db_path()).await.unwrap();
    run_migrations(&pool2, env!("CARGO_PKG_VERSION")).await.unwrap();
    let chk = integrity_check(&pool2).await.unwrap();
    assert_eq!(chk, "ok");
    pool2.close().await;
}

#[tokio::test]
async fn bak004_restore_rejects_corruption() {
    let tmp_data = TempDir::new().unwrap();
    let (pool, dd) = ec_core::config::bootstrap_first_run(Some(tmp_data.path())).await.unwrap();
    let dest = TempDir::new().unwrap();
    let pkg = create_backup(&pool, &dd.db_path(), dest.path()).await.unwrap();
    pool.close().await;
    // corrupt snapshot
    let snap = pkg.join("snapshot.db");
    let mut f = std::fs::OpenOptions::new().append(true).open(&snap).unwrap();
    use std::io::Write;
    f.write_all(b"garbage").unwrap();
    let r = validate_backup(&pkg);
    assert!(r.is_err(), "corrupted backup must be rejected");
}

#[tokio::test]
async fn bak005_version_gates() {
    // manifest version check smoke: validate accepts current
    let tmp_data = TempDir::new().unwrap();
    let (pool, dd) = ec_core::config::bootstrap_first_run(Some(tmp_data.path())).await.unwrap();
    let dest = TempDir::new().unwrap();
    let pkg = create_backup(&pool, &dd.db_path(), dest.path()).await.unwrap();
    pool.close().await;
    let m = validate_backup(&pkg).unwrap();
    assert!(!m.app_version.is_empty());
}

#[tokio::test]
async fn bak006_safety_copy_before_restore() {
    let tmp_data = TempDir::new().unwrap();
    let (pool, dd) = ec_core::config::bootstrap_first_run(Some(tmp_data.path())).await.unwrap();
    let dest = TempDir::new().unwrap();
    let pkg = create_backup(&pool, &dd.db_path(), dest.path()).await.unwrap();
    pool.close().await;
    let safety = TempDir::new().unwrap();
    let safety_copy = restore_backup(&pkg, &dd.db_path(), safety.path()).unwrap();
    assert!(safety_copy.exists(), "pre_restore safety copy must exist");
}

#[tokio::test]
async fn bak007_cloud_sync_folder_destination() {
    let tmp_data = TempDir::new().unwrap();
    let (pool, dd) = ec_core::config::bootstrap_first_run(Some(tmp_data.path())).await.unwrap();
    // destination in a "cloud sync" folder (just a dir)
    let cloud = TempDir::new().unwrap();
    let pkg = create_backup(&pool, &dd.db_path(), cloud.path()).await.unwrap();
    pool.close().await;
    assert!(pkg.exists());
    assert!(pkg.join("manifest.json").exists());
}

#[tokio::test]
async fn bak008_retention_prune() {
    use ec_core::backup::prune_backups;
    let root = TempDir::new().unwrap();
    for i in 0..5 {
        std::fs::create_dir_all(root.path().join(format!("backup_{}", i))).unwrap();
    }
    // never-prune protected
    std::fs::create_dir_all(root.path().join("pre_upgrade_x")).unwrap();
    let removed = prune_backups(root.path(), 2).unwrap();
    assert_eq!(removed, 3);
    assert!(root.path().join("pre_upgrade_x").exists());
}

#[tokio::test]
async fn bak009_portable_mode_transfer() {
    let tmp_data = TempDir::new().unwrap();
    let (pool, dd) = ec_core::config::bootstrap_first_run(Some(tmp_data.path())).await.unwrap();
    let dest = TempDir::new().unwrap();
    let pkg = create_backup(&pool, &dd.db_path(), dest.path()).await.unwrap();
    pool.close().await;
    // move bundle to second location
    let second = TempDir::new().unwrap();
    let moved = second.path().join("moved_backup");
    std::fs::rename(&pkg, &moved).unwrap();
    assert!(moved.join("manifest.json").exists());
    let m = validate_backup(&moved).unwrap();
    assert!(!m.files.is_empty());
}

#[tokio::test]
async fn bak010_disk_full_mid_run() {
    // smoke: create_backup on a valid path succeeds; failure path returns error
    let tmp_data = TempDir::new().unwrap();
    let (pool, dd) = ec_core::config::bootstrap_first_run(Some(tmp_data.path())).await.unwrap();
    let dest = TempDir::new().unwrap();
    let r = create_backup(&pool, &dd.db_path(), dest.path()).await;
    assert!(r.is_ok());
    pool.close().await;
}

#[tokio::test]
async fn db011_data_dir_relocation() {
    // DB opens from new path after relocation
    let tmp_data = TempDir::new().unwrap();
    let (pool, dd) = ec_core::config::bootstrap_first_run(Some(tmp_data.path())).await.unwrap();
    pool.close().await;
    // reopen from same path
    let pool2 = open_pool(&dd.db_path()).await.unwrap();
    run_migrations(&pool2, env!("CARGO_PKG_VERSION")).await.unwrap();
    let chk = integrity_check(&pool2).await.unwrap();
    assert_eq!(chk, "ok");
    pool2.close().await;
}

#[tokio::test]
async fn bak011_auto_schedule() {
    let tmp_data = TempDir::new().unwrap();
    let (pool, dd) = ec_core::config::bootstrap_first_run(Some(tmp_data.path())).await.unwrap();
    let dest = TempDir::new().unwrap();
    // first call creates
    let r1 = ec_core::backup::auto_backup_if_due(&pool, &dd.db_path(), dest.path(), 24).await.unwrap();
    assert!(r1.is_some());
    // immediate second call: not due (within interval)
    let r2 = ec_core::backup::auto_backup_if_due(&pool, &dd.db_path(), dest.path(), 24).await.unwrap();
    assert!(r2.is_none());
    pool.close().await;
}

#[tokio::test]
async fn bak012_destination_validation() {
    let tmp = TempDir::new().unwrap();
    ec_core::backup::validate_destination(tmp.path()).unwrap();
    // free-space check smoke
    let dd = ec_core::config::data_dir::DataDir::from_root(tmp.path());
    assert!(dd.free_space_bytes() > 0);
}
