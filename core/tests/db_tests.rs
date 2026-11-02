//! Database integration tests (docs/TEST_PLAN.md §2) — Phase 2 gate
//! db001..db010. Migrations + seeds are provided by ec-core.

use std::path::PathBuf;

use ec_core::common::{allocate_next_no, seed_sequence, SequenceKind};
use ec_core::database::test_utils::fresh_seeded_pool;
use ec_core::database::{
    integrity_check, open_pool, quick_check, run_migrations, scan_money_column_storage,
    test_utils, wal_checkpoint,
};
use sqlx::Row;

// db001 — fresh DB from empty file: migrations idempotent, seeds present.
#[tokio::test]
async fn db001_create_fresh_db() {
    let (pool, _t, branch_id) = fresh_seeded_pool().await;
    // seeds present
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(n >= 20, "chart of accounts seeded, got {}", n);

    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM permissions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(n >= 40, "permissions seeded, got {}", n);

    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM roles")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 7, "7 system roles expected");

    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM print_profiles")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(n >= 10);

    // main branch + central register + money sequences
    let _: i64 = sqlx::query_scalar("SELECT id FROM branches WHERE code='MAIN' ")
        .fetch_one(&pool)
        .await
        .unwrap();
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM voucher_sequences WHERE branch_id = ? AND kind='student_no'",
    )
    .bind(branch_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 1);
}

// db002 — migrations chain applies in order; a second run is a no-op;
// checksums are the tracked truth (sqlx validates them).
#[tokio::test]
async fn db002_migration_chain_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let path: PathBuf = tmp.path().join("m.db");
    let pool = open_pool(&path).await.unwrap();
    run_migrations(&pool, "t1").await.unwrap();
    run_migrations(&pool, "t2").await.unwrap(); // second call must not fail
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 3, "three migrations expected (init + seed + teacher_link)");
    // db_meta stamped with max version
    let v: i64 = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(v >= 2);
}

// db003 — foreign keys are enforced everywhere the schema declares them.
#[tokio::test]
async fn db003_foreign_keys_enforced() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    // FK to missing subject
    let err = sqlx::query(
        "INSERT INTO courses (branch_id, course_no, name, subject_id, teacher_id, room_id, created_by)
         VALUES (1, 1, 'X', 999, 1, 1, 1)",
    )
    .execute(&pool)
    .await;
    assert!(err.is_err(), "FK violation must be rejected: {:?}", err.map(|_| ()));
    // FK to missing teacher
    let err = sqlx::query(
        "INSERT INTO courses (branch_id, course_no, name, subject_id, teacher_id, room_id, created_by)
         VALUES (1, 1, 'X', 1, 999, 1, 1)",
    )
    .execute(&pool)
    .await;
    assert!(err.is_err());

    // on-delete RESTRICT protection: deleting a referenced branch fails
    let err = sqlx::query("DELETE FROM branches WHERE code='MAIN'")
        .execute(&pool)
        .await;
    assert!(err.is_err(), "branch referenced by center rows must not be deleted");
}

// db004 — WAL mode is on and a checkpoint does not lose rows.
#[tokio::test]
async fn db004_wal() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    let row = sqlx::query("PRAGMA journal_mode").fetch_one(&pool).await.unwrap();
    let mode: String = row.get(0);
    assert_eq!(mode.to_lowercase(), "wal");
    sqlx::query("INSERT INTO subjects (code, name) VALUES ('T1', 'Test')")
        .execute(&pool)
        .await
        .unwrap();
    wal_checkpoint(&pool).await.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM subjects WHERE code='T1'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 1, "data survived checkpoint");
}

