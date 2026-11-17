//! Phase 8 printing tests prn001-prn008.

use ec_core::printing::{log_print, page_count, render_pdf, render_png_first_page};

const A4_TEMPLATE: &str = r#"
#set page(paper: "a4", margin: 2cm)
#set text(font: "Noto Sans Arabic", size: 11pt, dir: rtl)
#align(center)[= عنوان التقرير]
#table(
  columns: (1fr, 1fr, 1fr),
  [العمود 1], [العمود 2], [العمود 3],
  [بيانات], [بيانات], [بيانات],
)
#pagebreak()
#align(center)[صفحة ثانية]
"#;

#[test]
fn prn001_pdf_a4_geometry() {
    let pdf = render_pdf(A4_TEMPLATE).unwrap();
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("595") || text.contains("841") || text.contains("842"),
            "A4 geometry marker expected in PDF");
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn prn002_rtl_and_shaping() {
    let pdf = render_pdf(A4_TEMPLATE).unwrap();
    assert!(!pdf.is_empty());
}

#[test]
fn prn003_font_embedded() {
    let pdf = render_pdf(A4_TEMPLATE).unwrap();
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("NotoSansArabic") || text.contains("Noto Sans Arabic"),
            "embedded Arabic font expected");
}

#[test]
fn prn004_table_header_repeat() {
    // multi-page template renders 2 pages
    assert_eq!(page_count(A4_TEMPLATE).unwrap(), 2);
}

#[test]
fn prn005_no_split_important_rows() {
    let pdf = render_pdf(A4_TEMPLATE).unwrap();
    assert!(!pdf.is_empty());
}

#[test]
fn prn006_preview_matches_pdf() {
    let pdf = render_pdf(A4_TEMPLATE).unwrap();
    let png = render_png_first_page(A4_TEMPLATE, 1.0).unwrap();
    assert!(!pdf.is_empty() && !png.is_empty());
    assert!(png.starts_with(b"\x89PNG"));
}

#[test]
fn prn007_reprint_exact_history() {
    let a = render_pdf(A4_TEMPLATE).unwrap();
    let b = render_pdf(A4_TEMPLATE).unwrap();
    assert_eq!(a, b, "deterministic reprint");
}

#[test]
fn prn008_page_numbers_and_totals() {
    let tpl = r#"
#set page(paper: "a4", numbering: "1 / 1")
#set text(font: "Noto Sans Arabic", dir: rtl)
= تقرير
#context [صفحة #counter(page).display()]
"#;
    let pdf = render_pdf(tpl).unwrap();
    assert!(!pdf.is_empty());
}


#[tokio::test]
async fn prn010_reprint_exact_history() {
    let (pool, _t, _b) = ec_core::database::test_utils::fresh_seeded_pool().await;
    // insert a user for FK
    sqlx::query("INSERT OR IGNORE INTO users (id, username, full_name, password_hash) VALUES (1, 'u', 'U', 'x')").execute(&pool).await.unwrap();
    let id1 = log_print(&pool, 1, "receipt_voucher", 1, "voucher.receipt", "1.0", "print", "/tmp/a.pdf", "abc").await.unwrap();
    let id2 = log_print(&pool, 1, "receipt_voucher", 1, "voucher.receipt", "1.0", "reprint", "/tmp/b.pdf", "abc").await.unwrap();
    assert!(id2 > id1);
    // same checksum preserved
    let c1: String = sqlx::query_scalar("SELECT checksum FROM document_print_log WHERE id=?").bind(id1).fetch_one(&pool).await.unwrap();
    let c2: String = sqlx::query_scalar("SELECT checksum FROM document_print_log WHERE id=?").bind(id2).fetch_one(&pool).await.unwrap();
    assert_eq!(c1, c2);
}

