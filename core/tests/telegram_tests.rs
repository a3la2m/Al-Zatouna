//! Phase 20: Telegram tables and seeding. Network calls are not tested here
//! (they require a real bot token); we verify the schema and the pure helpers.

use ec_core::database::test_utils::fresh_seeded_pool;

#[tokio::test]
async fn tg001_schema_and_permission_seeded() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    for table in ["telegram_settings", "telegram_chats", "telegram_messages",
                  "telegram_notifications", "telegram_flow_state",
                  "telegram_link_codes", "app_notifications", "service_requests"] {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?")
            .bind(table).fetch_one(&pool).await.unwrap();
        assert_eq!(n, 1, "table {table} exists");
    }
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM permissions WHERE code='notifications.manage'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(n, 1);
    let granted: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM role_permissions rp JOIN roles r ON r.id=rp.role_id
         JOIN permissions p ON p.id=rp.permission_id
         WHERE r.code='system_admin' AND p.code='notifications.manage'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(granted, 1);
}

#[tokio::test]
async fn tg002_chat_link_and_log_tables_work() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    let branch: i64 = sqlx::query_scalar("SELECT id FROM branches WHERE code='MAIN'").fetch_one(&pool).await.unwrap();
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (branch_id, student_no, full_name, phone) VALUES (?,1,'طالب تلجرام','05xxxxxxxx') RETURNING id",
    ).bind(branch).fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO telegram_chats (chat_id,person_type,person_id,phone) VALUES ('12345','student',?,'05xxxxxxxx')")
        .bind(sid).execute(&pool).await.unwrap();
    let chat: String = sqlx::query_scalar("SELECT chat_id FROM telegram_chats WHERE person_type='student' AND person_id=?")
        .bind(sid).fetch_one(&pool).await.unwrap();
    assert_eq!(chat, "12345");

    sqlx::query("INSERT INTO telegram_messages (chat_id,person_type,person_id,direction,body,kind) VALUES ('12345','student',?,'out','مرحباً','test')")
        .bind(sid).execute(&pool).await.unwrap();
    let body: String = sqlx::query_scalar("SELECT body FROM telegram_messages WHERE chat_id='12345'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(body, "مرحباً");
}

#[tokio::test]
async fn tg003_notifications_inbox() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    let id = ec_core::notifications::push(&pool, "payment", "سند قبض", "تم استلام 100", "receipt_voucher", Some(1), "", None)
        .await.unwrap();
    assert!(id > 0);
    assert_eq!(ec_core::notifications::unseen_count(&pool).await.unwrap(), 1);
    let list = ec_core::notifications::list(&pool, 10).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].title, "سند قبض");
    ec_core::notifications::mark_seen(&pool, id).await.unwrap();
    assert_eq!(ec_core::notifications::unseen_count(&pool).await.unwrap(), 0);
}

#[tokio::test]
async fn tg004_user_phone_and_link_codes_schema() {
    let (pool, _t, _b) = fresh_seeded_pool().await;
    // users gained a phone column (Phase 23)
    let has_phone: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name='phone'",
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(has_phone, 1, "users.phone exists");

    // a one-time link code row can be stored and consumed
    sqlx::query(
        "INSERT INTO telegram_link_codes (code,person_type,person_id,expires_at)
         VALUES ('LNK-TEST-CODE','manager',1,'2999-01-01T00:00:00+00:00')",
    ).execute(&pool).await.unwrap();
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM telegram_link_codes WHERE code='LNK-TEST-CODE' AND used=0",
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(n, 1);
    sqlx::query("UPDATE telegram_link_codes SET used=1 WHERE code='LNK-TEST-CODE'")
        .execute(&pool).await.unwrap();
    let unused: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM telegram_link_codes WHERE code='LNK-TEST-CODE' AND used=0",
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(unused, 0);
}
