//! Telegram integration (Phase 20): Bot API client, chat linking, notifications,
//! message log, and an interactive query/recovery bot. Hand-rolled over the
//! Telegram Bot HTTP API (no external crate), mirroring the proven architecture
//! from the Employee-Attendance-System project.

use chrono::Timelike;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use std::time::Duration;

const API: &str = "https://api.telegram.org/bot";

// ---------------------------------------------------------------- settings

pub async fn setting(pool: &SqlitePool, key: &str, default: &str) -> String {
    sqlx::query_scalar::<_, String>("SELECT value FROM telegram_settings WHERE key=?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| default.to_string())
}

pub async fn set_setting(pool: &SqlitePool, key: &str, value: &str) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO telegram_settings (key,value) VALUES (?,?)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

fn enabled(s: &str) -> bool {
    s == "1" || s.eq_ignore_ascii_case("true")
}

// ---------------------------------------------------------------- HTTP / API

fn api(token: &str, method: &str, body: Value, timeout_secs: u64) -> Result<Value, String> {
    let url = format!("{API}{}/{method}", token.trim());
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .map_err(|e| e.to_string())?;
    let v: Value = resp.json().map_err(|e| e.to_string())?;
    if v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) {
        Ok(v.get("result").cloned().unwrap_or(Value::Null))
    } else {
        Err(v.get("description").and_then(|x| x.as_str()).unwrap_or("Telegram API error").to_string())
    }
}

/// Send a message, splitting to 3800-char chunks (Telegram's 4096 limit).
fn send_raw(token: &str, chat: &str, text: &str, keyboard: Option<Value>) -> Result<(), String> {
    let chars: Vec<char> = text.chars().collect();
    let chunks: Vec<String> = chars
        .chunks(3800)
        .map(|c| c.iter().collect::<String>())
        .collect();
    for (i, chunk) in chunks.iter().enumerate() {
        let mut body = json!({
            "chat_id": chat,
            "text": chunk,
            "disable_web_page_preview": true,
        });
        if i == 0 {
            if let Some(kb) = &keyboard {
                body["reply_markup"] = kb.clone();
            }
        }
        api(token, "sendMessage", body, 30)?;
    }
    Ok(())
}

/// Send and log to the message history (best effort).
pub async fn send_message(
    pool: &SqlitePool,
    chat: &str,
    text: &str,
    kind: &str,
    keyboard: Option<Value>,
) -> Result<(), String> {
    let token = setting(pool, "tg_token", "").await;
    if token.trim().is_empty() {
        return Err("لم يتم إعداد رمز البوت".into());
    }
    send_raw(&token, chat, text, keyboard)?;
    let person = link_by_chat(pool, chat).await?;
    let (ptype, pid) = person
        .as_ref()
        .map(|p| (p.person_type.clone(), Some(p.person_id)))
        .unwrap_or_else(|| (String::new(), None));
    log_message(pool, chat, &ptype, pid, "out", text, kind).await;
    Ok(())
}

/// Fire-and-forget send (used by on-event notifications from sync commands).
#[allow(dead_code)]
pub async fn send_async(pool: SqlitePool, chat: String, text: String, kind: String) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        if let Ok(rt) = rt {
            rt.block_on(async {
                let _ = send_message(&pool, &chat, &text, &kind, None).await;
            });
        }
    });
}

// ---------------------------------------------------------------- message log

