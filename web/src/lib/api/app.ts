/// Typed IPC surface for dashboard, reports, users, settings and cash.
/// Every call funnels through `$lib/api` so browser preview fails loudly
/// instead of pretending to succeed.
import { invoke } from '$lib/api';
import type { Student } from '$lib/api/people';

/* ----------------------------- dashboard ----------------------------- */

export type DashboardKpis = {
  students_total: number;
  students_active: number;
  courses_total: number;
  courses_active: number;
  enrollments_active: number;
  collected_minor: number;
  receivable_minor: number;
  today_sessions: number;
  pending_refunds: number;
  pending_shortfalls: number;
  cash_balance_minor: number;
  teacher_payables_minor: number;
  due_fees_minor: number;
  courses_needing_settlement: number;
  collected_today_minor: number;
  collected_month_minor: number;
  expenses_month_minor: number;
  students_new_month: number;
  receipts_pending: number;
};

export type ChartPoint = { label: string; value: number };
export type CashFlowPoint = {
  date: string;
  in_minor: number;
  out_minor: number;
  balance_minor: number;
};

export type DashboardCharts = {
  collections_by_month: ChartPoint[];
  expenses_by_month: ChartPoint[];
  students_by_status: ChartPoint[];
  enrollments_by_status: ChartPoint[];
  attendance_by_status: ChartPoint[];
  top_courses: ChartPoint[];
  cash_flow: CashFlowPoint[];
};

export const dashboardApi = {
  kpis: (token: string) => invoke<DashboardKpis>('dashboard_kpis', { token }),
  charts: (token: string) => invoke<DashboardCharts>('dashboard_charts', { token })
};

/* ------------------------------ reports ------------------------------ */

export type ReportDefinition = {
  code: string;
  title: string;
  category: string;
  columns: string[];
};

export type ReportResult = {
  code: string;
  title: string;
  category: string;
  columns: string[];
  rows: Record<string, unknown>[];
  row_count: number;
};

export type SavedReport = {
  id: number;
  name: string;
  definition_json: string;
  is_shared: boolean;
  is_favorite: boolean;
  filters_json: string;
};

export const reportsApi = {
  definitions: (token: string) => invoke<ReportDefinition[]>('reports_definitions', { token }),
  run: (token: string, code: string) => invoke<ReportResult>('reports_run', { token, code }),
  export: (token: string, code: string) => invoke<string>('reports_export', { token, code }),
  list: (token: string) => invoke<SavedReport[]>('reports_list', { token }),
  save: (token: string, name: string, definition_json: string) =>
    invoke<number>('reports_save', { token, name, definition_json })
};

/* ------------------------------ users -------------------------------- */

export type UserSummary = {
  id: number;
  username: string;
  full_name: string;
  is_active: boolean;
  must_change_password: boolean;
  last_login_at: string | null;
  created_at: string;
  roles: string[];
};

export const usersApi = {
  list: (token: string) => invoke<UserSummary[]>('auth_list_users', { token }),
  roles: (token: string) => invoke<string[]>('auth_list_roles', { token }),
  create: (
    token: string,
    data: { username: string; full_name: string; password: string; roles: string[] }
  ) =>
    invoke<number>('auth_create_user', {
      token,
      username: data.username,
      full_name: data.full_name,
      password: data.password,
      roles: data.roles
    }),
  resetPassword: (token: string, target_user_id: number, new_password: string, reason: string) =>
    invoke<void>('auth_reset_password', { token, target_user_id, new_password, reason }),
  disable: (token: string, target_user_id: number, reason: string) =>
    invoke<void>('auth_disable_user', { token, target_user_id, reason }),
  enable: (token: string, target_user_id: number) =>
    invoke<void>('auth_enable_user', { token, target_user_id }),
  changePassword: (token: string, current: string, new_password: string) =>
    invoke<void>('auth_change_password', { token, current, new_password })
};

/* ----------------------------- settings ------------------------------ */

export type AppInfo = {
  app: string;
  version: string;
  data_dir: string;
  accounts_seeded: number;
  users: number;
};

export const settingsApi = {
  info: () => invoke<AppInfo>('app_info'),
  changeDataDir: (token: string, path: string) =>
    invoke<{ data_dir: string; changed: boolean }>('settings_change_data_dir', { token, path })
};

/* ------------------------------ cash --------------------------------- */

export type CashRegisterRow = {
  id: number;
  branch_id: number;
  code: string;
  name: string;
  status: string;
  current_user_id: number | null;
  balance_minor: number;
  movement_count: number;
};

