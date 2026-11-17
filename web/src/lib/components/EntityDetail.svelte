<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { sessionToken } from '$lib/stores/app';
  import {
    detailsApi,
    type StudentDetail,
    type TeacherDetail,
    type CourseDetail,
    type RoomDetail, enrollmentsApi, scheduleApi, type ScheduleRule
  } from '$lib/api/app';
  import { peopleApi, type Student } from '$lib/api/people';
  import { toast } from '$lib/toasts';
  import Modal from '$lib/components/Modal.svelte';
  import Field from '$lib/components/Field.svelte';
  import Button from '$lib/components/Button.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import StatCard from '$lib/components/StatCard.svelte';
  import DataTable from '$lib/components/DataTable.svelte';
  import Alert from '$lib/components/Alert.svelte';
  import { count, date, money } from '$lib/format';
  import type { Column } from '$lib/table';

  type Kind = 'student' | 'teacher' | 'course' | 'room';
  let {
    kind,
    id,
    onClose
  }: { kind: Kind; id: number; onClose: () => void } = $props();

  let loading = $state(true);
  let error = $state('');
  let student = $state<StudentDetail | null>(null);
  let teacher = $state<TeacherDetail | null>(null);
  let course = $state<CourseDetail | null>(null);
  let room = $state<RoomDetail | null>(null);

  // add-student-from-course
  let showEnroll = $state(false);
  let students = $state<Student[]>([]);
  let enrollStudentId = $state(0);
  let enrollFeeMajor = $state(0);
  let enrollError = $state('');
  let enrollSaving = $state(false);

  // session generation from course file
  let genFrom = $state('');
  let genTo = $state('');
  let genBusy = $state(false);

  const TITLES: Record<Kind, string> = {
    student: 'ملف الطالب',
    teacher: 'ملف المدرس',
    course: 'ملف الدورة',
    room: 'ملف القاعة'
  };

  onMount(async () => {
    const token = get(sessionToken);
    if (!token) return;
    loading = true;
    error = '';
    try {
      if (kind === 'student') student = await detailsApi.student(token, id);
      else if (kind === 'teacher') teacher = await detailsApi.teacher(token, id);
      else if (kind === 'course') course = await detailsApi.course(token, id);
      else if (kind === 'room') room = await detailsApi.room(token, id);
    } catch (e) {
      error = (e as any)?.message ?? 'تعذّر تحميل التفاصيل';
    } finally {
      loading = false;
    }
  });

  // columns
  const stuEnrCols: Column<StudentDetail['enrollments'][number]>[] = [
    { key: 'course_name', label: 'الدورة' },
    { key: 'net_fee_minor', label: 'الرسوم', num: true, format: (v) => money(Number(v)) },
    { key: 'paid_minor', label: 'المدفوع', num: true, format: (v) => money(Number(v)) },
    { key: 'remaining_minor', label: 'المتبقي', num: true, format: (v) => money(Number(v)) },
    { key: 'installments_open', label: 'أقساط مفتوحة', num: true },
    { key: 'status', label: 'الحالة' }
  ];
  const receiptCols: Column<StudentDetail['receipts'][number]>[] = [
    { key: 'voucher_no', label: 'السند', num: true },
    { key: 'voucher_date', label: 'التاريخ', format: (v) => date(String(v)) },
    { key: 'course_name', label: 'الدورة' },
    { key: 'method', label: 'الطريقة' },
    { key: 'amount_minor', label: 'المبلغ', num: true, format: (v) => money(Number(v)) }
  ];
  const teacherCourseCols: Column<TeacherDetail['courses'][number]>[] = [
    { key: 'course_name', label: 'الدورة' },
    { key: 'subject_name', label: 'المادة' },
    { key: 'status', label: 'الحالة' },
    { key: 'students_count', label: 'الطلاب', num: true },
    { key: 'sessions_count', label: 'الحصص', num: true },
    { key: 'hours_done', label: 'ساعات', num: true, format: (v) => Number(v).toFixed(1) },
    { key: 'fee_minor', label: 'الرسوم', num: true, format: (v) => money(Number(v)) }
  ];
  const teacherPayCols: Column<TeacherDetail['payments'][number]>[] = [
    { key: 'voucher_no', label: 'السند', num: true },
    { key: 'voucher_date', label: 'التاريخ', format: (v) => date(String(v)) },
    { key: 'purpose', label: 'الغرض' },
    { key: 'amount_minor', label: 'المبلغ', num: true, format: (v) => money(Number(v)) },
    { key: 'status', label: 'الحالة' }
  ];
  const courseStudentCols: Column<CourseDetail['students'][number]>[] = [
    { key: 'student_no', label: '#', num: true },
    { key: 'student_name', label: 'الطالب' },
    { key: 'net_fee_minor', label: 'الرسوم', num: true, format: (v) => money(Number(v)) },
    { key: 'paid_minor', label: 'المدفوع', num: true, format: (v) => money(Number(v)) },
    { key: 'remaining_minor', label: 'المتبقي', num: true, format: (v) => money(Number(v)) },
    { key: 'status', label: 'الحالة' }
  ];
  const sessionCols: Column<CourseDetail['sessions'][number]>[] = [
    { key: 'seq', label: '#', num: true },
    { key: 'session_date', label: 'التاريخ', format: (v) => date(String(v)) },
    { key: 'start_time', label: 'من' },
    { key: 'end_time', label: 'إلى' },
    { key: 'room_name', label: 'القاعة' },
    { key: 'status', label: 'الحالة' }
  ];
  const roomCols: Column<RoomDetail['bookings'][number]>[] = [
    { key: 'course_name', label: 'الدورة' },
    { key: 'session_date', label: 'التاريخ', format: (v) => (v ? date(String(v)) : 'جدول أسبوعي') },
    { key: 'start_time', label: 'من' },
    { key: 'end_time', label: 'إلى' },
    { key: 'teacher_name', label: 'المدرس' },
    { key: 'status', label: 'الحالة' }
  ];

  async function reload() {
    const token = get(sessionToken);
    if (!token) return;
    if (kind === 'course') course = await detailsApi.course(token, id);
    else if (kind === 'student') student = await detailsApi.student(token, id);
    else if (kind === 'teacher') teacher = await detailsApi.teacher(token, id);
    else if (kind === 'room') room = await detailsApi.room(token, id);
  }

  async function openEnroll() {
    showEnroll = true; enrollError = '';
    if (course) enrollFeeMajor = Number((course.fee_minor / 100).toFixed(2));
    const token = get(sessionToken);
    if (!token) return;
    try { students = await peopleApi.students.list(token, 1, undefined, undefined, 500); }
    catch (e) { enrollError = (e as any)?.message ?? 'تعذّر تحميل الطلاب'; }
  }

  async function submitEnroll(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token || !course) return;
    enrollError = ''; enrollSaving = true;
    try {
      await enrollmentsApi.enroll(token, {
        student_id: enrollStudentId, course_id: course.course_id,
        base_fee_minor: Math.round(Number(enrollFeeMajor || 0) * 100)
      });
      toast.ok('تم تسجيل الطالب في الدورة — أُنشئ سند قبض وسند دفع');
      showEnroll = false;
      await reload();
    } catch (err) { enrollError = (err as any)?.message ?? 'تعذّر التسجيل'; }
    finally { enrollSaving = false; }
  }

  async function generateSessions() {
    const token = get(sessionToken);
    if (!token || !course || !genFrom || !genTo) return;
    genBusy = true;
    try {
      const made = await scheduleApi.generate(token, course.course_id, genFrom, genTo, true);
      toast.ok(`تم إنشاء ${made.length} حصة`);
      await reload();
    } catch (e) { toast.error((e as any)?.message ?? 'تعذّر إنشاء الحصص'); }
    finally { genBusy = false; }
  }

  const subtitle = $derived(
    student ? student.full_name : teacher ? teacher.full_name : course ? course.course_name : room ? room.name : ''
  );
