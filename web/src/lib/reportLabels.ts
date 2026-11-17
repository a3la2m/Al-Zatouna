/// Arabic headers for every column the report registry can emit.
/// A column without an entry falls back to the raw key, so adding a report
/// never breaks the screen.
export const COLUMN_LABELS: Record<string, string> = {
  // people
  id: 'المعرّف',
  full_name: 'الاسم الكامل',
  phone: 'الهاتف',
  status: 'الحالة',
  name: 'الاسم',
  capacity: 'السعة',
  course_id: 'الدورة',
  course_name: 'اسم الدورة',
  student_id: 'الطالب',
  teacher_id: 'المدرس',
  enrolled: 'المسجّلون',
  students: 'الطلاب',
  courses: 'الدورات',
  sessions: 'الحصص',
  // education
  enrolled_date: 'تاريخ التسجيل',
  base_fee_minor: 'الرسوم الأساسية',
  discount_minor: 'الخصم',
  waiver_minor: 'الإعفاء',
  credit_applied_minor: 'الرصيد المستخدم',
  net_fee_minor: 'صافي الرسوم',
  weekday: 'اليوم',
  start_time: 'من',
  end_time: 'إلى',
  session_date: 'تاريخ الحصة',
  reason: 'السبب',
  late_minutes: 'دقائق التأخير',
  rate: 'نسبة الالتزام (%)',
  present: 'حاضر',
  absent: 'غائب',
  late: 'متأخر',
  // finance
  period: 'الفترة',
  amount_minor: 'المبلغ',
  collected_minor: 'المحصّل',
  receivable_minor: 'الذمة المدينة',
  due_minor: 'المستحق',
  paid_minor: 'المحصّل',
  revenue_minor: 'الإيرادات',
  expense_minor: 'المصروفات',
  net_minor: 'صافي الدخل',
  in_minor: 'وارد',
  out_minor: 'صادر',
  date: 'التاريخ',
  category: 'البند',
  count: 'العدد',
  method: 'الطريقة',
  fee_minor: 'الرسوم',
  entry_date: 'تاريخ القيد',
  debit_minor: 'مدين',
  credit_minor: 'دائن',
  balance_minor: 'الرصيد',
  account_id: 'الحساب',
  entry_id: 'القيد',
  balanced: 'متوازن',
  balanced_label: 'متوازن',
  // vouchers / closings
  closing_id: 'سند الإغلاق',
  variance_minor: 'الفرق',
  // control
  user_id: 'المستخدم',
  action: 'الإجراء',
  created_at: 'التاريخ',
  entity_type: 'الكيان',
  permission: 'الصلاحية',
  enrollment_id: 'التسجيل',
  due_date: 'تاريخ الاستحقاق',
  decision: 'القرار',
  center_share_minor: 'حصة المركز',
  teacher_share_minor: 'حصة المدرس',
  uncollected_minor: 'المبلغ غير المحصّل',
  beneficiary: 'المستفيد',
  description: 'الوصف',
  room_id: 'القاعة',
  title: 'العنوان',
  balance_minor2: 'الرصيد'
};

/** Column keys whose value is integer minor units → render as money. */
export function isMoneyColumn(key: string): boolean {
  return key.endsWith('_minor') || key === 'amount_minor';
}

/** Column keys that are calendar dates. */
export function isDateColumn(key: string): boolean {
  return key.endsWith('_date') || key.endsWith('_at') || key === 'date';
}
