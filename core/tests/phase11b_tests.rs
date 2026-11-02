//! Phase 11 continuation: payment voucher lifecycle + payment methods.

use ec_core::database::test_utils::fresh_seeded_pool;
use ec_core::people::context::Actor;
use ec_core::auth::permissions::load_for_user;
use ec_core::vouchers::payment_vouchers::{create_payment_voucher, approve_payment_voucher, pay_payment_voucher};
use sqlx::SqlitePool;

async fn admin_actor(pool: &SqlitePool) -> Actor {
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, full_name, password_hash) VALUES ('pvadmin','Admin','x') RETURNING id",
    ).fetch_one(pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, (SELECT id FROM roles WHERE code='system_admin'))")
        .bind(uid).execute(pool).await.unwrap();
    let ps = load_for_user(pool, uid).await.unwrap();
    Actor { user_id: uid, username: "pvadmin".into(), permissions: ps }
}

#[tokio::test]
async fn vch009_payment_voucher_lifecycle() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let acc: i64 = sqlx::query_scalar("SELECT id FROM accounts LIMIT 1").fetch_one(&pool).await.unwrap();
    let payable: i64 = sqlx::query_scalar("SELECT id FROM accounts LIMIT 1 OFFSET 1").fetch_one(&pool).await.unwrap();
    let pv = create_payment_voucher(&pool, &admin, branch, "teacher", None, "entitlement payout", 1000, "cash", acc, "teacher_entitlement", None, "pvkey1").await.unwrap();
    assert_eq!(pv.status, "draft");
    let approved = approve_payment_voucher(&pool, &admin, pv.id, 999).await.unwrap();
    assert_eq!(approved.status, "approved");
    // pay requires payable balance; seed a payable balance via journal entry
    let lines = vec![
        ec_core::finance::ledger::JournalLine { account_id: payable, debit_minor: 2000, credit_minor: 0, memo: "".into() },
        ec_core::finance::ledger::JournalLine { account_id: acc, debit_minor: 0, credit_minor: 2000, memo: "".into() },
    ];
    ec_core::finance::ledger::post_entry(&pool, &admin, &ec_core::finance::ledger::NewJournalEntry {
        entry_date: "2026-01-01".into(), currency: "ILS".into(), source_type: "adjustment".into(),
        source_id: 0, memo: "seed payable".into(), lines,
    }).await.unwrap();
    let paid = pay_payment_voucher(&pool, &admin, pv.id, payable, acc).await.unwrap();
    assert_eq!(paid.status, "paid");
}

#[tokio::test]
async fn vch010_overpay_blocked() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let acc: i64 = sqlx::query_scalar("SELECT id FROM accounts LIMIT 1").fetch_one(&pool).await.unwrap();
    let payable: i64 = sqlx::query_scalar("SELECT id FROM accounts LIMIT 1 OFFSET 1").fetch_one(&pool).await.unwrap();
    let pv = create_payment_voucher(&pool, &admin, branch, "teacher", None, "payout", 5000, "cash", acc, "teacher_entitlement", None, "pvkey2").await.unwrap();
    approve_payment_voucher(&pool, &admin, pv.id, 999).await.unwrap();
    let r = pay_payment_voucher(&pool, &admin, pv.id, payable, acc).await;
    assert!(r.is_err(), "overpay must be blocked");
    let msg = r.unwrap_err().to_string();
    assert!(msg.contains("overpaid_source") || msg.contains("يتجاوز"), "msg: {}", msg);
}

#[tokio::test]
async fn vch011_payment_method_account_mapping() {
    // method values accepted by schema
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;
    let acc: i64 = sqlx::query_scalar("SELECT id FROM accounts LIMIT 1").fetch_one(&pool).await.unwrap();
    for method in ["cash", "card", "transfer", "wallet"] {
        let pv = create_payment_voucher(&pool, &admin, branch, "other", None, "m", 100, method, acc, "other", None, &format!("mk{}", method)).await;
        assert!(pv.is_ok(), "method {} should be accepted", method);
    }
    // invalid method rejected by DB CHECK
    let r = create_payment_voucher(&pool, &admin, branch, "other", None, "m", 100, "bitcoin", acc, "other", None, "mkbad").await;
    assert!(r.is_err());
}