export type CashMovementRow = {
  id: number;
  register_id: number;
  kind: string;
  direction: string;
  amount_minor: number;
  ref_type: string;
  ref_id: number | null;
  occurred_at: string;
  balance_after_minor: number;
};

export type CashKindTotal = {
  kind: string;
  direction: string;
  total_minor: number;
  count: number;
};

export type CashClosingRow = {
  id: number;
  register_id: number;
  closed_at: string;
  opening_balance_minor: number;
  booked_balance_minor: number;
  counted_balance_minor: number;
  variance_minor: number;
  status: string;
};

export type CashRegisterSummary = {
  register: CashRegisterRow;
  opening_balance_minor: number;
  total_in_minor: number;
  total_out_minor: number;
  balance_minor: number;
  movement_count: number;
  today_in_minor: number;
  today_out_minor: number;
  kind_totals: CashKindTotal[];
  last_closing: CashClosingRow | null;
};

export const cashApi = {
  registers: (token: string) => invoke<CashRegisterRow[]>('cash_registers_list', { token }),
  summary: (token: string, register_id: number) =>
    invoke<CashRegisterSummary>('cash_summary', { token, register_id }),
  movements: (token: string, register_id: number, limit = 200) =>
    invoke<CashMovementRow[]>('cash_list_movements', { token, register_id, limit }),
  open: (token: string, branch_id: number, code: string) =>
    invoke<number>('cash_open_register', { token, branch_id, code }),
  record: (
    token: string,
    data: {
      register_id: number;
      kind: string;
      direction: string;
      amount_minor: number;
      ref_type: string;
    }
  ) =>
    invoke<{ id: number; balance_after_minor: number }>('cash_record_movement', {
      token,
      ...data
    }),
  close: (
    token: string,
    data: {
      register_id: number;
      opening_balance_minor: number;
      counted_balance_minor: number;
      cash_variance_account: number;
      cash_account: number;
    }
  ) => invoke<number>('cash_close_register', { token, ...data })
};

/* ------------------------- enrollments / expenses -------------------- */

export type Enrollment = {
  id: number;
  branch_id: number;
  student_id: number;
  course_id: number;
  enrolled_date: string;
  base_fee_minor: number;
  discount_minor: number;
  waiver_minor: number;
  credit_applied_minor: number;
  net_fee_minor: number;
  status: string;
  notes: string;
  frozen_reason: string;
};

export type Expense = {
  id: number;
  branch_id: number;
  category_id: number;
  expense_date: string;
  amount_minor: number;
  beneficiary: string;
  description: string;
  method: string;
  account_id: number;
  status: string;
};

export type ExpenseCategory = {
  id: number;
  code: string;
  name: string;
  is_active: boolean;
};

export const enrollmentsApi = {
  list: (token: string, course_id: number | null = null, student_id: number | null = null) =>
    invoke<Enrollment[]>('enrollments_list', { token, course_id, student_id }),
  enroll: (
    token: string,
    data: {
      student_id: number;
      course_id: number;
      base_fee_minor: number;
      discount_minor?: number;
      waiver_minor?: number;
      enrolled_date?: string;
      notes?: string;
    }
  ) =>
    invoke<Enrollment>('enrollments_enroll', {
      token,
      student_id: data.student_id,
      course_id: data.course_id,
      base_fee_minor: data.base_fee_minor,
      discount_minor: data.discount_minor ?? 0,
      waiver_minor: data.waiver_minor ?? 0,
      enrolled_date: data.enrolled_date ?? new Date().toISOString().slice(0, 10),
      notes: data.notes ?? ''
    }),
  setStatus: (token: string, id: number, new_status: string, reason: string) =>
    invoke<Enrollment>('enrollments_set_status', { token, id, new_status, reason })
};

export const expensesApi = {
  list: (token: string, status: string | null = null, limit = 300) =>
    invoke<Expense[]>('expenses_list', { token, status, limit }),
  categories: (token: string) =>
    invoke<ExpenseCategory[]>('expense_categories_list', { token })
};

/* ----------------------------- accounts ------------------------------ */

export type AccountOption = { id: number; code: string; name: string; kind: string };

export const accountsApi = {
  list: (token: string) => invoke<AccountOption[]>('accounts_list', { token })
};

/* ------------------- education: courses, sessions, attendance -------------- */

export type Course = {
  id: number;
  branch_id: number;
  course_no: number;
  name: string;
  subject_id: number | null;
  teacher_id: number | null;
  room_id: number | null;
  start_date: string;
  end_date: string;
  fee_minor: number;
  status: string;
  max_students: number;
};

