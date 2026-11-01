//! SQLite database layer: pool pragmas, migrations, integrity checks.

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

use crate::error::{AppError, Result};

/// Every connection opens with the project pragmas (docs/DATABASE.md §1).
fn connect_options(db_path: &Path) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(5))
}

/// Open (or create) a database with the standard pragmas. One-connection
/// pool by design — SQLite desktop app with a single writer (ADR-004);
/// background long jobs acquire from the same pool.
pub async fn open_pool(db_path: &Path) -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_options(db_path))
        .await
        .map_err(|e| AppError::storage(format!("open db: {e}")))?;
    Ok(pool)
}

/// Apply all embedded migrations (numbered SQL files) and stamp `db_meta`.
/// sqlx tracks applied versions in `_sqlx_migrations` with checksums — the
/// single source of truth (docs/DATABASE.md §3).
pub async fn run_migrations(pool: &SqlitePool, app_version: &str) -> Result<()> {
    static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
    MIGRATOR
        .run(pool)
        .await
        .map_err(|e| AppError::storage(format!("migrations: {e}")))?;
    stamp_db_meta(pool, app_version).await?;
    Ok(())
}

async fn stamp_db_meta(pool: &SqlitePool, app_version: &str) -> Result<()> {
    let row = sqlx::query(
        "SELECT COALESCE(MAX(version), 0) AS v FROM _sqlx_migrations",
    )
    .fetch_one(pool)
    .await?;
    let version: i64 = row.get(0);
    sqlx::query(
        r#"
        INSERT INTO db_meta (id, schema_version, applied_at, app_version_at_apply)
        VALUES (1, ?, strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?)
        ON CONFLICT (id) DO UPDATE
          SET schema_version = excluded.schema_version,
              applied_at     = excluded.applied_at,
              app_version_at_apply = excluded.app_version_at_apply
        "#,
    )
    .bind(version)
    .bind(app_version)
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    Ok(())
}

/// `PRAGMA integrity_check` — used after backups/restore (docs/DATABASE.md).
pub async fn integrity_check(pool: &SqlitePool) -> Result<String> {
    let row = sqlx::query("PRAGMA integrity_check").fetch_one(pool).await?;
    let s: String = row.get(0);
    Ok(s)
}

/// `PRAGMA quick_check` — fast boot check.
pub async fn quick_check(pool: &SqlitePool) -> Result<String> {
    let row = sqlx::query("PRAGMA quick_check").fetch_one(pool).await?;
    let s: String = row.get(0);
    Ok(s)
}

/// WAL checkpoint (used before backups and on graceful close).
pub async fn wal_checkpoint(pool: &SqlitePool) -> Result<()> {
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    Ok(())
}

/// db009 helper: verify that no money-bearing column uses float storage.
/// Returns offending (table, column) pairs; an empty vec means schema is
/// integer-clean for amounts.
pub async fn scan_money_column_storage(pool: &SqlitePool) -> Result<Vec<(String, String)>> {
    let tables = sqlx::query(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE '_sqlx%' AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_all(pool)
    .await?;
    let mut bad = Vec::new();
    for t in tables {
        let tname: String = t.get(0);
        let cols = sqlx::query(&format!(
            "SELECT name, type FROM pragma_table_info('{tname}')"
        ))
        .fetch_all(pool)
        .await?;
        for c in cols {
            let cname: String = c.get::<String, _>(0);
            let ctype: String = c.get::<String, _>(1).to_uppercase();
            let is_money_named = cname.ends_with("_minor")
                || cname.ends_with("_fee_minor")
                || cname.contains("amount");
            if is_money_named && (ctype.contains("REAL") || ctype.contains("FLOAT") || ctype.contains("DOUBLE")) {
                bad.push((tname.clone(), cname));
            }
        }
    }
    Ok(bad)
}

/// Test-time helpers shared by integration tests (docs/TEST_PLAN.md §1).
/// Gated behind `test-utils` so production builds never expose them;
/// the crate self-enables the feature in dev-dependencies.
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils {
    use std::path::PathBuf;
    use tempfile::TempDir;
    use sqlx::{Row, Sqlite, SqlitePool, Transaction};

    use crate::common::{allocate_next_no, seed_sequence, SequenceKind};
    use crate::database::{open_pool, run_migrations};

    /// A fresh, migrated, seeded sqlite in a temporary directory.
    /// Returns (pool, tempdir guard, main branch id).
    pub async fn fresh_seeded_pool() -> (SqlitePool, TempDir, i64) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path: PathBuf = tmp.path().join("education_center.db");
        let pool = open_pool(&path).await.expect("open pool");
        run_migrations(&pool, "test").await.expect("migrations");
        let row = sqlx::query("SELECT id FROM branches WHERE code='MAIN'")
            .fetch_one(&pool)
            .await
            .expect("main branch");
        let branch_id: i64 = row.get(0);
        (pool, tmp, branch_id)
    }

    /// Seed the year-scoped voucher sequences used by tests.
    pub async fn seed_branch_sequences(
        conn: &mut sqlx::SqliteConnection,
        branch_id: i64,
    ) {
        let year = chrono::Utc::now().format("%Y").to_string().parse().unwrap_or(0);
        for k in [SequenceKind::ReceiptVoucher, SequenceKind::PaymentVoucher, SequenceKind::JournalEntry] {
            seed_sequence(&mut *conn, branch_id, k, year).await.expect("seed sequence");
        }
    }

    /// Convenience: current fiscal year as i64 for tests.
    pub fn test_year() -> i64 {
        chrono::Utc::now().format("%Y").to_string().parse().unwrap_or(0)
    }

    /// Run a closure inside a transaction; Err => rollback (drop), Ok => commit.
    pub async fn in_tx<F, Fut, T, E>(
        pool: &SqlitePool,
        f: F,
    ) -> std::result::Result<T, E>
    where
        F: FnOnce(Transaction<'static, Sqlite>) -> Fut,
        Fut: std::future::Future<Output = std::result::Result<T, E>>,
    {
        let tx = pool.begin().await.expect("begin tx");
        f(tx).await
    }

    /// Allocate + commit helper for tests.
    pub async fn alloc_receipt_no(
        pool: &SqlitePool,
        branch_id: i64,
    ) -> i64 {
        let mut conn = pool.acquire().await.unwrap();
        seed_branch_sequences(&mut conn, branch_id).await;
        let y = test_year();
        allocate_next_no(&mut conn, branch_id, SequenceKind::ReceiptVoucher, y)
            .await
            .unwrap()
    }
}