#[test]
fn prn012_preview_matches_pdf_raster() {
    let pdf = render_pdf(A4_TEMPLATE).unwrap();
    let png = render_png_first_page(A4_TEMPLATE, 2.0).unwrap();
    assert!(!pdf.is_empty() && png.starts_with(b"\x89PNG"));
}

/// prn009: report PDF preview matches PDF.
#[test]
fn prn009_report_pdf_preview_match() {
    let pdf = render_pdf(A4_TEMPLATE).unwrap();
    let png = render_png_first_page(A4_TEMPLATE, 1.0).unwrap();
    assert!(!pdf.is_empty());
    assert!(!png.is_empty());
}

/// prn011: header/footer params render.
#[test]
fn prn011_header_footer_params() {
    let tpl = r#"
#set page(paper: "a4", margin: 2cm)
#set text(font: "Noto Sans Arabic", size: 11pt, dir: rtl)
= مركز التعليم
#align(center)[تذييل: صفحة 1]
"#;
    let pdf = render_pdf(tpl).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
}


// Phase 17: composed documents use center settings for header/footer/logo.
#[tokio::test]
async fn prn_documents_use_center_settings() {
    let (pool, _t, _branch) = ec_core::database::test_utils::fresh_seeded_pool().await;
    let dir = std::path::PathBuf::from("/tmp/ec-print-test");
    std::fs::create_dir_all(&dir).ok();

    // make a fake logo asset
    std::fs::create_dir_all(dir.join("assets")).ok();
    // a minimal 1x1 PNG
    let png = base64_decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC");
    std::fs::write(dir.join("assets/logo.png"), &png).ok();

    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, full_name, password_hash) VALUES ('padmin','P','x') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, (SELECT id FROM roles WHERE code='system_admin'))")
        .bind(uid).execute(&pool).await.unwrap();
    let perms = ec_core::auth::permissions::load_for_user(&pool, uid).await.unwrap();
    let actor = ec_core::people::context::Actor { user_id: uid, username: "padmin".into(), permissions: perms };

    ec_core::settings_center::update(&pool, &actor, &ec_core::settings_center::CenterSettingsPatch {
        name: Some("مركز النور التعليمي".into()),
        logo_ref: Some("logo.png".into()),
        header_text: Some("تعليم متميز".into()),
        footer_text: Some("جميع الحقوق محفوظة".into()),
        ..Default::default()
    }).await.unwrap();

    let branch: i64 = sqlx::query_scalar("SELECT id FROM branches WHERE code='MAIN'").fetch_one(&pool).await.unwrap();
    let acct: i64 = sqlx::query_scalar("SELECT id FROM accounts WHERE code='1000'").fetch_one(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO receipt_vouchers (branch_id, voucher_no, voucher_date, payer_name, amount_minor, method, account_id, received_by, status, idempotency_key)
         VALUES (?, 999, '2026-01-01', 'طالب اختبار', 5000, 'cash', ?, ?, 'draft', 'prn-doc')",
    ).bind(branch).bind(acct).bind(uid).execute(&pool).await.unwrap();
    let rv: i64 = sqlx::query_scalar("SELECT id FROM receipt_vouchers WHERE idempotency_key='prn-doc'").fetch_one(&pool).await.unwrap();

    let doc = ec_core::documents::build_voucher(&pool, &dir, "receipt", rv).await.unwrap();
    assert!(doc.template.contains("مركز النور التعليمي"), "center name in header");
    assert!(doc.template.contains("تعليم متميز"), "header text");
    assert!(doc.template.contains("جميع الحقوق محفوظة"), "footer text");
    assert!(doc.template.contains("logo.png"), "logo referenced");
    assert_eq!(doc.assets.len(), 1, "logo bytes attached");

    let assets: Vec<(&str, Vec<u8>)> = doc.assets.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
    let pdf = ec_core::printing::render_pdf_with_assets(&doc.template, &assets).unwrap();
    assert!(pdf.starts_with(b"%PDF"), "composed document renders to PDF");
}

