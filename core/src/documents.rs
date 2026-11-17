//! Document composition (Phase 17): builds a Typst template from real DB rows
//! plus the center's header / footer / logo settings. The printed document is
//! assembled here so both preview and execute use exactly the same bytes.

use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::AppResult;

pub struct RenderedDocument {
    pub template: String,
    pub assets: Vec<(String, Vec<u8>)>,
}

fn esc(s: &str) -> String {
    // Typst markup: escape backslash, hash, brackets that could appear in data.
    s.replace('\\', "\\\\")
        .replace('#', "\\#")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

fn opt_str(r: &sqlx::sqlite::SqliteRow, col: &str) -> String {
    r.try_get::<String, _>(col).unwrap_or_default()
}

/// Compose the shared header (center name, logo, identity) + footer.
fn shell(center: &crate::settings_center::CenterSettings, body: &str, profile: &Profile) -> String {
    let page = format!(
        "#set page(paper: \"a4\", margin: (top: {}mm, bottom: {}mm, left: {}mm, right: {}mm))\n",
        profile.margin_top, profile.margin_bottom, profile.margin_left, profile.margin_right
    );
    let mut out = String::new();
    out.push_str(&page);
    out.push_str(&format!(
        "#set text(font: \"Noto Sans Arabic\", size: {}pt, dir: rtl)\n",
        profile.font_size
    ));
    out.push_str("#set par(spacing: 0.6em)\n");

    // header
    if profile.show_header {
        out.push_str("#grid(\n  columns: (auto, 1fr),\n  align: (left, right),\n\n");
        if !center.logo_ref.is_empty() {
            out.push_str("  [#box(width: 20mm, height: 20mm, image(\"logo.png\", width: 100%, fit: \"contain\"))],\n");
        } else {
            out.push_str("  [],\n");
        }
        out.push_str(&format!("  [\n    #text(size: 15pt, weight: \"bold\")[{}]\n", esc(&center.name)));
        if !center.trade_name.is_empty() && center.trade_name != center.name {
            out.push_str(&format!("    #text(size: 10pt)[{}]\n", esc(&center.trade_name)));
        }
        let mut id_line: Vec<String> = Vec::new();
        if !center.license_no.is_empty() { id_line.push(format!("رخصة: {}", center.license_no)); }
        if !center.tax_no.is_empty() { id_line.push(format!("ضريبي: {}", center.tax_no)); }
        if !id_line.is_empty() {
            out.push_str(&format!("    #text(size: 8pt)[{}]\n", esc(&id_line.join("  •  "))));
        }
        out.push_str("  ],\n)\n");
        out.push_str("#line(length: 100%, stroke: 0.8pt + rgb(29, 51, 87))\n");
        if !center.header_text.is_empty() {
            out.push_str(&format!("#align(center)[#text(size: 8pt)[{}]]\n", esc(&center.header_text)));
        }
        out.push_str("#v(6pt)\n");
    }

    out.push_str(body);

    if profile.show_footer {
        out.push_str("#v(1fr)\n#line(length: 100%, stroke: 0.5pt + rgb(136, 148, 171))\n");
        let mut foot: Vec<String> = Vec::new();
        if !center.address.is_empty() { foot.push(center.address.clone()); }
        if !center.phones.is_empty() { foot.push(format!("هاتف: {}", center.phones)); }
        if !center.email.is_empty() { foot.push(center.email.clone()); }
        if !center.website.is_empty() { foot.push(center.website.clone()); }
        if !foot.is_empty() {
            out.push_str(&format!("#align(center)[#text(size: 7.5pt)[{}]]\n", esc(&foot.join("  •  "))));
        }
        if !center.footer_text.is_empty() {
            out.push_str(&format!("#align(center)[#text(size: 8pt)[{}]]\n", esc(&center.footer_text)));
        }
        out.push_str("#align(center)[#text(size: 7pt)[صفحة #counter(page)]]\n");
    }
    out
}

#[derive(Clone)]
pub struct Profile {
    pub code: String,
    pub show_header: bool,
    pub show_footer: bool,
    pub margin_top: f64,
    pub margin_bottom: f64,
    pub margin_left: f64,
    pub margin_right: f64,
    pub font_size: f64,
}

pub async fn profile(pool: &SqlitePool, code: &str) -> AppResult<Profile> {
    let r = sqlx::query(
        "SELECT code, show_header, show_footer, margins_json, font_size_pt
         FROM print_profiles WHERE code = ?",
    )
    .bind(code)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("قالب الطباعة غير موجود"))?;
    let margins: serde_json::Value = serde_json::from_str(&opt_str(&r, "margins_json"))
        .unwrap_or(serde_json::json!({}));
    let mm = margins.get("mm").cloned().unwrap_or(serde_json::json!({}));
    let m = |k: &str, d: f64| mm.get(k).and_then(|v| v.as_f64()).unwrap_or(d);
    Ok(Profile {
        code: opt_str(&r, "code"),
        show_header: r.get::<i64, _>("show_header") == 1,
        show_footer: r.get::<i64, _>("show_footer") == 1,
        margin_top: m("top", 15.0),
        margin_bottom: m("bottom", 15.0),
        margin_left: m("left", 15.0),
        margin_right: m("right", 15.0),
        font_size: r.get::<i64, _>("font_size_pt") as f64,
    })
}

