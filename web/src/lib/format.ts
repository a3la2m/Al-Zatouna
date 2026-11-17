/// Formatting helpers — money, dates and Arabic status labels.
/// Money is stored everywhere as integer minor units (ADR: no floats).

const CURRENCY_SYMBOL: Record<string, string> = {
  ILS: '₪',
  JOD: 'د.أ',
  USD: '$',
  EUR: '€',
  SAR: 'ر.س',
  AED: 'د.إ',
  EGP: 'ج.م'
};

/** 1234567 → "12,345.67" (grouped, always 2 decimals). */
export function formatMinor(minor: number | null | undefined, decimals = 2): string {
  const v = Number(minor ?? 0);
  const factor = 10 ** decimals;
  const major = v / factor;
  return major.toLocaleString('en-US', {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals
  });
}

/** 1234567 → "12,345.67 ₪" */
export function money(minor: number | null | undefined, currency = 'ILS', decimals = 2): string {
  const sym = CURRENCY_SYMBOL[currency] ?? currency;
  return `${formatMinor(minor, decimals)} ${sym}`;
}

/** Compact money for tight spaces: 1234567 → "12.3K" */
export function moneyCompact(minor: number | null | undefined, currency = 'ILS'): string {
  const v = Number(minor ?? 0) / 100;
  const sym = CURRENCY_SYMBOL[currency] ?? currency;
  const abs = Math.abs(v);
  if (abs >= 1_000_000) return `${(v / 1_000_000).toFixed(1)}M ${sym}`;
  if (abs >= 1_000) return `${(v / 1_000).toFixed(1)}K ${sym}`;
  return `${v.toFixed(0)} ${sym}`;
}

/** Plain integer counts with grouping. */
export function count(n: number | null | undefined): string {
  return Number(n ?? 0).toLocaleString('en-US');
}

/** "2026-11-15" → "15/11/2026" (falls back to the raw string). */
export function date(iso: string | null | undefined): string {
  if (!iso) return '—';
  const d = new Date(iso.length <= 10 ? `${iso}T00:00:00` : iso);
  if (Number.isNaN(d.getTime())) return iso;
  const dd = String(d.getDate()).padStart(2, '0');
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  return `${dd}/${mm}/${d.getFullYear()}`;
}

/** "2026-11-15T12:30:00Z" → "15/11/2026 12:30" */
export function dateTime(iso: string | null | undefined): string {
  if (!iso) return '—';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return date(iso);
  const hh = String(d.getHours()).padStart(2, '0');
  const mi = String(d.getMinutes()).padStart(2, '0');
  return `${date(iso)} ${hh}:${mi}`;
}

/** "2026-11" → "نوفمبر 2026" */
const MONTHS_AR = [
  'يناير',
  'فبراير',
  'مارس',
  'أبريل',
  'مايو',
  'يونيو',
  'يوليو',
  'أغسطس',
  'سبتمبر',
  'أكتوبر',
  'نوفمبر',
  'ديسمبر'
];

export function monthLabel(ym: string | null | undefined): string {
  if (!ym) return '';
  const [y, m] = ym.split('-');
  const idx = Number(m) - 1;
  if (Number.isNaN(idx) || idx < 0 || idx > 11) return ym;
  return `${MONTHS_AR[idx]} ${y}`;
}

/** Short Arabic label for a YYYY-MM axis tick. */
export function monthShort(ym: string | null | undefined): string {
  if (!ym) return '';
  const [y, m] = ym.split('-');
  const idx = Number(m) - 1;
  if (Number.isNaN(idx) || idx < 0 || idx > 11) return ym;
  return `${MONTHS_AR[idx].slice(0, 3)} ${y.slice(2)}`;
}

const AR_DAYS = ['الأحد', 'الإثنين', 'الثلاثاء', 'الأربعاء', 'الخميس', 'الجمعة', 'السبت'];

/** "2026-11-15" → "الأحد 15/11" */
export function dayLabel(iso: string | null | undefined): string {
  if (!iso) return '';
  const d = new Date(`${iso.slice(0, 10)}T00:00:00`);
  if (Number.isNaN(d.getTime())) return iso;
  return `${AR_DAYS[d.getDay()]} ${d.getDate()}/${d.getMonth() + 1}`;
}

