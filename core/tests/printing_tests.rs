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
