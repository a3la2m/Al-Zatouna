//! Phase 24: shareable data bundles (zip export/import round-trip).

use ec_core::database::test_utils::fresh_seeded_pool;

#[tokio::test]
async fn arch001_export_bundle_is_importable() {
    let (pool, t, _b) = fresh_seeded_pool().await;
    let data_dir = t.path().to_path_buf();
    let zip_path = data_dir.join("share.zip");

    let out = ec_core::archive::export_bundle(&pool, &data_dir, &zip_path)
        .await
        .unwrap();
    assert!(out.exists(), "archive created");
    assert_eq!(out.extension().and_then(|e| e.to_str()), Some("zip"));
    let size = std::fs::metadata(&out).unwrap().len();
    assert!(size > 1000, "archive has content ({size} bytes)");

    let dest = data_dir.join("imported");
    let db = ec_core::archive::import_bundle(&out, &dest).unwrap();
    assert!(db.exists());

    let pool2 = ec_core::database::open_pool(&db).await.unwrap();
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='students'",
    )
    .fetch_one(&pool2)
    .await
    .unwrap();
    assert_eq!(n, 1, "imported database keeps its schema");
    let accounts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(&pool2)
        .await
        .unwrap();
    assert!(accounts > 0, "seeded rows survived the round-trip");
}

#[tokio::test]
async fn arch002_imports_a_raw_database_file() {
    let (pool, t, _b) = fresh_seeded_pool().await;
    let data_dir = t.path().to_path_buf();
    // make a consistent copy of the DB
    let raw = data_dir.join("copy.db");
    ec_core::backup::create_snapshot(&pool, &raw).await.unwrap();
    assert!(raw.exists());

    let dest = data_dir.join("raw-import");
    let db = ec_core::archive::import_bundle(&raw, &dest).unwrap();
    assert_eq!(db, dest.join("database").join("education_center.db"));
    assert!(db.exists());
}

#[test]
fn arch003_rejects_unknown_file() {
    let tmp = tempfile::tempdir().unwrap();
    let bogus = tmp.path().join("not-a-db.txt");
    std::fs::write(&bogus, b"hello").unwrap();
    let dest = tmp.path().join("out");
    let err = ec_core::archive::import_bundle(&bogus, &dest).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("أرشيف") || msg.contains("قاعدة"), "unexpected: {msg}");
}

#[test]
fn arch004_suggested_name_has_zip_extension() {
    let name = ec_core::archive::suggested_name();
    assert!(name.starts_with("الزيتونة-"));
    assert!(name.ends_with(".zip"));
}