export type Session = {
  id: number;
  course_id: number;
  seq_in_course: number;
  session_date: string;
  start_time: string;
  end_time: string;
  room_id: number | null;
  teacher_id: number | null;
  status: string;
  session_kind: string;
  reason: string;
};

export type AttendanceSheetRow = {
  student_id: number;
  student_no: number;
  full_name: string;
  attendance_id: number | null;
  status: string | null;
  late_minutes: number | null;
  notes: string;
  recorded_at: string | null;
};

export const coursesApi = {
  list: (token: string, status: string | null = null) =>
    invoke<Course[]>('courses_list', { token, branch_id: 1, status }),
  create: (
    token: string,
    data: {
      name: string;
      subject_id: number;
      teacher_id: number;
      room_id: number;
      fee_minor: number;
      is_private?: boolean;
    }
  ) => invoke<Course>('courses_create', { token, branch_id: 1, is_private: data.is_private ?? false, ...data }),
  setStatus: (token: string, id: number, new_status: string, reason: string) =>
    invoke<Course>('courses_set_status', { token, id, new_status, reason })
};

export const sessionsApi = {
  list: (token: string, course_id: number | null = null) =>
    invoke<Session[]>('sessions_list', { token, course_id }),
  setStatus: (token: string, id: number, status: string, reason: string) =>
    invoke<Session>('sessions_set_status', { token, id, status, reason }),
  create: (
    token: string,
    data: { course_id: number; session_date: string; start_time: string; end_time: string }
  ) => invoke<Session>('sessions_create', { token, ...data })
};

export const attendanceApi = {
  sheet: (token: string, session_id: number) =>
    invoke<AttendanceSheetRow[]>('attendance_sheet', { token, session_id }),
  record: (
    token: string,
    data: {
      session_id: number;
      student_id: number;
      status: string;
      late_minutes: number;
      notes?: string;
    }
  ) => invoke<{ id: number; status: string }>('attendance_record_student', { token, ...data })
};

/* --------------------------- finance: payments ---------------------------- */

export type PaymentResult = {
  voucher_id: number;
  total_minor: number;
};

export type PaymentVoucherRow = {
  id: number;
  voucher_no: number;
  voucher_date: string;
  beneficiary_type: string;
  beneficiary_name: string;
  amount_minor: number;
  method: string;
  status: string;
  source_type: string;
};

export const paymentsApi = {
  record: (
    token: string,
    data: { enrollment_id: number; amount_minor: number; idempotency_key: string }
  ) => invoke<PaymentResult>('payments_record', { token, ...data }),
  recordService: (
    token: string,
    data: {
      payer_name: string;
      description: string;
      amount_minor: number;
      method: string;
      idempotency_key: string;
    }
  ) => invoke<number>('payments_record_service', { token, ...data })
};

export const paymentVouchersApi = {
  list: (token: string, status: string | null = null, limit = 300) =>
    invoke<PaymentVoucherRow[]>('payment_vouchers_list', { token, status, limit })
};

/* --------------------------- people (extra) ------------------------------ */

export const peopleExtras = {
  students: {
    edit: (token: string, id: number, patch: Record<string, unknown>) =>
      invoke<Student>('students_edit', { token, id, ...patch })
  }
};


/* ------------------- detail views (Phase 17) ---------------------- */

export type StudentEnrollmentDetail = {
  enrollment_id: number; course_id: number; course_name: string; status: string;
  base_fee_minor: number; discount_minor: number; waiver_minor: number;
  net_fee_minor: number; paid_minor: number; remaining_minor: number;
  enrolled_date: string; installment_count: number; installments_open: number;
};
export type StudentDetail = {
  student_id: number; full_name: string; student_no: number; phone: string;
  enrollments: StudentEnrollmentDetail[];
  credit_ledger: { id: number; ref_type: string; credit_minor: number; occurred_at: string; note: string }[];
  receipts: { id: number; voucher_no: number; voucher_date: string; amount_minor: number; course_name: string; method: string }[];
  attendance_total: number; attendance_present: number;
  total_fee_minor: number; total_paid_minor: number; total_remaining_minor: number;
  credit_balance_minor: number;
};

export type TeacherDetail = {
  teacher_id: number; full_name: string; specialty: string; phone: string;
  courses: { course_id: number; course_name: string; subject_name: string; status: string; start_date: string; fee_minor: number; students_count: number; sessions_count: number; hours_done: number }[];
  subjects: string[];
  payments: { id: number; voucher_no: number; voucher_date: string; amount_minor: number; purpose: string; status: string }[];
  payable_minor: number; paid_minor: number; remaining_minor: number;
  sessions_taught: number; hours_taught: number; attendance_records: number;
  teacher_account_balance_minor: number;
};