pub async fn log_message(
    pool: &SqlitePool,
    chat: &str,
    person_type: &str,
    person_id: Option<i64>,
    direction: &str,
    body: &str,
    kind: &str,
) {
    sqlx::query(
        "INSERT INTO telegram_messages (chat_id,person_type,person_id,direction,body,kind)
         VALUES (?,?,?,?,?,?)",
    )
    .bind(chat)
    .bind(person_type)
    .bind(person_id)
    .bind(direction)
    .bind(body)
    .bind(kind)
    .execute(pool)
    .await
    .ok();
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct ChatMessage {
    pub id: i64,
    pub chat_id: String,
    pub person_type: String,
    pub person_id: Option<i64>,
    pub direction: String,
    pub body: String,
    pub kind: String,
    pub created_at: String,
    pub person_name: String,
}

pub async fn list_messages(
    pool: &SqlitePool,
    chat: Option<&str>,
    kind: Option<&str>,
    search: Option<&str>,
    limit: i64,
) -> Result<Vec<ChatMessage>, String> {
    let limit = limit.clamp(1, 1000);
    let rows = sqlx::query(
        "SELECT m.*,
           COALESCE(
             (SELECT st.full_name FROM students st WHERE m.person_type='student' AND st.id=m.person_id),
             (SELECT tc.full_name FROM teachers tc WHERE m.person_type='teacher' AND tc.id=m.person_id),
             (SELECT u.full_name FROM users u WHERE m.person_type='manager' AND u.id=m.person_id),
             ''
           ) AS person_name
         FROM telegram_messages m
         WHERE (?1 IS NULL OR m.chat_id=?1)
           AND (?2 IS NULL OR m.kind=?2)
         ORDER BY m.id DESC LIMIT ?3",
    )
    .bind(chat)
    .bind(kind)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let needle = search.map(|s| s.to_lowercase());
    Ok(rows
        .iter()
        .map(|r| ChatMessage {
            id: r.get("id"),
            chat_id: r.get("chat_id"),
            person_type: r.get("person_type"),
            person_id: r.get("person_id"),
            direction: r.get("direction"),
            body: r.get("body"),
            kind: r.get("kind"),
            created_at: r.get("created_at"),
            person_name: r.get("person_name"),
        })
        .filter(|m| {
            needle.as_ref().map_or(true, |n| {
                m.body.to_lowercase().contains(n) || m.person_name.to_lowercase().contains(n)
            })
        })
        .collect())
}

// ---------------------------------------------------------------- linking

#[derive(Debug, serde::Serialize, Clone)]
pub struct ChatLink {
    pub id: i64,
    pub chat_id: String,
    pub person_type: String,
    pub person_id: i64,
    pub person_name: String,
    pub phone: String,
    pub linked_at: String,
    pub active: bool,
}

pub fn normalize_phone(s: &str) -> String {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 9 {
        digits[digits.len() - 9..].to_string()
    } else {
        digits
    }
}

pub async fn link_by_chat(pool: &SqlitePool, chat: &str) -> Result<Option<ChatLink>, String> {
    let r = sqlx::query(
        "SELECT c.*,
           COALESCE(
             (SELECT st.full_name FROM students st WHERE c.person_type='student' AND st.id=c.person_id),
             (SELECT tc.full_name FROM teachers tc WHERE c.person_type='teacher' AND tc.id=c.person_id),
             (SELECT u.full_name FROM users u WHERE c.person_type='manager' AND u.id=c.person_id),
             ''
           ) AS person_name
         FROM telegram_chats c WHERE c.chat_id=? AND c.active=1",
    )
    .bind(chat)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(r.map(|r| ChatLink {
        id: r.get("id"), chat_id: r.get("chat_id"), person_type: r.get("person_type"),
        person_id: r.get("person_id"), person_name: r.get("person_name"),
        phone: r.get("phone"), linked_at: r.get("linked_at"),
        active: r.get::<i64, _>("active") == 1,
    }))
}

pub async fn list_links(pool: &SqlitePool) -> Result<Vec<ChatLink>, String> {
    let rows = sqlx::query(
        "SELECT c.*,
           COALESCE(
             (SELECT st.full_name FROM students st WHERE c.person_type='student' AND st.id=c.person_id),
             (SELECT tc.full_name FROM teachers tc WHERE c.person_type='teacher' AND tc.id=c.person_id),
             (SELECT u.full_name FROM users u WHERE c.person_type='manager' AND u.id=c.person_id),
             ''
           ) AS person_name
         FROM telegram_chats c ORDER BY c.person_type, c.person_id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows
        .iter()
        .map(|r| ChatLink {
            id: r.get("id"), chat_id: r.get("chat_id"), person_type: r.get("person_type"),
            person_id: r.get("person_id"), person_name: r.get("person_name"),
            phone: r.get("phone"), linked_at: r.get("linked_at"),
            active: r.get::<i64, _>("active") == 1,
        })
        .collect())
}

/// Link a chat to a person by phone (last 9 digits). Returns the person's name.
pub async fn link_by_phone(
    pool: &SqlitePool,
    chat: &str,
    phone: &str,
) -> Result<Option<(String, i64, String)>, String> {
    let norm = normalize_phone(phone);
    // student first
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, full_name, phone FROM students WHERE status='active'",
    )
    .fetch_all(pool).await.map_err(|e| e.to_string())?;
    for (id, name, ph) in &rows {
        if normalize_phone(ph) == norm {
            return link_person(pool, chat, "student", *id, &norm, name).await.map(|_| Some(("student".into(), *id, name.clone())));
        }
    }
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, full_name, phone FROM teachers WHERE status='active'",
    )
    .fetch_all(pool).await.map_err(|e| e.to_string())?;
    for (id, name, ph) in &rows {
        if normalize_phone(ph) == norm {
            return link_person(pool, chat, "teacher", *id, &norm, name).await.map(|_| Some(("teacher".into(), *id, name.clone())));
        }
    }
    Ok(None)
}