/** Human "منذ 3 ساعات". */
export function ago(iso: string | null | undefined): string {
  if (!iso) return '—';
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return iso;
  const s = Math.max(0, Math.floor((Date.now() - then) / 1000));
  if (s < 60) return 'الآن';
  const m = Math.floor(s / 60);
  if (m < 60) return `منذ ${m} دقيقة`;
  const h = Math.floor(m / 60);
  if (h < 24) return `منذ ${h} ساعة`;
  const d = Math.floor(h / 24);
  if (d < 30) return `منذ ${d} يوم`;
  const mo = Math.floor(d / 30);
  if (mo < 12) return `منذ ${mo} شهر`;
  return `منذ ${Math.floor(mo / 12)} سنة`;
}

/* --------------------------------------------------------------------------
   Status → { label, tone }. Every screen renders statuses through this so the
   same state never shows up as two different words/colours across the app.
   -------------------------------------------------------------------------- */
export type Tone = 'ok' | 'info' | 'warn' | 'danger' | 'muted' | 'accent';

const STATUS: Record<string, [string, Tone]> = {
  // people
  active: ['نشط', 'ok'],
  inactive: ['غير نشط', 'muted'],
  frozen: ['مجمّد', 'warn'],
  archived: ['مؤرشف', 'muted'],
  available: ['متاحة', 'ok'],
  reserved: ['محجوزة', 'warn'],
  maintenance: ['صيانة', 'danger'],
  // education
  draft: ['مسودة', 'muted'],
  registration_open: ['التسجيل مفتوح', 'info'],
  completed: ['مكتمل', 'ok'],
  suspended: ['معلّق', 'warn'],
  cancelled: ['ملغى', 'danger'],
  scheduled: ['مجدولة', 'info'],
  executed: ['منفّذة', 'ok'],
  postponed: ['مؤجلة', 'warn'],
  makeup: ['تعويضية', 'accent'],
  // enrollments
  withdrawn: ['منسحب', 'danger'],
  transferred: ['محوّل', 'accent'],
  // finance
  posted: ['مرحّل', 'ok'],
  paid: ['مدفوع', 'ok'],
  pending: ['قيد الانتظار', 'warn'],
  pending_approval: ['بانتظار الاعتماد', 'warn'],
  approved: ['معتمد', 'ok'],
  rejected: ['مرفوض', 'danger'],
  reversed: ['معكوس', 'danger'],
  submitted: ['مقدَّم', 'info'],
  open: ['مفتوح', 'info'],
  partially_paid: ['مدفوع جزئياً', 'warn'],
  decided: ['مقرَّر', 'accent'],
  deferred: ['مؤجَّل', 'warn'],
  // attendance
  present: ['حاضر', 'ok'],
  absent: ['غائب', 'danger'],
  late: ['متأخر', 'warn'],
  excused: ['معذور', 'info'],
  // cash
  closed: ['مغلق', 'muted'],
  // payment voucher purposes
  draft_only: ['—', 'muted']
};

export function statusInfo(status: string | null | undefined): { label: string; tone: Tone } {
  if (!status) return { label: '—', tone: 'muted' };
  const hit = STATUS[status];
  if (hit) return { label: hit[0], tone: hit[1] };
  return { label: status, tone: 'muted' };
}

export function statusLabel(status: string | null | undefined): string {
  return statusInfo(status).label;
}

export function toneClass(tone: Tone): string {
  switch (tone) {
    case 'ok':
      return 'ec-badge-ok';
    case 'info':
      return 'ec-badge-info';
    case 'warn':
      return 'ec-badge-warn';
    case 'danger':
      return 'ec-badge-danger';
    case 'accent':
      return 'ec-badge-accent';
    default:
      return 'ec-badge-muted';
  }
}

/** Direction of a cash movement. */
export function directionLabel(dir: string): string {
  return dir === 'in' ? 'قبض' : dir === 'out' ? 'صرف' : dir;
}

/** Cash movement kind → Arabic. */
export const KIND_LABELS: Record<string, string> = {
  opening: 'رصيد افتتاحي',
  receipt: 'قبض',
  payment: 'صرف',
  transfer_in: 'تحويل وارد',
  transfer_out: 'تحويل صادر',
  adjustment: 'تسوية'
};

/** Report category → Arabic + tone. */
export const REPORT_CATEGORY: Record<string, [string, Tone]> = {
  admin: ['إداري', 'info'],
  financial: ['مالي', 'ok'],
  control: ['رقابي', 'accent']
};
