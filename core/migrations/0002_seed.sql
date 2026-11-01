-- =============================================================
-- migration 0002: seed data — chart of accounts, roles, permissions,
-- default center settings, main branch, default cash register,
-- default print profiles and expense categories.
-- =============================================================

-- ---------- chart of accounts (docs/ERD.md §4) ----------
INSERT INTO accounts (code, name, kind, parent_id, is_postable) VALUES
  ('1000','الصندوق النقدي','asset',NULL,1),
  ('1100','الحساب البنكي','asset',NULL,1),
  ('1200','حسابات المحافظ الإلكترونية','asset',NULL,1),
  ('1300','ذمم الطلاب المدينة','asset',NULL,1),
  ('1400','مستحقات المدرسين','liability',NULL,1),
  ('1500','التزامات استرداد معتمدة','liability',NULL,1),
  ('1600','مستحقات موردون','liability',NULL,1),
  ('2000','إيرادات مأخرة (Deferred Revenue)','liability',NULL,1),
  ('3000','حقوق الملكية / رأس المال','equity',NULL,1),
  ('4000','إيرادات الدورات','revenue',NULL,1),
  ('4100','إيرادات حصة المركز','revenue',NULL,1),
  ('7000','فروقات الصندوق','revenue',NULL,1),
  ('5000','مصروفات','expense',NULL,0),
  ('6000','مصروف الاستردادات','expense',NULL,1),
  ('6100','خصم إداري (مخفف إيراد)','revenue',NULL,1);
-- expense children (postable)
INSERT INTO accounts (code, name, kind, parent_id, is_postable)
SELECT c.code, c.name, 'expense', (SELECT id FROM accounts WHERE code='5000'), 1 FROM (
  SELECT '5100' AS code, 'إيجار' AS name UNION ALL
  SELECT '5200','كهرباء' UNION ALL
  SELECT '5300','مياه' UNION ALL
  SELECT '5400','قرطاسية' UNION ALL
  SELECT '5500','رواتب وأجور' UNION ALL
  SELECT '5600','صيانة' UNION ALL
  SELECT '5700','خدمات' UNION ALL
  SELECT '5800','مصروفات إدارية' UNION ALL
  SELECT '5900','مصروفات أخرى'
) c;

-- user placeholder users are NOT seeded here — first-run wizard creates the admin.
INSERT INTO roles (code, name, is_system) VALUES
  ('system_admin','مدير النظام',1),
  ('center_manager','مدير المركز',1),
  ('accountant','المحاسب',1),
  ('registration_clerk','موظف التسجيل',1),
  ('scheduler','مسؤول الجدولة',1),
  ('teacher','مدرس',1),
  ('read_only','قراءة فقط',1);

-- permissions catalog (docs/SECURITY.md §3)
INSERT INTO permissions (code, name, grp) VALUES
  ('students.view','عرض الطلاب','students'),
  ('students.create','إنشاء الطلاب','students'),
  ('students.edit','تعديل الطلاب','students'),
  ('students.archive','أرشفة الطلاب','students'),
  ('students.attachments.manage','إدارة مرفقات الطلاب','students'),
  ('teachers.view','عرض المدرسين','teachers'),
  ('teachers.manage','إدارة المدرسين','teachers'),
  ('teachers.attachments.manage','إدارة مرفقات المدرسين','teachers'),
  ('subjects.manage','إدارة المواد','courses'),
  ('rooms.manage','إدارة القاعات','courses'),
  ('courses.view','عرض الدورات','courses'),
  ('courses.create','إنشاء الدورات','courses'),
  ('courses.edit','تعديل الدورات','courses'),
  ('courses.approve','اعتماد الدورات','courses'),
  ('courses.amend','تعديل اتفاقية الدورة','courses'),
  ('schedule.manage','إدارة الجدول','scheduling'),
  ('sessions.executed','تسجيل تنفيذ الحصص','scheduling'),
  ('sessions.cancel','إلغاء الحصص','scheduling'),
  ('sessions.makeup','إدارة الحصص التعويضية','scheduling'),
  ('attendance.record','تسجيل حضور الطلاب','scheduling'),
  ('attendance.teacher.record','تسجيل حضور المدرس','scheduling'),
  ('agreements.manage','إدارة الاتفاقيات المالية','finance'),
  ('shortfall.propose','تقديم حالة عجز','finance'),
  ('shortfall.decide','اعتماد قرار العجز','finance'),
  ('refunds.request','تقديم طلب استرداد','finance'),
  ('refunds.approve','اعتماد الاسترداد','finance'),
  ('settlements.prepare','إعداد التسويات','finance'),
  ('settlements.approve','اعتماد التسويات','finance'),
  ('ledger.view','عرض الدفتر المحاسبي','finance'),
  ('vouchers.receipt.create','إنشاء سند قبض','money'),
  ('vouchers.receipt.approve','اعتماد سند قبض','money'),
  ('vouchers.payment.create','إنشاء سند صرف','money'),
  ('vouchers.payment.approve','اعتماد سند صرف','money'),
  ('payments.record','تسجيل الدفعات','money'),
  ('cash.manage','إدارة الصندوق','money'),
  ('cash.close','إغلاق الصندوق','money'),
  ('expenses.create','إنشاء المصروفات','money'),
  ('expenses.approve','اعتماد المصروفات','money'),
  ('reports.view','عرض التقارير','reports'),
  ('reports.export','تصدير التقارير','reports'),
  ('reports.builder','منشئ التقارير','reports'),
  ('print.profiles.manage','إدارة إعدادات الطباعة','print'),
  ('print.execute','الطباعة وإصدار المستندات','print'),
  ('users.manage','إدارة المستخدمين','admin'),
  ('users.password_reset','إعادة تعيين كلمات المرور','admin'),
  ('roles.manage','إدارة الأدوار','admin'),
  ('permissions.assign','تخصيص الصلاحيات','admin'),
  ('audit.view','عرض سجل التدقيق','admin'),
  ('settings.manage','إدارة إعدادات النظام','admin'),
  ('backup.create','إنشاء نسخ احتياطية','admin'),
  ('backup.restore','استعادة نسخ احتياطية','admin'),
  ('datadir.change','تغيير مسار البيانات','admin');