async fn link_person(
    pool: &SqlitePool,
    chat: &str,
    ptype: &str,
    pid: i64,
    phone: &str,
    _name: &str,
) -> Result<(), String> {
    // clear any prior chat link for this person type+id and this chat
    sqlx::query("DELETE FROM telegram_chats WHERE chat_id=? OR (person_type=? AND person_id=?)")
        .bind(chat).bind(ptype).bind(pid)
        .execute(pool).await.map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO telegram_chats (chat_id,person_type,person_id,phone) VALUES (?,?,?,?)",
    )
    .bind(chat).bind(ptype).bind(pid).bind(phone)
    .execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn unlink(pool: &SqlitePool, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM telegram_chats WHERE id=?")
        .bind(id).execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn set_active(pool: &SqlitePool, id: i64, active: bool) -> Result<(), String> {
    sqlx::query("UPDATE telegram_chats SET active=? WHERE id=?")
        .bind(if active { 1 } else { 0 }).bind(id)
        .execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

// ------------------------------------------------- self-service linking (app ↔ bot)

/// Name of a person row regardless of its type.
pub async fn person_name(pool: &SqlitePool, ptype: &str, pid: i64) -> String {
    let sql = match ptype {
        "student" => "SELECT full_name FROM students WHERE id=?",
        "teacher" => "SELECT full_name FROM teachers WHERE id=?",
        _ => "SELECT full_name FROM users WHERE id=?",
    };
    sqlx::query_scalar::<_, String>(sql)
        .bind(pid)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// One-time code typed into the bot as `/link CODE`. Short-lived, single use.
pub async fn create_link_code(
    pool: &SqlitePool,
    person_type: &str,
    person_id: i64,
    ttl_minutes: i64,
) -> Result<String, String> {
    use rand::RngCore;
    // invalidate any previous code for this person
    sqlx::query("DELETE FROM telegram_link_codes WHERE person_type=? AND person_id=?")
        .bind(person_type)
        .bind(person_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    let mut raw = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut raw);
    const A: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut code = String::from("LNK-");
    for (i, b) in raw.iter().enumerate() {
        if i == 4 {
            code.push('-');
        }
        code.push(A[(*b as usize) % A.len()] as char);
    }
    let expires = chrono::Local::now() + chrono::Duration::minutes(ttl_minutes.max(2));
    sqlx::query(
        "INSERT INTO telegram_link_codes (code,person_type,person_id,expires_at) VALUES (?,?,?,?)",
    )
    .bind(&code)
    .bind(person_type)
    .bind(person_id)
    .bind(expires.to_rfc3339())
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(code)
}

/// Consume a linking code: attach `chat` to the code's person.
pub async fn consume_link_code(
    pool: &SqlitePool,
    chat: &str,
    raw_code: &str,
) -> Result<Option<(String, i64, String)>, String> {
    let code = raw_code.trim().to_uppercase().replace(' ', "");
    if code.is_empty() {
        return Ok(None);
    }
    let row = sqlx::query(
        "SELECT person_type, person_id FROM telegram_link_codes
         WHERE code=? AND used=0 AND expires_at > ?",
    )
    .bind(&code)
    .bind(chrono::Local::now().to_rfc3339())
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    let Some(r) = row else { return Ok(None) };
    let ptype: String = r.get("person_type");
    let pid: i64 = r.get("person_id");
    let name = person_name(pool, &ptype, pid).await;
    link_person(pool, chat, &ptype, pid, "", &name).await?;
    sqlx::query("UPDATE telegram_link_codes SET used=1 WHERE code=?")
        .bind(&code)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Some((ptype, pid, name)))
}

/// The chat link (any state) of a person — used by the «حسابي» screen.
pub async fn link_for_person(
    pool: &SqlitePool,
    ptype: &str,
    pid: i64,
) -> Result<Option<ChatLink>, String> {
    let name = person_name(pool, ptype, pid).await;
    let row = sqlx::query("SELECT * FROM telegram_chats WHERE person_type=? AND person_id=?")
        .bind(ptype)
        .bind(pid)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.map(|r| ChatLink {
        id: r.get("id"),
        chat_id: r.get("chat_id"),
        person_type: r.get("person_type"),
        person_id: r.get("person_id"),
        person_name: name,
        phone: r.get("phone"),
        linked_at: r.get("linked_at"),
        active: r.get::<i64, _>("active") == 1,
    }))
}

/// Disconnect a person from the bot and drop pending codes.
pub async fn unlink_person(pool: &SqlitePool, ptype: &str, pid: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM telegram_chats WHERE person_type=? AND person_id=?")
        .bind(ptype)
        .bind(pid)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM telegram_link_codes WHERE person_type=? AND person_id=?")
        .bind(ptype)
        .bind(pid)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------- notifications

/// Send a notification to everyone linked (students/teachers/managers) or to
/// a specific person. Respects per-kind enable switches. Logs delivery.
pub async fn notify(
    pool: &SqlitePool,
    kind: &str,
    title: &str,
    body: &str,
    target_type: Option<&str>,
    target_id: Option<i64>,
) -> Result<i64, String> {
    let switch = match kind {
        "payment" | "receipt" => "tg_notify_payment",
        "attendance" | "absence" => "tg_notify_attendance",
        "session" | "reminder" => "tg_notify_session",
        "expense" | "voucher" => "tg_notify_expense",
        _ => "tg_notify_other",
    };
    if !enabled(&setting(pool, switch, "1").await) {
        return Ok(0);
    }
    let rows = sqlx::query(
        "SELECT chat_id, person_type, person_id FROM telegram_chats
         WHERE active=1 AND (?1 IS NULL OR person_type=?1) AND (?2 IS NULL OR person_id=?2)",
    )
    .bind(target_type)
    .bind(target_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let text = format!("*{title}\n\n{body}");
    let mut sent = 0i64;
    for r in rows {
        let chat: String = r.get("chat_id");
        let ptype: String = r.get("person_type");
        let pid: i64 = r.get("person_id");
        match send_message(pool, &chat, &text, kind, None).await {
            Ok(()) => {
                sqlx::query(
                    "INSERT INTO telegram_notifications (chat_id,person_type,person_id,kind,title,body,status)
                     VALUES (?,?,?,?,?,?,'sent')",
                )
                .bind(&chat).bind(&ptype).bind(pid).bind(kind).bind(title).bind(&text)
                .execute(pool).await.ok();
                sent += 1;
            }
            Err(e) => {
                sqlx::query(
                    "INSERT INTO telegram_notifications (chat_id,person_type,person_id,kind,title,body,status,error)
                     VALUES (?,?,?,?,?,?,'failed',?)",
                )
                .bind(&chat).bind(&ptype).bind(pid).bind(kind).bind(title).bind(&text).bind(&e)
                .execute(pool).await.ok();
            }
        }
    }
    Ok(sent)
}

pub async fn broadcast(pool: &SqlitePool, text: &str, target: &str) -> Result<i64, String> {
    let (ptype, pid): (Option<&str>, Option<i64>) = match target {
        "students" => (Some("student"), None),
        "teachers" => (Some("teacher"), None),
        "managers" => (Some("manager"), None),
        _ => (None, None),
    };
    notify(pool, "broadcast", "إشعار من المركز", text, ptype, pid).await
}

// ---------------------------------------------------------------- query replies

fn money(minor: i64) -> String {
    format!("{:.2}", minor as f64 / 100.0)
}

/// Build a reply for a text command from a linked person.
pub async fn handle_command(
    pool: &SqlitePool,
    link: &ChatLink,
    raw: &str,
) -> Result<String, String> {
    let cmd = raw.trim().trim_start_matches('/').trim().to_lowercase();
    let cmd = match cmd.as_str() {
        "start" | "ابدأ" | "بدء" => "help".to_string(),
        "help" | "مساعدة" | "أوامر" => "help".to_string(),
        _ => cmd,
    };

    if !enabled(&setting(pool, "tg_queries_enabled", "1").await)
        && cmd != "help" && cmd != "recover" && cmd != "link"
    {
        return Ok("الاستعلامات معطّلة حالياً من الإدارة.".into());
    }

    match cmd.as_str() {
        "help" => Ok(help_text(link)),
        "today" | "اليوم" | "حالتي" => today_status(pool, link).await,
        "schedule" | "جدولي" | "الجدول" => my_schedule(pool, link).await,
        "fees" | "رسومي" | "الرسوم" | "حسابي" => my_fees(pool, link).await,
        "attendance" | "حضوري" | "الحضور" => my_attendance(pool, link).await,
        "courses" | "دوراتي" => my_courses(pool, link).await,
        "recover" | "استرداد" => Ok("لاسترداد كلمة المرور، افتح التطبيق واضغط «نسيت كلمة المرور؟» وأدخل أحد رموز الاسترداد التي أنشأتها من الإعدادات ← حسابي.\nإذا لم يكن لديك رموز، تواصل مع الإدارة.".into()),
        "link" | "ربط" => Ok(format!("حسابك مرتبط بالفعل كـ {} ({})، هاتف {}.",
            link.person_name, link.person_type, link.phone)),
        _ => Ok("أمر غير معروف. أرسل /help لرؤية الأوامر المتاحة.".into()),
    }
}

fn help_text(link: &ChatLink) -> String {
    let mut s = format!("مرحباً {} 👋\n\nالأوامر المتاحة لك:\n", link.person_name);
    s.push_str("/today — حالة اليوم والدورات النشطة\n");
    s.push_str("/schedule — جدولك الأسبوعي\n");
    s.push_str("/fees — رسومك وما عليك\n");
    s.push_str("/attendance — ملخص حضورك\n");
    s.push_str("/courses — دوراتك المسجلة\n");
    s.push_str("/recover — مساعدة في استرداد كلمة المرور\n");
    s.push_str("/link — معلومات الربط\n");
    s.push_str("/help — هذه القائمة\n");
    s
}

async fn my_schedule(pool: &SqlitePool, link: &ChatLink) -> Result<String, String> {
    let rows = match link.person_type.as_str() {
        "teacher" => sqlx::query(
            "SELECT cs.weekday, cs.start_time, cs.end_time, c.name AS course, COALESCE(r.name,'') AS room
             FROM course_schedules cs JOIN courses c ON c.id=cs.course_id
             LEFT JOIN rooms r ON r.id=cs.room_id
             WHERE cs.teacher_id=? OR c.teacher_id=? ORDER BY cs.weekday, cs.start_time",
        ).bind(link.person_id).bind(link.person_id).fetch_all(pool).await,
        "student" => sqlx::query(
            "SELECT cs.weekday, cs.start_time, cs.end_time, c.name AS course, COALESCE(r.name,'') AS room
             FROM enrollments e JOIN courses c ON c.id=e.course_id
             JOIN course_schedules cs ON cs.course_id=c.id
             LEFT JOIN rooms r ON r.id=cs.room_id
             WHERE e.student_id=? AND e.status IN ('active','frozen')
             ORDER BY cs.weekday, cs.start_time",
        ).bind(link.person_id).fetch_all(pool).await,
        _ => sqlx::query("SELECT 1 AS weekday, '' AS start_time, '' AS end_time, '' AS course, '' AS room WHERE 0").fetch_all(pool).await,
    }.map_err(|e| e.to_string())?;
    if rows.is_empty() {
        return Ok("لا يوجد جدول أسبوعي مرتبط بحسابك حالياً.".into());
    }
    let days = ["", "الاثنين", "الثلاثاء", "الأربعاء", "الخميس", "الجمعة", "السبت", "الأحد"];
    let mut out = String::from("📅 جدولك الأسبوعي:\n\n");
    for r in rows {
        let wd: i64 = r.get("weekday");
        out.push_str(&format!("• {} {}–{} — {} ({})\n",
            days.get(wd as usize).copied().unwrap_or(""),
            r.get::<String, _>("start_time"), r.get::<String, _>("end_time"),
            r.get::<String, _>("course"), r.get::<String, _>("room")));
    }
    Ok(out)
}

async fn my_courses(pool: &SqlitePool, link: &ChatLink) -> Result<String, String> {
    let rows: Vec<(String, String, i64)> = match link.person_type.as_str() {
        "student" => sqlx::query_as(
            "SELECT c.name, e.status, e.net_fee_minor FROM enrollments e
             JOIN courses c ON c.id=e.course_id WHERE e.student_id=? ORDER BY e.id DESC",
        ).bind(link.person_id).fetch_all(pool).await.map_err(|e| e.to_string())?,
        "teacher" => sqlx::query_as(
            "SELECT c.name, c.status, c.fee_minor FROM courses c WHERE c.teacher_id=? ORDER BY c.id DESC",
        ).bind(link.person_id).fetch_all(pool).await.map_err(|e| e.to_string())?,
        _ => vec![],
    };
    if rows.is_empty() { return Ok("لا توجد دورات مرتبطة بحسابك.".into()); }
    let mut out = String::from("📚 دوراتك:\n\n");
    for (name, status, fee) in rows {
        out.push_str(&format!("• {} — {} — {}\n", name, status, money(fee)));
    }
    Ok(out)
}

async fn my_fees(pool: &SqlitePool, link: &ChatLink) -> Result<String, String> {
    if link.person_type == "teacher" {
        let rows: Vec<(i64, i64, String)> = sqlx::query_as(
            "SELECT amount_minor, 0, status FROM payment_vouchers WHERE teacher_id=? ORDER BY id DESC LIMIT 20",
        ).bind(link.person_id).fetch_all(pool).await.map_err(|e| e.to_string())?;
        if rows.is_empty() { return Ok("لا توجد سندات صرف مرتبطة بك.".into()); }
        let mut out = String::from("💰 سندات الصرف الخاصة بك:\n\n");
        for (amt, _, st) in rows { out.push_str(&format!("• {} — {}\n", money(amt), st)); }
        return Ok(out);
    }
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT c.name, e.net_fee_minor,
           COALESCE((SELECT SUM(pa.amount_minor) FROM payment_applications pa WHERE pa.enrollment_id=e.id),0)
         FROM enrollments e JOIN courses c ON c.id=e.course_id
         WHERE e.student_id=? AND e.status IN ('active','frozen')",
    ).bind(link.person_id).fetch_all(pool).await.map_err(|e| e.to_string())?;
    if rows.is_empty() { return Ok("لا توجد رسوم مرتبطة بحسابك.".into()); }
    let mut out = String::from("💰 حسابك:\n\n");
    let (mut total, mut paid) = (0i64, 0i64);
    for (name, fee, p) in rows {
        total += fee; paid += p;
        out.push_str(&format!("• {} — الرسوم {} — المدفوع {} — المتبقي {}\n",
            name, money(fee), money(p), money(fee - p)));
    }
    out.push_str(&format!("\nالإجمالي: {} — المتبقي: {}", money(total), money(total - paid)));
    Ok(out)
}

async fn my_attendance(pool: &SqlitePool, link: &ChatLink) -> Result<String, String> {
    if link.person_type != "student" {
        return Ok("هذا الأمر متاح للطلاب فقط.".into());
    }
    let row: (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN status IN ('present','late') THEN 1 ELSE 0 END),0)
         FROM student_attendance WHERE student_id=?",
    ).bind(link.person_id).fetch_one(pool).await.map_err(|e| e.to_string())?;
    Ok(format!("📋 حضورك: {} حصة مسجّلة، {} حاضر/متأخر، نسبة الالتزام {:.0}٪.",
        row.0, row.1, if row.0 > 0 { row.1 as f64 / row.0 as f64 * 100.0 } else { 0.0 }))
}