// db005 — transaction atomicity: mid-command failure leaves nothing behind.
#[tokio::test]
async fn db005_transaction_atomicity() {
    let (pool, _t, _branch_id) = fresh_seeded_pool().await;
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM subjects")
        .fetch_one(&pool)
        .await
        .unwrap();

    // half-insert then a deliberate constraint blow-up
    let res = async {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("INSERT INTO subjects (code, name) VALUES ('ATOMIC1', 'ناجح أولاً')")
            .execute(&mut *tx)
            .await
            .unwrap();
        // duplicate code → fails → whole tx rolls back
        sqlx::query("INSERT INTO subjects (code, name) VALUES ('ATOMIC1', 'يتكرر ويفشل')")
            .execute(&mut *tx)
            .await
            .expect_err("duplicate must fail");
        tx.rollback().await.unwrap();
        Ok::<(), ec_core::AppError>(())
    };
    let _ = res.await;

    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM subjects")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, after, "no partial writes survived a failed transaction");
}

// db006 — posted journal entries are immutable (trigger backstop).
#[tokio::test]
async fn db006_posted_entry_immutable() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    let _uid = test_utils_user(&pool).await;
    let acc_cash: i64 = sqlx::query_scalar("SELECT id FROM accounts WHERE code='1000'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let acc_recv: i64 = sqlx::query_scalar("SELECT id FROM accounts WHERE code='1300'")
        .fetch_one(&pool)
        .await
        .unwrap();

    // post a balanced entry (student receivable recognized)
    let mut tx = pool.begin().await.unwrap();
    sqlx::query(
        r#"INSERT INTO journal_entries
             (entry_no, entry_date, memo, currency, total_debit_minor, total_credit_minor,
              status, source_type, source_id, posted_by, posted_at)
           VALUES (1, '2026-01-01', 'enroll', 'ILS', 10000, 10000, 'posted',
                   'enroll_recognition', 1, NULL, strftime('%Y-%m-%dT%H:%M:%fZ','now'))"#,
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO journal_lines (entry_id, account_id, debit_minor, credit_minor)
         VALUES ((SELECT id FROM journal_entries WHERE entry_no=1), ?, 10000, 0)",
    )
    .bind(acc_recv)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO journal_lines (entry_id, account_id, debit_minor, credit_minor)
         VALUES ((SELECT id FROM journal_entries WHERE entry_no=1), ?, 0, 10000)",
    )
    .bind(acc_cash)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // update of a posted entry is aborted (only posted->reversed allowed)
    let err = sqlx::query("UPDATE journal_entries SET memo='hacked' WHERE entry_no=1")
        .execute(&pool)
        .await;
    assert!(err.is_err(), "posted entry update must abort");

    // delete is aborted
    let err = sqlx::query("DELETE FROM journal_entries WHERE entry_no=1")
        .execute(&pool)
        .await;
    assert!(err.is_err());

    // line mutation is aborted
    let err = sqlx::query("UPDATE journal_lines SET debit_minor=1 WHERE debit_minor=10000")
        .execute(&pool)
        .await;
    assert!(err.is_err());

    // audit log is append-only (insert one row first so the trigger fires)
    sqlx::query(
        "INSERT INTO audit_log (action, entity_type) VALUES ('x', 'tests')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let err = sqlx::query("UPDATE audit_log SET action='x'")
        .execute(&pool)
        .await;
    assert!(err.is_err(), "audit rows must not update");
    let err = sqlx::query("DELETE FROM audit_log")
        .execute(&pool)
        .await;
    assert!(err.is_err(), "audit rows must not delete");
}

// db007 — integrity remains ok after a stress run of mixed operations.
#[tokio::test]
async fn db007_integrity_check_after_ops() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    // ~500 mixed ops
    for i in 0..100u32 {
        let code = format!("S{i}");
        sqlx::query("INSERT INTO subjects (code, name) VALUES (?, ?)")
            .bind(&code)
            .bind(format!("مادة {i}"))
            .execute(&pool)
            .await
            .unwrap();
        let id: i64 = sqlx::query_scalar("SELECT id FROM subjects WHERE code = ?")
            .bind(&code)
            .fetch_one(&pool)
            .await
            .unwrap();
        let updated = format!("مادة محدثة {i}");
        sqlx::query("UPDATE subjects SET name=? WHERE id=?")
            .bind(&updated)
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        for j in 0..5u32 {
            // mixed write/load on a busy-ish table (no financial side effects)
            sqlx::query("INSERT INTO guardians (full_name) VALUES (?)")
                .bind(format!("ولي {i}-{j}"))
                .execute(&pool)
                .await
                .unwrap();
        }
        sqlx::query("DELETE FROM guardians WHERE full_name LIKE ?")
            .bind(format!("ولي {i}-%"))
            .execute(&pool)
            .await
            .unwrap();
    }
    let ok = integrity_check(&pool).await.unwrap();
    assert_eq!(ok.to_lowercase(), "ok", "integrity must hold after stress");
}