export type CourseStudentRow = {
  enrollment_id: number; student_id: number; student_name: string; student_no: number;
  net_fee_minor: number; paid_minor: number; remaining_minor: number; status: string;
};
export type CourseSessionRow = {
  id: number; seq: number; session_date: string; start_time: string; end_time: string; status: string; room_name: string;
};
export type CourseDetail = {
  course_id: number; course_name: string; status: string;
  teacher_name: string; subject_name: string; room_name: string;
  fee_minor: number; start_date: string; end_date: string;
  students: CourseStudentRow[]; sessions: CourseSessionRow[]; schedule: CourseSessionRow[];
  collected_minor: number; outstanding_minor: number;
  sessions_total: number; sessions_done: number; hours_planned: number; hours_done: number;
  days_remaining: number; agreement_status: string;
};

export type RoomBookingRow = {
  id: number; course_name: string; session_date: string; start_time: string; end_time: string; status: string; teacher_name: string;
};
export type RoomDetail = {
  room_id: number; code: string; name: string; capacity: number; status: string; description: string;
  active_courses: number; sessions_total: number; sessions_executed: number; hours_used: number;
  bookings: RoomBookingRow[]; schedule: RoomBookingRow[];
};

export const detailsApi = {
  student: (token: string, id: number) => invoke<StudentDetail>('details_student', { token, student_id: id }),
  teacher: (token: string, id: number) => invoke<TeacherDetail>('details_teacher', { token, teacher_id: id }),
  course: (token: string, id: number) => invoke<CourseDetail>('details_course', { token, course_id: id }),
  room: (token: string, id: number) => invoke<RoomDetail>('details_room', { token, room_id: id })
};

/* ------------------- center settings + printing (Phase 17) ---------------- */

export type CenterSettings = {
  id: number; name: string; trade_name: string; logo_ref: string; address: string;
  phones: string; email: string; website: string; license_no: string; tax_no: string;
  currency_code: string; currency_decimals: number; timezone: string; academic_year: string;
  header_text: string; footer_text: string;
};
export type CenterSettingsPatch = Partial<Omit<CenterSettings, 'id'>>;

export const centerApi = {
  uploadLogo: (token: string, filename: string, dataBase64: string) =>
    invoke<string>('settings_upload_logo', { token, filename, data_base64: dataBase64 }),
  get: () => invoke<CenterSettings>('settings_center_get'),
  update: (token: string, patch: CenterSettingsPatch) =>
    invoke<CenterSettings>('settings_center_update', { token, patch })
};

export type PrintProfile = {
  code: string; name: string; doc_type: string; show_header: number; show_footer: number;
  font_size_pt: number; copies: number;
};
export const printApi = {
  profiles: () => invoke<PrintProfile[]>('print_profiles_list'),
  preview: (token: string, documentType: string, id: number) =>
    invoke<string>('print_preview', { token, document_type: documentType, id }),
  execute: (token: string, documentType: string, id: number, copies = 1) =>
    invoke<{ path: string; pages: number }>('print_execute', {
      token, document_type: documentType, id, copies
    })
};

/* ------------------- schedule + session generation (Phase 18) ------------- */

export type ScheduleRule = {
  id: number; course_id: number; course_name: string; weekday: number;
  start_time: string; end_time: string; room_id: number; room_name: string;
  teacher_id: number; teacher_name: string; effective_from: string; effective_to: string;
};
export type Clash = {
  kind: string; message: string; with_course: string; weekday: number;
  start_time: string; end_time: string; room_name: string; teacher_name: string;
};
export type GeneratedSession = {
  course_id: number; course_name: string; session_date: string;
  start_time: string; end_time: string; room_id: number; room_name: string;
  teacher_id: number; teacher_name: string; source_schedule_id: number; seq_in_course: number;
};
export const scheduleApi = {
  list: (token: string, courseId: number | null = null) =>
    invoke<ScheduleRule[]>('schedule_list', { token, course_id: courseId }),
  add: (token: string, data: {
    course_id: number; weekday: number; start_time: string; end_time: string;
    room_id: number; teacher_id: number; effective_from?: string; effective_to?: string;
  }) => invoke<ScheduleRule>('schedule_add', {
    token, ...data, effective_from: data.effective_from ?? '', effective_to: data.effective_to ?? ''
  }),
  remove: (token: string, id: number) => invoke<void>('schedule_delete', { token, id }),
  check: (token: string, data: {
    course_id: number; weekday: number; start_time: string; end_time: string;
    room_id: number; teacher_id: number;
  }) => invoke<Clash[]>('schedule_check_clashes', { token, ...data }),
  generate: (token: string, courseId: number, from: string, to: string, commit: boolean) =>
    invoke<GeneratedSession[]>('sessions_generate', {
      token, course_id: courseId, from, to, commit
    })
};