async fn today_status(pool: &SqlitePool, link: &ChatLink) -> Result<String, String> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let sessions: Vec<(String, String, String, String)> = match link.person_type.as_str() {
        "teacher" => sqlx::query_as(
            "SELECT c.name, s.start_time, s.end_time, s.status FROM sessions s
             JOIN courses c ON c.id=s.course_id WHERE s.session_date=? AND s.teacher_id=? ORDER BY s.start_time",
        ).bind(&today).bind(link.person_id).fetch_all(pool).await,
        "student" => sqlx::query_as(
            "SELECT c.name, s.start_time, s.end_time, s.status FROM sessions s
             JOIN courses c ON c.id=s.course_id
             JOIN enrollments e ON e.course_id=c.id
             WHERE s.session_date=? AND e.student_id=? ORDER BY s.start_time",
        ).bind(&today).bind(link.person_id).fetch_all(pool).await,
        _ => sqlx::query_as("SELECT '' AS a, '' AS b, '' AS c, '' AS d WHERE 0").fetch_all(pool).await,
    }.map_err(|e| e.to_string())?;
    let mut out = format!("📊 حالة اليوم ({today}):\n\n");
    if sessions.is_empty() {
        out.push_str("لا توجد حصص مجدولة لك اليوم.");
    } else {
        for (name, st, et, status) in sessions {
            out.push_str(&format!("• {} {}–{} — {}\n", name, st, et, status));
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- bot polling

fn reply_keyboard(rows: Vec<Vec<&str>>) -> Value {
    let keyboard: Vec<Vec<Value>> = rows
        .iter()
        .map(|r| r.iter().map(|label| json!({ "text": label })).collect())
        .collect();
    json!({
        "keyboard": keyboard,
        "resize_keyboard": true,
        "one_time_keyboard": false,
    })
}

fn contact_keyboard() -> Value {
    json!({
        "keyboard": [[{ "text": "📱 مشاركة رقم الهاتف", "request_contact": true }]],
        "resize_keyboard": true,
        "one_time_keyboard": true,
    })
}

fn remove_keyboard() -> Value {
    json!({ "remove_keyboard": true })
}

/// Spawn the long-polling bot on Tauri's shared async runtime. Using the same
/// runtime as the pool is essential — a separate runtime would strand the
/// pool's connection-maintenance tasks and cause acquire timeouts.
pub fn spawn_bot(pool: SqlitePool) {
    tauri::async_runtime::spawn(async move {
        loop {
            let enabled_flag = setting(&pool, "tg_enabled", "0").await;
            let token = setting(&pool, "tg_token", "").await;
            if !enabled(&enabled_flag) || token.trim().is_empty() {
                tokio::time::sleep(Duration::from_secs(4)).await;
                continue;
            }
            let offset: i64 = setting(&pool, "tg_offset", "0").await.parse().unwrap_or(0);
            let body = json!({
                "offset": offset,
                "timeout": 20,
                "allowed_updates": ["message"],
            });
            let t = token.clone();
            let result = tauri::async_runtime::spawn_blocking(move || api(&t, "getUpdates", body, 35)).await;
            match result {
                Ok(Ok(updates)) => {
                    if let Some(arr) = updates.as_array() {
                        for u in arr {
                            let id = u.get("update_id").and_then(|x| x.as_i64()).unwrap_or(offset);
                            handle_update(&pool, &token, u).await;
                            set_setting(&pool, "tg_offset", &(id + 1).to_string()).await.ok();
                        }
                        if !arr.is_empty() {
                            tokio::time::sleep(Duration::from_millis(300)).await;
                        }
                    }
                }
                Ok(Err(e)) => {
                    if e.contains("Conflict") {
                        let t = token.clone();
                        tauri::async_runtime::spawn_blocking(move || api(&t, "deleteWebhook", json!({}), 10)).await.ok();
                    }
                    tokio::time::sleep(Duration::from_secs(6)).await;
                }
                Err(_) => tokio::time::sleep(Duration::from_secs(6)).await,
            }
        }
    });
}

async fn handle_update(pool: &SqlitePool, token: &str, u: &Value) {
    // re-check enabled (race with admin disabling)
    if !enabled(&setting(pool, "tg_enabled", "0").await) {
        return;
    }
    let Some(msg) = u.get("message") else { return; };
    let chat = match msg.get("chat").and_then(|c| c.get("id")) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => return,
    };
    let text = msg.get("text").and_then(|x| x.as_str()).unwrap_or("").trim().to_string();

    // contact sharing → link by phone
    if let Some(contact) = msg.get("contact") {
        let phone = contact.get("phone_number").and_then(|x| x.as_str()).unwrap_or("");
        let user_id = msg.get("from").and_then(|f| f.get("id")).and_then(|x| x.as_i64());
        let contact_uid = contact.get("user_id").and_then(|x| x.as_i64());
        // reject contacts that don't belong to the sender
        if let (Some(uid), Some(cuid)) = (user_id, contact_uid) {
            if uid != cuid {
                send_raw(token, &chat, "يجب مشاركة جهة اتصالك أنت فقط.", None).ok();
                return;
            }
        }
        match link_by_phone(pool, &chat, phone).await {
            Ok(Some((ptype, _pid, name))) => {
                log_message(pool, &chat, &ptype, None, "in", &format!("📱 مشاركة رقم: {phone}"), "contact").await;
                let reply = format!("✅ تم ربط حسابك كـ {name} ({ptype}).\n\nأرسل /help لرؤية الأوامر.");
                send_raw(token, &chat, &reply, Some(remove_keyboard())).ok();
                log_message(pool, &chat, &ptype, None, "out", &reply, "contact").await;
            }
            _ => {
                // No account with this phone → offer a service request.
                let state = serde_json::json!({
                    "step": "phone", "phone": phone,
                    "name": msg.get("contact").and_then(|c| c.get("first_name")).and_then(|x| x.as_str()).unwrap_or("").to_string()
                });
                set_setting(pool, &format!("flow:{}", chat), &state.to_string()).await.ok();
                let reply = format!(
                    "مرحباً! رقم {} غير مسجّل لدينا، لكن يمكنك طلب خدمة:\n\nاكتب اسمك الآن (أو أرسل /cancel للإلغاء).",
                    phone
                );
                send_raw(token, &chat, &reply, None).ok();
                log_message(pool, &chat, "", None, "out", &reply, "service_flow").await;
            }
        }
        return;
    }

    if text.is_empty() {
        return;
    }

    let lower = text.to_lowercase();

    // ---- self-service linking from the app: /link CODE or /start CODE ----
    if lower.starts_with("/link") || lower.starts_with("/start") {
        if let Some(code) = text.split_whitespace().nth(1) {
            match consume_link_code(pool, &chat, code).await {
                Ok(Some((ptype, pid, name))) => {
                    // managers double as bot admins
                    if ptype == "manager" {
                        let mut mg = setting(pool, "tg_manager_chats", "").await;
                        if !mg.split(',').any(|c| c.trim() == chat) {
                            if !mg.trim().is_empty() { mg.push(','); }
                            mg.push_str(&chat);
                            set_setting(pool, "tg_manager_chats", &mg).await.ok();
                        }
                    }
                    let reply = format!(
                        "✅ تم ربط حسابك كـ {name}.\n\nأرسل /help لرؤية الأوامر المتاحة."
                    );
                    send_raw(token, &chat, &reply, Some(remove_keyboard())).ok();
                    log_message(pool, &chat, &ptype, Some(pid), "in", &format!("/link {code}"), "link").await;
                    log_message(pool, &chat, &ptype, Some(pid), "out", &reply, "link").await;
                }
                Ok(None) => {
                    let reply = "❌ الرمز غير صالح أو منتهي الصلاحية.\nأنشئ رمزاً جديداً من البرنامج (حسابي ← ربط التلجرام).";
                    send_raw(token, &chat, reply, None).ok();
                }
                Err(e) => {
                    send_raw(token, &chat, &format!("تعذّر الربط: {e}"), None).ok();
                }
            }
            return;
        }
    }

    if lower == "/unlink" || lower == "إلغاء الربط" {
        match link_by_chat(pool, &chat).await.ok().flatten() {
            Some(l) => {
                unlink_person(pool, &l.person_type, l.person_id).await.ok();
                send_raw(token, &chat, "تم إلغاء ربط حسابك بالبوت. يمكنك الربط مجدداً من البرنامج.", None).ok();
            }
            None => {
                send_raw(token, &chat, "حسابك غير مرتبط أصلاً.", None).ok();
            }
        }
        return;
    }

    // /start with no link → ask for contact
    if lower.starts_with("/start") || lower == "ابدأ" || lower == "بدء" {
        let link = link_by_chat(pool, &chat).await.ok().flatten();
        if link.is_none() {
            let welcome = "مرحباً! 👋\nلاستخدام البوت، شارك رقم هاتفك المسجّل في المركز بالضغط على الزر أدناه.";
            send_raw(token, &chat, welcome, Some(contact_keyboard())).ok();
            return;
        }
    }

    // manager-only commands (by chat id listed in tg_manager_chats setting)
    let managers = setting(pool, "tg_manager_chats", "").await;
    let is_manager = managers.split(',').any(|c| c.trim() == chat);

    if lower == "/broadcast" || lower == "إذاعة" {
        if !is_manager {
            send_raw(token, &chat, "هذا الأمر متاح للإدارة فقط.", None).ok();
            return;
        }
        let rep = "أرسل نص الإشعار الآن (سيُرسل لكل الحسابات المرتبطة).";
        send_raw(token, &chat, rep, None).ok();
        set_setting(pool, &format!("flow:{chat}"), "broadcast").await.ok();
        return;
    }

    // pending broadcast flow
    let flow_key = format!("flow:{chat}");
    let flow = setting(pool, &flow_key, "").await;
    if flow == "broadcast" && !is_manager {
        set_setting(pool, &flow_key, "").await.ok();
    }
    if flow == "broadcast" && is_manager {
        set_setting(pool, &flow_key, "").await.ok();
        match broadcast(pool, &text, "all").await {
            Ok(n) => { send_raw(token, &chat, &format!("تم إرسال الإشعار إلى {n} حساب."), None).ok(); }
            Err(e) => { send_raw(token, &chat, &format!("تعذّر الإرسال: {e}"), None).ok(); }
        }
        return;
    }

    // ---- service-request flow (unregistered users) ----
    if text == "/cancel" || text == "إلغاء" {
        set_setting(pool, &flow_key, "").await.ok();
        send_raw(token, &chat, "تم إلغاء الطلب.", None).ok();
        return;
    }
    if flow.starts_with('{') {
        let mut st: serde_json::Value = serde_json::from_str(&flow).unwrap_or(serde_json::Value::Null);
        let step = st.get("step").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let phone = st.get("phone").and_then(|x| x.as_str()).unwrap_or("").to_string();
        match step.as_str() {
            "phone" => {
                st["step"] = serde_json::json!("name");
                st["requester"] = serde_json::json!(text.clone());
                set_setting(pool, &flow_key, &st.to_string()).await.ok();
                send_raw(token, &chat, "ما الخدمة المطلوبة؟ (مثلاً: ملخص مادة، طباعة ورقة، كتاب)", None).ok();
                return;
            }
            "name" => {
                st["step"] = serde_json::json!("service");
                st["service"] = serde_json::json!(text.clone());
                set_setting(pool, &flow_key, &st.to_string()).await.ok();
                send_raw(token, &chat, "أضف أي تفاصيل إضافية (أو أرسل /done للإرسال):", None).ok();
                return;
            }
            "service" => {
                if text == "/done" {
                    let requester = st.get("requester").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let service = st.get("service").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let details = st.get("details").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    match create_service_request(pool, &chat, &phone, &requester, &service, &details).await {
                        Ok(_) => {
                            send_raw(token, &chat, "✅ تم إرسال طلبك إلى الإدارة — سيتم الرد عليك هنا قريباً.", None).ok();
                            // alert managers via Telegram
                            let mg = setting(pool, "tg_manager_chats", "").await;
                            for m in mg.split(',').filter(|x| !x.trim().is_empty()) {
                                let note = format!("📢 طلب خدمة جديد\nالاسم: {}\nالهاتف: {}\nالخدمة: {}\nالتفاصيل: {}", requester, phone, service, details);
                                send_message(pool, m.trim(), &note, "service_request", None).await.ok();
                            }
                        }
                        Err(e) => { send_raw(token, &chat, &format!("تعذّر إرسال الطلب: {}", e), None).ok(); }
                    }
                    set_setting(pool, &flow_key, "").await.ok();
                } else {
                    st["step"] = serde_json::json!("service");
                    st["details"] = serde_json::json!(text.clone());
                    set_setting(pool, &flow_key, &st.to_string()).await.ok();
                    send_raw(token, &chat, "تم تسجيل التفاصيل. أرسل /done للإرسال.", None).ok();
                }
                return;
            }
            _ => {}
        }
    }

    // resolve link
    let link = match link_by_chat(pool, &chat).await.ok().flatten() {
        Some(l) => l,
        None => {
            let reply = "حسابك غير مرتبط. أرسل /start ثم شارك رقم هاتفك.";
            send_raw(token, &chat, reply, Some(contact_keyboard())).ok();
            return;
        }
    };

    log_message(pool, &chat, &link.person_type, Some(link.person_id), "in", &text, "command").await;

    let reply = handle_command(pool, &link, &text).await.unwrap_or_else(|e| format!("تعذّر تنفيذ الأمر: {e}"));
    send_raw(token, &chat, &reply, Some(reply_keyboard(vec![
        vec!["/today", "/schedule", "/fees"],
        vec!["/attendance", "/courses", "/help"],
    ]))).ok();
    log_message(pool, &chat, &link.person_type, Some(link.person_id), "out", &reply, "command").await;
}

/// Verify the bot token via getMe and configure commands/description.
pub fn test_bot(token: &str) -> Result<String, String> {
    let me = api(token, "getMe", json!({}), 15)?;
    let name = me.get("first_name").and_then(|x| x.as_str()).unwrap_or("?");
    let username = me.get("username").and_then(|x| x.as_str()).unwrap_or("?");
    // best effort: set commands menu (Arabic)
    let commands = json!({ "commands": [
        { "command": "start", "description": "بدءاستخدام البوت" },
        { "command": "help", "description": "الأوامر المتاحة" },
        { "command": "today", "description": "حالة اليوم" },
        { "command": "schedule", "description": "جدولي الأسبوعي" },
        { "command": "fees", "description": "رسومي وحسابي" },
        { "command": "attendance", "description": "ملخص حضوري" },
        { "command": "courses", "description": "دوراتي" },
        { "command": "recover", "description": "استرداد كلمة المرور" }
    ]});
    api(token, "setMyCommands", commands, 10).ok();
    api(token, "deleteWebhook", json!({}), 10).ok();
    Ok(format!("البوت يعمل: @{username} ({name})"))
}

// ---------------------------------------------------------------- reminders

/// Check sessions starting within `minutes_ahead` and notify their teachers and
/// enrolled students. Uses a dedup setting key per session so each reminder is
/// sent once.
pub async fn send_session_reminders(pool: &SqlitePool, minutes_ahead: i64) -> Result<i64, String> {
    // In-app + desktop reminders work with or without Telegram; only the
    // per-event switch gates them. Telegram delivery is opt-in on top.
    if !enabled(&setting(pool, "tg_notify_session", "1").await) {
        return Ok(0);
    }
    let tg_token = setting(pool, "tg_token", "").await;
    let tg_on = enabled(&setting(pool, "tg_enabled", "0").await) && !tg_token.trim().is_empty();
    let now = chrono::Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let now_min = (now.hour() * 60 + now.minute()) as i64;
    let to_min = now_min + minutes_ahead;

    let rows = sqlx::query(
        "SELECT s.id, s.course_id, s.start_time, s.end_time, c.name AS course,
                COALESCE(t.full_name,'') AS teacher_name, s.teacher_id,
                COALESCE(r.name,'') AS room_name
         FROM sessions s JOIN courses c ON c.id=s.course_id
         LEFT JOIN teachers t ON t.id=s.teacher_id
         LEFT JOIN rooms r ON r.id=s.room_id
         WHERE s.session_date=? AND s.status='scheduled'",
    )
    .bind(&today)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut sent = 0i64;
    for r in rows {
        let st: String = r.get("start_time");
        let parts: Vec<&str> = st.split(':').collect();
        let h: i64 = parts.get(0).and_then(|x| x.parse().ok()).unwrap_or(0);
        let m: i64 = parts.get(1).and_then(|x| x.parse().ok()).unwrap_or(0);
        let sess_min = h * 60 + m;
        if sess_min < now_min || sess_min > to_min { continue; }
        let sid: i64 = r.get("id");
        let dedup_key = format!("tg_reminded_{sid}");
        if !setting(pool, &dedup_key, "").await.is_empty() { continue; }
        let text = format!("⏰ تذكير بحصة قريبة\n\n{} — {}–{}\nالقاعة: {} — المدرس: {}",
            r.get::<String,_>("course"), st, r.get::<String,_>("end_time"),
            r.get::<String,_>("room_name"), r.get::<String,_>("teacher_name"));
        // in-app inbox + desktop toast (always)
        ec_core::notifications::push(
            pool, "reminder", "تذكير بحصة قريبة", &text,
            "session", Some(sid), "", None,
        ).await.ok();
        crate::desktop_notify_global("تذكير بحصة قريبة", &text);

        // Telegram delivery (only when the bot is enabled)
        if tg_on {
            // notify teacher
            if let Some(tid) = r.get::<Option<i64>, _>("teacher_id") {
                notify(pool, "reminder", "تذكير بحصة", &text, Some("teacher"), Some(tid)).await.ok();
            }
            // notify enrolled students
            let students: Vec<i64> = sqlx::query_scalar(
                "SELECT student_id FROM enrollments WHERE course_id=? AND status IN ('active','frozen')",
            ).bind(r.get::<i64,_>("course_id")).fetch_all(pool).await.unwrap_or_default();
            for stid in students {
                notify(pool, "reminder", "تذكير بحصة", &text, Some("student"), Some(stid)).await.ok();
            }
        }
        set_setting(pool, &dedup_key, "1").await.ok();
        sent += 1;
    }
    Ok(sent)
}

/// Background loop: runs session reminders every 2 minutes.
pub fn spawn_reminders(pool: SqlitePool) {
    tauri::async_runtime::spawn(async move {
        loop {
            send_session_reminders(&pool, 30).await.ok();
            tokio::time::sleep(Duration::from_secs(120)).await;
        }
    });
}

// ---------------------------------------------------------------- service requests

/// A service requested by an unregistered phone via the bot.
/// Stored as pending; managers are alerted in-app.
pub async fn create_service_request(
    pool: &SqlitePool,
    chat: &str,
    phone: &str,
    requester: &str,
    service: &str,
    details: &str,
) -> Result<i64, String> {
    let res = sqlx::query(
        "INSERT INTO service_requests (chat_id,phone,requester,service,details) VALUES (?,?,?,?,?)",
    )
    .bind(chat).bind(phone).bind(requester).bind(service).bind(details)
    .execute(pool).await.map_err(|e| e.to_string())?;
    let id = res.last_insert_rowid();
    // in-app notification for managers
    let note = format!("{} — {} ({})", service, requester, phone);
    ec_core::notifications::push(pool, "service_request", "طلب خدمة جديد",
        &note, "service_request", Some(id), "", None).await.ok();
    crate::desktop_notify_global("طلب خدمة جديد", &note);
    Ok(id)
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct ServiceRequest {
    pub id: i64, pub chat_id: String, pub phone: String, pub requester: String,
    pub service: String, pub details: String, pub status: String,
    pub response: String, pub created_at: String,
}

pub async fn list_service_requests(pool: &SqlitePool, status: Option<&str>) -> Result<Vec<ServiceRequest>, String> {
    let rows = sqlx::query(
        "SELECT * FROM service_requests WHERE (?1 IS NULL OR status=?1) ORDER BY id DESC LIMIT 200",
    ).bind(status).fetch_all(pool).await.map_err(|e| e.to_string())?;
    Ok(rows.iter().map(|r| ServiceRequest {
        id: r.get("id"), chat_id: r.get("chat_id"), phone: r.get("phone"),
        requester: r.get("requester"), service: r.get("service"), details: r.get("details"),
        status: r.get("status"), response: r.get("response"), created_at: r.get("created_at"),
    }).collect())
}

pub async fn respond_service_request(
    pool: &SqlitePool, id: i64, approve: bool, response: &str, actor_id: i64,
) -> Result<(), String> {
    let status = if approve { "done" } else { "rejected" };
    sqlx::query("UPDATE service_requests SET status=?, response=?, handled_by=?, handled_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?")
        .bind(status).bind(response).bind(actor_id).bind(id).execute(pool).await.map_err(|e| e.to_string())?;
    // notify the requester via Telegram if they have a chat
    let chat: Option<String> = sqlx::query_scalar("SELECT chat_id FROM service_requests WHERE id=?")
        .bind(id).fetch_optional(pool).await.map_err(|e| e.to_string())?;
    if let Some(chat) = chat {
        let text = format!("{} طلبك: {}\n{}", if approve { "✅ تمت الموافقة على" } else { "❌ تم رفض" }, response, "");
        send_message(pool, &chat, &text, "service_request", None).await.ok();
    }
    Ok(())
}