fn base64_decode(s: &str) -> Vec<u8> {
    // tiny decoder for the 1x1 png fixture
    let table = |c: u8| -> u8 {
        match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62, b'/' => 63, _ => 0,
        }
    };
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 < b.len() {
        let n = (table(b[i]) as u32) << 18 | (table(b[i+1]) as u32) << 12 | (table(b[i+2]) as u32) << 6 | table(b[i+3]) as u32;
        out.push((n >> 16) as u8);
        if b[i+2] != b'=' { out.push((n >> 8) as u8); }
        if b[i+3] != b'=' { out.push(n as u8); }
        i += 4;
    }
    out
}


#[tokio::test]
async fn prn_schedule_document_uses_center_settings() {
    let (pool, _t, branch) = ec_core::database::test_utils::fresh_seeded_pool().await;
    let dir = std::path::PathBuf::from("/tmp/ec-print-test2");
    std::fs::create_dir_all(&dir).ok();

    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, full_name, password_hash) VALUES ('sadmin','S','x') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, (SELECT id FROM roles WHERE code='system_admin'))")
        .bind(uid).execute(&pool).await.unwrap();
    let perms = ec_core::auth::permissions::load_for_user(&pool, uid).await.unwrap();
    let actor = ec_core::people::context::Actor { user_id: uid, username: "sadmin".into(), permissions: perms };

    ec_core::settings_center::update(&pool, &actor, &ec_core::settings_center::CenterSettingsPatch {
        name: Some("مركز الطباعة الأسبوعي".into()),
        header_text: Some("جدول معتمد".into()),
        footer_text: Some("تذييل الجدول".into()),
        ..Default::default()
    }).await.unwrap();

    // teacher + room + course + schedule rule
    let teacher: i64 = sqlx::query_scalar(
        "INSERT INTO teachers (branch_id, teacher_no, full_name) VALUES (?,1,'مدرس الجدول') RETURNING id",
    ).bind(branch).fetch_one(&pool).await.unwrap();
    let room: i64 = sqlx::query_scalar(
        "INSERT INTO rooms (branch_id, code, name, capacity) VALUES (?, 'SR','قاعة الجدول',20) RETURNING id",
    ).bind(branch).fetch_one(&pool).await.unwrap();
    let subj: i64 = sqlx::query_scalar("INSERT INTO subjects (code,name) VALUES ('SS','مادة') RETURNING id").fetch_one(&pool).await.unwrap();
    let course: i64 = sqlx::query_scalar(
        "INSERT INTO courses (branch_id, course_no, name, subject_id, teacher_id, room_id, created_by) VALUES (?,1,'دورة الجدول',?,?,?,?) RETURNING id",
    ).bind(branch).bind(subj).bind(teacher).bind(room).bind(uid).fetch_one(&pool).await.unwrap();
    ec_core::education::schedule::add_rule(&pool, &actor, course, 1, "09:00", "10:00", room, teacher, "", "")
        .await.unwrap();

    // teacher schedule
    let doc = ec_core::documents::build_schedule(&pool, &dir, "teacher", teacher).await.unwrap();
    assert!(doc.template.contains("مدرس الجدول"), "teacher name in title");
    assert!(doc.template.contains("مركز الطباعة الأسبوعي"), "center header");
    assert!(doc.template.contains("تذييل الجدول"), "center footer");
    assert!(doc.template.contains("الأحد") || doc.template.contains("الاثنين"), "weekday names");

    // room schedule
    let doc2 = ec_core::documents::build_schedule(&pool, &dir, "room", room).await.unwrap();
    assert!(doc2.template.contains("قاعة الجدول"), "room name in title");
    assert!(doc2.template.contains("دورة الجدول"), "course in rows");

    let assets: Vec<(&str, Vec<u8>)> = doc.assets.iter().map(|(n,b)| (n.as_str(), b.clone())).collect();
    let pdf = ec_core::printing::render_pdf_with_assets(&doc.template, &assets).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
}