/// Load the logo bytes from the data directory if `logo_ref` is set.
pub fn load_logo(data_dir: &std::path::Path, logo_ref: &str, assets: &mut Vec<(String, Vec<u8>)>) {
    if logo_ref.is_empty() { return; }
    let path = if std::path::Path::new(logo_ref).is_absolute() {
        std::path::PathBuf::from(logo_ref)
    } else {
        data_dir.join("assets").join(logo_ref)
    };
    if let Ok(bytes) = std::fs::read(&path) {
        // Typst detects the format from the extension; use a neutral name with
        // the same extension as the source file.
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png");
        assets.push((format!("logo.{}", ext), bytes));
    }
}

fn rupees(minor: i64, decimals: i64) -> String {
    let div = 10i64.pow(decimals as u32);
    format!("{:.2}", minor as f64 / div as f64)
}

/// Build the document for a receipt voucher or a payment voucher.
pub async fn build_voucher(
    pool: &SqlitePool,
    data_dir: &std::path::Path,
    kind: &str, // "receipt" | "payment"
    id: i64,
) -> AppResult<RenderedDocument> {
    let center = crate::settings_center::get(pool).await?;
    let code = if kind == "receipt" { "voucher.receipt" } else { "voucher.payment" };
    let prof = profile(pool, code).await?;
    let mut assets = Vec::new();
    load_logo(data_dir, &center.logo_ref, &mut assets);

    let body = if kind == "receipt" {
        let r = sqlx::query(
            "SELECT rv.voucher_no, rv.voucher_date, rv.payer_name, rv.amount_minor,
                    rv.method, rv.narrative, rv.status,
                    COALESCE(st.full_name,'') AS student_name,
                    COALESCE(c.name,'') AS course_name
             FROM receipt_vouchers rv
             LEFT JOIN students st ON st.id=rv.student_id
             LEFT JOIN courses c ON c.id=rv.course_id
             WHERE rv.id=?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("سند القبض غير موجود"))?;
        let method_raw = opt_str(&r, "method");
        let method = match method_raw.as_str() {
            "cash" => "نقدي", "card" => "بطاقة", "transfer" => "تحويل", "wallet" => "محفظة", x => x,
        };
        format!(
            r#"
#align(center)[#text(size: 16pt, weight: "bold")[سند قبض]]
#align(center)[#text(size: 9pt)[رقم السند: {} — التاريخ: {}]]
#v(10pt)
#table(
  columns: (1fr, 2fr),
  stroke: 0.5pt + rgb(203, 213, 224),
  inset: 7pt,
  [اسم الطالب], [{}],
  [الدورة], [{}],
  [المبلغ], [{} {}],
  [طريقة الدفع], [{}],
  [البيان], [{}],
  [الحالة], [{}],
)
#v(14pt)
#grid(columns: (1fr, 1fr), [الاسم: ............................], [التوقيع: ............................])
"#,
            opt_str(&r, "voucher_no"), opt_str(&r, "voucher_date"),
            esc(&opt_str(&r, "payer_name")), esc(&opt_str(&r, "course_name")),
            rupees(r.get::<i64, _>("amount_minor"), center.currency_decimals), center.currency_code,
            method, esc(&opt_str(&r, "narrative")), esc(&opt_str(&r, "status")),
        )
    } else {
        let r = sqlx::query(
            "SELECT pv.voucher_no, pv.voucher_date, pv.beneficiary_name, pv.amount_minor,
                    pv.method, pv.purpose, pv.status,
                    COALESCE(t.full_name,'') AS teacher_name
             FROM payment_vouchers pv
             LEFT JOIN teachers t ON t.id=pv.teacher_id
             WHERE pv.id=?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("سند الصرف غير موجود"))?;
        let beneficiary = {
            let t = opt_str(&r, "teacher_name");
            if !t.is_empty() { t } else { opt_str(&r, "beneficiary_name") }
        };
        let method_raw = opt_str(&r, "method");
        let method = match method_raw.as_str() {
            "cash" => "نقدي", "card" => "بطاقة", "transfer" => "تحويل", "wallet" => "محفظة", x => x,
        };
        format!(
            r#"
#align(center)[#text(size: 16pt, weight: "bold")[سند صرف]]
#align(center)[#text(size: 9pt)[رقم السند: {} — التاريخ: {}]]
#v(10pt)
#table(
  columns: (1fr, 2fr),
  stroke: 0.5pt + rgb(203, 213, 224),
  inset: 7pt,
  [المستفيد], [{}],
  [الغرض], [{}],
  [المبلغ], [{} {}],
  [طريقة الدفع], [{}],
  [الحالة], [{}],
)
#v(14pt)
#grid(columns: (1fr, 1fr), [المستلم: ............................], [المدير: ............................])
"#,
            opt_str(&r, "voucher_no"), opt_str(&r, "voucher_date"),
            esc(&beneficiary), esc(&opt_str(&r, "purpose")),
            rupees(r.get::<i64, _>("amount_minor"), center.currency_decimals), center.currency_code,
            method, esc(&opt_str(&r, "status")),
        )
    };

    Ok(RenderedDocument {
        template: shell(&center, &body, &prof),
        assets,
    })
}