// db008 — busy timeout + single retry path; second concurrent writer waits
// at most busy_timeout, no state corruption.
#[tokio::test]
async fn db008_busy_handling() {
    let (pool, _t, _b) = fresh_seeded_pool().await;

    // hold a write transaction long enough to collide
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO subjects (code, name) VALUES ('HOLD1', 'محجوز')")
        .execute(&mut *tx)
        .await
        .unwrap();

    // the same pool serializes: an insert into another logical area must
    // wait for the outer transaction to finish (max 5s busy timeout)
    let join = tokio::spawn({
        let pool = pool.clone();
        async move {
            sqlx::query("INSERT INTO subjects (code, name) VALUES ('HOLD2', 'ينتظر')")
                .execute(&pool)
                .await
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    tx.commit().await.unwrap();
    let res = join.await.unwrap();
    assert!(res.is_ok(), "queued insert must succeed after commit: {:?}", res.err());

    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM subjects WHERE code LIKE 'HOLD%'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 2, "no lost writes under serialization");
}

// db009 — money columns are integer storage only.
#[tokio::test]
async fn db009_money_no_float() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    let offenders = scan_money_column_storage(&pool).await.unwrap();
    assert!(offenders.is_empty(), "float money columns found: {offenders:?}");
}

// db010 — sequence allocation gaps: rolled-back allocations release numbers.
#[tokio::test]
async fn db010_sequences_gap_free_on_rollback() {
    let (pool, _t, branch_id) = fresh_seeded_pool().await;
    let year = test_utils::test_year();
    {
        let mut conn = pool.acquire().await.unwrap();
        seed_sequence(&mut conn, branch_id, SequenceKind::ReceiptVoucher, year)
            .await
            .unwrap();
    }

    // allocation then rollback (error made the whole tx drop with no commit)
    let result = test_utils::in_tx(&pool, |mut tx| async move {
        let n = allocate_next_no(&mut *tx, branch_id, SequenceKind::ReceiptVoucher, year)
            .await
            .unwrap();
        assert_eq!(n, 1);
        sqlx::query("INSERT INTO subjects (code, name) VALUES ('X', 'X')")
            .execute(&mut *tx)
            .await
            .unwrap();
        Err::<(), _>(std::io::Error::other("intentional rollback: db010"))
    })
    .await;
    assert!(result.is_err());

    // next allocation must re-issue number 1
    let mut conn = pool.acquire().await.unwrap();
    let n = allocate_next_no(&mut conn, branch_id, SequenceKind::ReceiptVoucher, year)
        .await
        .unwrap();
    assert_eq!(n, 1, "no gap; rolled-back allocation's number is reusable");
}

// helper: seeded admin-like user rows needed by FK-heavy flows.
async fn test_utils_user(pool: &sqlx::SqlitePool) -> i64 {
    let row = sqlx::query(
        "INSERT INTO users (username, full_name, password_hash, must_change_password)
         VALUES ('tester', 'مستخدم اختبار', 'argon2:placeholder', 0)
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    row.get::<i64, _>(0)
}

// db_quick_check guard run at every gate as part of the boot path.
#[tokio::test]
async fn db_boot_quick_check_ok() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    assert_eq!(quick_check(&pool).await.unwrap().to_lowercase(), "ok");
}