/* ------------------- Telegram bot (Phase 20) ------------------------ */

export type TelegramSettings = Record<string, string>;
export type TelegramLink = {
  id: number; chat_id: string; person_type: string; person_id: number;
  person_name: string; phone: string; linked_at: string; active: boolean;
};
export type TelegramMessage = {
  id: number; chat_id: string; person_type: string; person_id: number | null;
  direction: string; body: string; kind: string; created_at: string; person_name: string;
};
export type TelegramStats = { links: number; messages: number; notifications: number };

export const telegramApi = {
  settings: (token: string) => invoke<TelegramSettings>('telegram_get_settings', { token }),
  saveSettings: (token: string, values: TelegramSettings) =>
    invoke<void>('telegram_save_settings', { token, values }),
  test: (token: string) => invoke<string>('telegram_test', { token }),
  links: (token: string) => invoke<TelegramLink[]>('telegram_links', { token }),
  link: (token: string, chatId: string, personType: string, personId: number, phone: string) =>
    invoke<void>('telegram_link_manually', { token, chat_id: chatId, person_type: personType, person_id: personId, phone }),
  unlink: (token: string, id: number) => invoke<void>('telegram_unlink', { token, id }),
  setActive: (token: string, id: number, active: boolean) =>
    invoke<void>('telegram_set_active', { token, id, active }),
  messages: (token: string, chat?: string, kind?: string, search?: string, limit = 300) =>
    invoke<TelegramMessage[]>('telegram_messages', { token, chat, kind, search, limit }),
  broadcast: (token: string, text: string, target: string) =>
    invoke<number>('telegram_broadcast', { token, text, target }),
  notifyTest: (token: string, personType: string, personId: number) =>
    invoke<number>('telegram_notify_test', { token, person_type: personType, person_id: personId }),
  stats: (token: string) => invoke<TelegramStats>('telegram_stats', { token })
};

/* ------------------- notifications + service requests (Phase 22) --------- */

export type AppNotification = {
  id: number; kind: string; title: string; body: string; ref_type: string;
  ref_id: number | null; person_type: string; person_id: number | null;
  seen: boolean; created_at: string;
};
export type ServiceRequest = {
  id: number; chat_id: string; phone: string; requester: string; service: string;
  details: string; status: string; response: string; created_at: string;
};
export const notificationsApi = {
  list: (token: string, limit = 100) => invoke<AppNotification[]>('notifications_list', { token, limit }),
  unseen: (token: string) => invoke<number>('notifications_unseen', { token }),
  markSeen: (token: string, id: number) => invoke<void>('notifications_mark_seen', { token, id }),
  markAll: (token: string) => invoke<void>('notifications_mark_all', { token }),
  clear: (token: string) => invoke<void>('notifications_clear', { token })
};
export const serviceRequestsApi = {
  list: (token: string, status?: string) => invoke<ServiceRequest[]>('telegram_service_requests', { token, status }),
  respond: (token: string, id: number, approve: boolean, response: string) =>
    invoke<void>('telegram_respond_service', { token, id, approve, response })
};

/* ---------------------- my account (Phase 23) ---------------------------- */

export type AccountTelegramStatus = {
  user_id: number;
  full_name: string;
  username: string;
  phone: string;
  bot_username: string;
  bot_enabled: boolean;
  linked: boolean;
  chat_id: string | null;
  linked_at: string | null;
  link_active: boolean | null;
};

export const accountApi = {
  telegramStatus: (token: string) =>
    invoke<AccountTelegramStatus>('account_telegram_status', { token }),
  setPhone: (token: string, phone: string) =>
    invoke<string>('account_phone_set', { token, phone }),
  createLinkCode: (token: string) =>
    invoke<string>('account_telegram_link_code', { token }),
  unlinkTelegram: (token: string) =>
    invoke<void>('account_telegram_unlink', { token }),
  openUrl: (token: string, url: string) =>
    invoke<void>('open_external_url', { token, url })
};

/* ------------------- shareable data bundles (Phase 24) ------------------- */

export type ImportResult = { data_dir: string; imported: boolean };

export const backupApi = {
  exportArchive: (token: string, dest?: string) =>
    invoke<string>('backup_export_archive', { token, dest }),
  importArchive: (token: string, path?: string) =>
    invoke<ImportResult>('backup_import_archive', { token, path }),
  openPath: (token: string, path: string) => invoke<void>('open_path', { token, path })
};