</script>

<Modal open={true} title={TITLES[kind]} {subtitle} onClose={onClose} width="1080px">
  {#if loading}
    <div class="space-y-3">
      {#each Array(5) as _, i (i)}<div class="ec-skel h-12 rounded-lg"></div>{/each}
    </div>
  {:else if error}
    <Alert kind="error">{error}</Alert>
  {:else if student}
    <div class="ec-kpi-row mb-4">
      <StatCard label="إجمالي الرسوم" value={money(student.total_fee_minor)} icon="₪" accent="var(--ec-blue)" />
      <StatCard label="المدفوع" value={money(student.total_paid_minor)} icon="✓" accent="var(--ec-teal)" />
      <StatCard label="المتبقي" value={money(student.total_remaining_minor)} icon="⏳" accent="var(--ec-amber)" />
      <StatCard label="رصيد الطالب" value={money(student.credit_balance_minor)} icon="💳" accent="#8fae20" />
      <StatCard label="الحضور" value={`${count(student.attendance_present)}/${count(student.attendance_total)}`} icon="✓" accent="var(--ec-teal)" />
    </div>
    <h3 class="ec-card-title mb-2 mt-2">الدورات المسجّلة وحساب كل دورة</h3>
    <DataTable columns={stuEnrCols} rows={student.enrollments} rowKey={(r) => r.enrollment_id}
      emptyTitle="لا توجد تسجيلات" emptyHint="سجّل الطالب في دورة من صفحة التسجيلات." />
    <h3 class="ec-card-title mb-2 mt-5">سندات القبض</h3>
    <DataTable columns={receiptCols} rows={student.receipts} rowKey={(r) => r.id}
      emptyTitle="لا توجد سندات" emptyHint="تُنشأ عند تسجيل دفعة للطالب." />
    {#if student.credit_ledger.length}
      <h3 class="ec-card-title mb-2 mt-5">سجل الرصيد</h3>
      <div class="space-y-1.5">
        {#each student.credit_ledger as c (c.id)}
          <div class="flex justify-between rounded-lg px-3 py-2 text-[13px]" style="background:var(--ec-surface-2)">
            <span>{c.ref_type} — {c.note}</span><span class="ec-num">{money(c.credit_minor)}</span>
          </div>
        {/each}
      </div>
    {/if}
  {:else if teacher}
    <div class="ec-kpi-row mb-4">
      <StatCard label="المستحق" value={money(teacher.payable_minor)} icon="₪" accent="var(--ec-blue)" />
      <StatCard label="المدفوع" value={money(teacher.paid_minor)} icon="✓" accent="var(--ec-teal)" />
      <StatCard label="المتبقي" value={money(teacher.remaining_minor)} icon="⏳" accent="var(--ec-amber)" />
      <StatCard label="الحصص المنفّذة" value={count(teacher.sessions_taught)} icon="📚" accent="#8fae20" />
      <StatCard label="ساعات التدريس" value={teacher.hours_taught.toFixed(1)} icon="⏱" accent="var(--ec-teal)" />
    </div>
    <h3 class="ec-card-title mb-2">الدورات والمواد ({teacher.subjects.join('، ') || '—'})</h3>
    <DataTable columns={teacherCourseCols} rows={teacher.courses} rowKey={(r) => r.course_id}
      emptyTitle="لا توجد دورات" emptyHint="اربط المدرس بالدورات من صفحة الدورات." />
    <h3 class="ec-card-title mb-2 mt-5">سندات الصرف الخاصة بالمدرس</h3>
    <DataTable columns={teacherPayCols} rows={teacher.payments} rowKey={(r) => r.id}
      emptyTitle="لا توجد سندات" emptyHint="تُنشأ سندات الصرف للمدرس عند الدفع." />
  {:else if course}
    <div class="ec-kpi-row mb-4">
      <StatCard label="الطلاب" value={count(course.students.length)} icon="👤" accent="var(--ec-blue)" />
      <StatCard label="التحصيل" value={money(course.collected_minor)} icon="💰" accent="var(--ec-teal)" />
      <StatCard label="المتبقي" value={money(course.outstanding_minor)} icon="⏳" accent="var(--ec-amber)" />
      <StatCard label="الحصص" value={`${count(course.sessions_done)}/${count(course.sessions_total)}`} icon="📚" accent="#8fae20" />
      <StatCard label="الساعات" value={`${course.hours_done.toFixed(1)}/${course.hours_planned.toFixed(1)}`} icon="⏱" accent="var(--ec-teal)" />
      <StatCard label="الأيام المتبقية" value={course.days_remaining < 0 ? '—' : count(course.days_remaining)} icon="📅" accent="var(--ec-blue)" />
    </div>
    <div class="mb-4 flex flex-wrap gap-2 text-[13px]" style="color:var(--ec-text-2)">
      <span>المدرس: <b>{course.teacher_name || '—'}</b></span>
      <span>•</span><span>المادة: <b>{course.subject_name || '—'}</b></span>
      <span>•</span><span>القاعة: <b>{course.room_name || '—'}</b></span>
      <span>•</span><span>الرسوم: <b>{money(course.fee_minor)}</b></span>
      <span>•</span><span>الاتفاقية: <Badge status={course.agreement_status} /></span>
    </div>
    <div class="mb-3 flex justify-end">
      <Button variant="primary" size="sm" onclick={openEnroll}>+ تسجيل طالب في هذه الدورة</Button>
    </div>
    <h3 class="ec-card-title mb-2">الطلاب وحسابهم</h3>
    <DataTable columns={courseStudentCols} rows={course.students} rowKey={(r) => r.enrollment_id}
      emptyTitle="لا يوجد طلاب" emptyHint="سجّل الطلاب في الدورة من صفحة التسجيلات." />
    <div class="mt-5 rounded-xl border p-3" style="border-color:var(--ec-border); background:var(--ec-surface-2)">
      <div class="mb-2 text-[13px] font-bold">توليد الحصص من الجدول الأسبوعي</div>
      <div class="ec-toolbar" style="margin-bottom:0">
        <input class="ec-input" type="date" bind:value={genFrom} dir="ltr" aria-label="من تاريخ" />
        <input class="ec-input" type="date" bind:value={genTo} dir="ltr" aria-label="إلى تاريخ" />
        <Button variant="soft" size="sm" loading={genBusy} disabled={!genFrom || !genTo} onclick={generateSessions}>📅 توليد</Button>
      </div>
    </div>

    {#if course.schedule.length}
      <h3 class="ec-card-title mb-2 mt-5">الجدول الأسبوعي</h3>
      <div class="flex flex-wrap gap-2">
        {#each course.schedule as sc (sc.id)}
          <span class="ec-badge ec-badge-info no-dot">يوم {sc.seq} • {sc.start_time}–{sc.end_time} • {sc.room_name}</span>
        {/each}
      </div>
    {/if}
    <h3 class="ec-card-title mb-2 mt-5">الحصص</h3>
    <DataTable columns={sessionCols} rows={course.sessions} rowKey={(r) => r.id}
      emptyTitle="لا توجد حصص" emptyHint="تُنشأ الحصص من جدول الدورة." />
    {#if course.schedule.length}
      <h3 class="ec-card-title mb-2 mt-5">الجدول الأسبوعي</h3>
      <div class="flex flex-wrap gap-2">
        {#each course.schedule as s (s.id)}
          <span class="ec-badge ec-badge-info no-dot">يوم {s.seq} • {s.start_time}–{s.end_time} • {s.room_name}</span>
        {/each}
      </div>
    {/if}
  {:else if room}
    <div class="ec-kpi-row mb-4">
      <StatCard label="السعة" value={count(room.capacity)} icon="🪑" accent="var(--ec-blue)" />
      <StatCard label="دورات نشطة" value={count(room.active_courses)} icon="📚" accent="var(--ec-teal)" />
      <StatCard label="إجمالي الحصص" value={count(room.sessions_total)} icon="📅" accent="#8fae20" />
      <StatCard label="ساعات الاستخدام" value={room.hours_used.toFixed(1)} icon="⏱" accent="var(--ec-amber)" />
    </div>
    <h3 class="ec-card-title mb-2">جدول الحجز</h3>
    <DataTable columns={roomCols} rows={room.schedule} rowKey={(r) => r.id}
      emptyTitle="لا يوجد جدول حجز" emptyHint="اربط القاعة بجدول دورة." />
    <h3 class="ec-card-title mb-2 mt-5">الحركات والحصص التي استخدمت القاعة</h3>
    <DataTable columns={roomCols} rows={room.bookings} rowKey={(r) => `b${r.id}`}
      emptyTitle="لا توجد حركات" emptyHint="ستظهر هنا كل حصة استخدمت القاعة." />
  {/if}

  <div class="mt-5 flex justify-end">
    <Button variant="ghost" onclick={onClose}>إغلاق</Button>
  </div>
</Modal>

{#if showEnroll && course}
  <Modal open={true} title={`تسجيل طالب في ${course.course_name}`} subtitle="سيُنشأ سند قبض وسند دفع تلقائياً." onClose={() => (showEnroll = false)} width="560px">
    <form class="space-y-4" onsubmit={submitEnroll}>
      {#if enrollError}<Alert kind="error">{enrollError}</Alert>{/if}
      <Field label="الطالب" required>
        <select class="ec-select" bind:value={enrollStudentId}>
          <option value={0}>— اختر طالباً —</option>
          {#each students as st (st.id)}<option value={st.id}>{st.full_name} (#{st.student_no})</option>{/each}
        </select>
      </Field>
      <Field label="رسوم التسجيل" hint="تُملأ تلقائياً من رسوم الدورة — بالعملة.">
        <input class="ec-input ec-num" type="number" min="0" step="0.01" bind:value={enrollFeeMajor} dir="ltr" />
      </Field>
      <div class="flex justify-end gap-2">
        <Button variant="ghost" onclick={() => (showEnroll = false)}>إلغاء</Button>
        <Button type="submit" variant="primary" loading={enrollSaving} disabled={!enrollStudentId}>تسجيل</Button>
      </div>
    </form>
  </Modal>
{/if}