/// Build the enrollment receipt document (uses the receipt profile).
pub async fn build_enrollment_receipt(
    pool: &SqlitePool,
    data_dir: &std::path::Path,
    enrollment_id: i64,
) -> AppResult<RenderedDocument> {
    let voucher: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM receipt_vouchers WHERE enrollment_id=? AND idempotency_key LIKE 'enr-%' ORDER BY id DESC LIMIT 1",
    )
    .bind(enrollment_id)
    .fetch_optional(pool)
    .await?;
    if let Some(id) = voucher {
        return build_voucher(pool, data_dir, "receipt", id).await;
    }
    // fall back to a direct enrollment statement
    let center = crate::settings_center::get(pool).await?;
    let prof = profile(pool, "enrollment.receipt").await?;
    let mut assets = Vec::new();
    load_logo(data_dir, &center.logo_ref, &mut assets);
    let r = sqlx::query(
        "SELECT e.id, e.enrolled_date, e.net_fee_minor, st.full_name AS student_name,
                c.name AS course_name
         FROM enrollments e JOIN students st ON st.id=e.student_id
         JOIN courses c ON c.id=e.course_id WHERE e.id=?",
    )
    .bind(enrollment_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("التسجيل غير موجود"))?;
    let body = format!(
        r#"
#align(center)[#text(size: 16pt, weight: "bold")[إيصال تسجيل]]
#v(10pt)
#table(
  columns: (1fr, 2fr), stroke: 0.5pt + rgb(203, 213, 224), inset: 7pt,
  [الطالب], [{}],
  [الدورة], [{}],
  [تاريخ التسجيل], [{}],
  [الرسوم], [{} {}],
)
"#,
        esc(&opt_str(&r, "student_name")), esc(&opt_str(&r, "course_name")),
        opt_str(&r, "enrolled_date"),
        rupees(r.get::<i64, _>("net_fee_minor"), center.currency_decimals), center.currency_code,
    );
    Ok(RenderedDocument { template: shell(&center, &body, &prof), assets })
}