-- role → permission seeds: system_admin gets everything; others grouped.
INSERT INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='system_admin'), id FROM permissions;

INSERT INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='center_manager'), id FROM permissions
 WHERE code IN ('students.view','teachers.view','courses.view','courses.approve','courses.amend',
   'schedule.manage','agreements.manage','shortfall.propose','shortfall.decide',
   'refunds.approve','settlements.prepare','settlements.approve','ledger.view',
   'vouchers.receipt.approve','vouchers.payment.approve','cash.close','expenses.approve',
   'reports.view','reports.export','print.profiles.manage','audit.view','payments.record',
   'sessions.executed','sessions.cancel','attendance.record');

INSERT INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='accountant'), id FROM permissions
 WHERE code IN ('students.view','courses.view','courses.create','payments.record',
   'vouchers.receipt.create','vouchers.receipt.approve','vouchers.payment.create','vouchers.payment.approve',
   'cash.manage','cash.close','expenses.create','expenses.approve','settlements.prepare',
   'shortfall.propose','refunds.request','ledger.view','reports.view','reports.export','print.execute','agreements.manage');

INSERT INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='registration_clerk'), id FROM permissions
 WHERE code IN ('students.view','students.create','students.edit','students.attachments.manage',
   'courses.view','vouchers.receipt.create','payments.record','reports.view','print.execute','attendance.record');

INSERT INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='scheduler'), id FROM permissions
 WHERE code IN ('courses.view','schedule.manage','sessions.executed','sessions.cancel',
   'sessions.makeup','rooms.manage','subjects.manage','attendance.record','reports.view','print.execute');

INSERT INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='teacher'), id FROM permissions
 WHERE code IN ('courses.view','attendance.record','reports.view');

INSERT INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='read_only'), id FROM permissions
 WHERE code IN ('students.view','teachers.view','courses.view','reports.view','ledger.view');

-- ---------- default center settings + main branch + cash ----------
INSERT INTO center_settings (id, name, trade_name, currency_code, currency_decimals, timezone, academic_year, cash_rules_json)
VALUES (1, 'المركز التعليمي', '', 'ILS', 2, 'Asia/Hebron',
        printf('%d/%d', (strftime('%Y','now') + 0), ((strftime('%Y','now') + 0) + 1)),
        '{"require_open_register_for_cash": true}');

INSERT INTO branches (code, name, is_main) VALUES ('MAIN', 'الفرع الرئيسي', 1);

INSERT INTO cash_registers (branch_id, code, name, status, notes)
SELECT id, 'MAIN', 'الصندوق الرئيسي', 'active', 'الصندوق الافتراضي' FROM branches WHERE code='MAIN';

-- sequences for the main branch (year 0 = not year-scoped for ID types)
INSERT INTO voucher_sequences (branch_id, kind, fiscal_year, last_no)
SELECT id, kind, 0, 0 FROM branches
CROSS JOIN (SELECT 'student_no' AS kind UNION SELECT 'teacher_no' UNION SELECT 'course_no');

INSERT INTO expense_categories (code, name) VALUES
  ('rent','إيجار'), ('electricity','كهرباء'), ('water','مياه'),
  ('stationery','قرطاسية'), ('salaries','رواتب'), ('maintenance','صيانة'),
  ('services','خدمات'), ('admin','مصروفات إدارية'), ('other','أخرى');

-- ---------- default print profiles ----------
INSERT INTO print_profiles (code, name, doc_type, is_default) VALUES
  ('voucher.receipt','سند قبض','receipt_voucher',1),
  ('voucher.payment','سند صرف','payment_voucher',1),
  ('enrollment.receipt','إيصال تسجيل','enrollment_receipt',1),
  ('statement.student','كشف حساب طالب','student_statement',1),
  ('statement.teacher','كشف حساب مدرس','teacher_statement',1),
  ('attendance.sheet','كشف حضور وغياب','attendance_sheet',1),
  ('schedule.study','جدول دراسي','study_schedule',1),
  ('certificate.completion','شهادة إتمام','completion_certificate',1),
  ('report.financial','تقرير مالي','financial_report',1),
  ('report.admin','تقرير إداري','admin_report',1),
  ('closing.memo','محضر إغلاق صندوق','register_closing',1),
  ('refund.request','طلب استرداد','refund_request',1),
  ('shortfall.decision','قرار عجز','shortfall_decision',1),
  ('settlement.course','كشف تسوية دورة','course_settlement',1);