/// Build a weekly study schedule document for one teacher or one room.
/// Uses the center's header/logo/footer settings and the `schedule.study` profile.
pub async fn build_schedule(
    pool: &SqlitePool,
    data_dir: &std::path::Path,
    kind: &str, // "teacher" | "room"
    id: i64,
) -> AppResult<RenderedDocument> {
    let center = crate::settings_center::get(pool).await?;
    let prof = profile(pool, "schedule.study").await?;
    let mut assets = Vec::new();
    load_logo(data_dir, &center.logo_ref, &mut assets);

    let (title, subtitle, rows) = if kind == "teacher" {
        let t: (String,) = sqlx::query_as("SELECT full_name FROM teachers WHERE id=?")
            .bind(id).fetch_optional(pool).await?
            .ok_or_else(|| AppError::not_found("المدرس غير موجود"))?;
        let rows = sqlx::query(
            "SELECT cs.weekday, cs.start_time, cs.end_time, c.name AS course_name,
                    COALESCE(r.name,'') AS room_name, COALESCE(t2.full_name,'') AS teacher_name
             FROM course_schedules cs
             JOIN courses c ON c.id=cs.course_id
             LEFT JOIN rooms r ON r.id=cs.room_id
             LEFT JOIN teachers t2 ON t2.id=cs.teacher_id
             WHERE cs.teacher_id=? OR c.teacher_id=?
             ORDER BY cs.weekday, cs.start_time",
        ).bind(id).bind(id).fetch_all(pool).await?;
        (format!("جدول المدرس: {}", t.0), "المواعد الأسبوعية للمدرس".to_string(), rows)
    } else {
        let r: (String,) = sqlx::query_as("SELECT name FROM rooms WHERE id=?")
            .bind(id).fetch_optional(pool).await?
            .ok_or_else(|| AppError::not_found("القاعة غير موجودة"))?;
        let rows = sqlx::query(
            "SELECT cs.weekday, cs.start_time, cs.end_time, c.name AS course_name,
                    COALESCE(rm.name,'') AS room_name, COALESCE(t.full_name,'') AS teacher_name
             FROM course_schedules cs
             JOIN courses c ON c.id=cs.course_id
             LEFT JOIN rooms rm ON rm.id=cs.room_id
             LEFT JOIN teachers t ON t.id=cs.teacher_id
             WHERE cs.room_id=?
             ORDER BY cs.weekday, cs.start_time",
        ).bind(id).fetch_all(pool).await?;
        (format!("جدول القاعة: {}", r.0), "حجوزات القاعة الأسبوعية".to_string(), rows)
    };

    let days = ["", "الاثنين", "الثلاثاء", "الأربعاء", "الخميس", "الجمعة", "السبت", "الأحد"];
    let mut body = format!(
        "#align(center)[#text(size: 16pt, weight: \"bold\")[{}]]\n#align(center)[#text(size: 9pt)[{}]]\n#v(10pt)\n",
        esc(&title), esc(&subtitle)
    );
    if rows.is_empty() {
        body.push_str("#align(center)[#text(size: 11pt)[لا توجد مواعيد مجدولة]]\n");
    } else {
        body.push_str(
            "#table(\n  columns: (1fr, 1fr, 1fr, 1.6fr, 1.2fr, 1.2fr),\n  stroke: 0.5pt + rgb(203, 213, 224),\n  inset: 6pt,\n  [اليوم], [من], [إلى], [الدورة], [القاعة], [المدرس],\n"
        );
        for r in &rows {
            let wd: i64 = r.get("weekday");
            let day = days.get(wd as usize).copied().unwrap_or("");
            body.push_str(&format!(
                "[{}], [{}], [{}], [{}], [{}], [{}],\n",
                day,
                esc(&opt_str(r, "start_time")),
                esc(&opt_str(r, "end_time")),
                esc(&opt_str(r, "course_name")),
                esc(&opt_str(r, "room_name")),
                esc(&opt_str(r, "teacher_name")),
            ));
        }
        body.push_str(")\n");
    }
    Ok(RenderedDocument { template: shell(&center, &body, &prof), assets })
}
