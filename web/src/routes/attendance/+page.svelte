<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { sessionToken } from '$lib/stores/app';
  import {
    attendanceApi,
    coursesApi,
    sessionsApi,
    type AttendanceSheetRow,
    type Course,
    type Session
  } from '$lib/api/app';
  import { toast } from '$lib/toasts';
  import { goto } from '$app/navigation';
  import { invoke } from '$lib/api';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Card from '$lib/components/Card.svelte';
  import Button from '$lib/components/Button.svelte';
  import Alert from '$lib/components/Alert.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import Field from '$lib/components/Field.svelte';
  import StatCard from '$lib/components/StatCard.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import { count, date, dateTime, statusInfo } from '$lib/format';

  let sessions = $state<Session[]>([]);
  let courses = $state<Course[]>([]);
  let sessionId = $state(0);
  let sheet = $state<AttendanceSheetRow[]>([]);
  let loading = $state(true);
  let loadingSheet = $state(false);
  let error = $state('');
  let savingId = $state(0);

  const courseName = (id: number) => courses.find((c) => c.id === id)?.name ?? `#${id}`;
  const session = $derived(sessions.find((s) => s.id === sessionId) ?? null);

  /// Upcoming sessions first — the ones a teacher is most likely marking now.
  const sessionOptions = $derived(
    [...sessions].sort((a, b) => b.session_date.localeCompare(a.session_date)).slice(0, 120)
  );

  const tally = $derived({
    recorded: sheet.filter((r) => r.status !== null).length,
    present: sheet.filter((r) => r.status === 'present').length,
    absent: sheet.filter((r) => r.status === 'absent').length,
    late: sheet.filter((r) => r.status === 'late').length,
    excused: sheet.filter((r) => r.status === 'excused').length,
    pending: sheet.filter((r) => r.status === null).length
  });

  const rate = $derived(
    tally.recorded === 0
      ? 0
      : Math.round((100 * (tally.present + tally.late)) / tally.recorded)
  );

  async function loadSessions() {
    const token = get(sessionToken);
    if (!token) {
      loading = false;
      return;
    }
    loading = true;
    error = '';
    try {
      const [s, c] = await Promise.all([sessionsApi.list(token), coursesApi.list(token)]);
      sessions = s;
      courses = c;
      // default to the most recent session that still has unmarked students
      if (!sessionId && s.length) sessionId = s[0].id;
    } catch (e) {
      error = (e as Error)?.message ?? 'تعذّر تحميل الحصص';
    } finally {
      loading = false;
    }
  }

  async function loadSheet(id: number) {
    const token = get(sessionToken);
    if (!token || !id) {
      sheet = [];
      return;
    }
    loadingSheet = true;
    error = '';
    try {
      sheet = await attendanceApi.sheet(token, id);
    } catch (e) {
      error = (e as Error)?.message ?? 'تعذّر تحميل ورقة الحضور';
      sheet = [];
    } finally {
      loadingSheet = false;
    }
  }

  async function mark(row: AttendanceSheetRow, status: string) {
    const token = get(sessionToken);
    if (!token || !sessionId) return;
    savingId = row.student_id;
    try {
      await attendanceApi.record(token, {
        session_id: sessionId,
        student_id: row.student_id,
        status,
        late_minutes: status === 'late' ? 10 : 0,
        notes: ''
      });
      // optimistic update, then re-sync the whole sheet so tallies stay true
      sheet = sheet.map((r) =>
        r.student_id === row.student_id ? { ...r, status, attendance_id: r.attendance_id ?? 0 } : r
      );
    } catch (err) {
      toast.error((err as Error)?.message ?? 'تعذّر تسجيل الحضور');
    } finally {
      savingId = 0;
    }
  }

  async function markAll(status: string) {
    const token = get(sessionToken);
    if (!token || !sessionId) return;
    const pending = sheet.filter((r) => r.status === null);
    if (!pending.length) return;
    try {
      for (const r of pending) {
        await attendanceApi.record(token, {
          session_id: sessionId,
          student_id: r.student_id,
          status,
          late_minutes: 0,
          notes: 'تسجيل جماعي'
        });
      }
      toast.ok(`تم تسجيل ${count(pending.length)} طالب كـ«${statusInfo(status).label}»`);
      await loadSheet(sessionId);
    } catch (err) {
      toast.error((err as Error)?.message ?? 'تعذّر التسجيل الجماعي');
      await loadSheet(sessionId);
    }
  }

  let teacherStatus = $state('present');
  let teacherNotes = $state('');

  async function recordTeacher() {
    const token = get(sessionToken);
    if (!token || !session || !session.teacher_id) return;
    try {
      await invoke('attendance_record_teacher', {
        token, session_id: sessionId, teacher_id: session.teacher_id,
        status: teacherStatus, notes: teacherNotes
      });
      toast.ok('تم تسجيل حضور المدرس');
      teacherNotes = '';
    } catch (err) { toast.error((err as any)?.message ?? 'تعذّر تسجيل حضور المدرس'); }
  }

  const REPORTS = [
    { code: 'rpt_attendance', label: 'الحضور والغياب', icon: '📋' },
    { code: 'rpt_attendance_rate', label: 'نسب الالتزام', icon: '📊' },
    { code: 'rpt_attendance_summary', label: 'ملخّص كل حصة', icon: '📑' },
    { code: 'rpt_teacher_performance', label: 'أداء المدرسين', icon: '👔' },
    { code: 'rpt_teacher_workload', label: 'حجم عمل المدرسين', icon: '⚖' },
    { code: 'rpt_sessions', label: 'الحصص', icon: '🗓' },
    { code: 'rpt_schedules', label: 'الجداول', icon: '📅' },
    { code: 'rpt_room_occupancy', label: 'إشغال القاعات', icon: '▦' }
  ];

  const STATUSES = [
    { id: 'present', label: 'حاضر', cls: 'ec-btn-success' },
    { id: 'absent', label: 'غائب', cls: 'ec-btn-danger' },
    { id: 'late', label: 'متأخر', cls: 'ec-btn-soft' },
    { id: 'excused', label: 'معذور', cls: 'ec-btn-ghost' }
  ];

  onMount(async () => {
    await loadSessions();
    if (sessionId) await loadSheet(sessionId);
  });
</script>

<PageHeader
  title="الحضور"
  subtitle="اختر حصصة ثم سجّل حالة كل طالب — الحضور مرتبط بالتسجيل في دورة الحصة فقط"
>
  <Button variant="ghost" size="sm" onclick={loadSessions} loading={loading}>↻ تحديث</Button>
</PageHeader>

{#if error}
  <div class="mb-4"><Alert kind="error" onClose={() => (error = '')}>{error}</Alert></div>
{/if}

<Card title="اختيار الحصة">
  {#if loading}
    <div class="ec-skel h-11 w-full max-w-xl rounded-lg"></div>
  {:else if sessions.length === 0}
    <EmptyState
      icon="🗓"
      title="لا توجد حصص"
      hint="أنشئ دورة أولاً — تُنشأ حصصها تلقائياً من جدول الدورة."
    />
  {:else}
    <div class="ec-toolbar" style="margin-bottom:0">
      <label class="ec-label" for="sess">الحصة</label>
      <select
        id="sess"
        class="ec-select"
        style="max-width:520px"
        value={sessionId}
        onchange={(e) => {
          sessionId = Number(e.currentTarget.value);
          loadSheet(sessionId);
        }}
      >
        {#each sessionOptions as s (s.id)}
          <option value={s.id}>
            {courseName(s.course_id)} — {date(s.session_date)} ({s.start_time}) [{statusInfo(s.status).label}]
          </option>
        {/each}
      </select>
      {#if session}
        <Badge status={session.status} />
      {/if}
    </div>
  {/if}
</Card>

<div class="ec-grid ec-cols-2 mt-4">
  <Card title="تقارير الحضور والجدول">
    <div class="flex flex-wrap gap-2">
      {#each REPORTS as r (r.code)}
        <Button variant="ghost" size="sm" onclick={() => goto(`/reports?code=${r.code}`)}>{r.icon} {r.label}</Button>
      {/each}
    </div>
  </Card>
  <Card title="حضور المدرس">
    {#if session && session.teacher_id}
      <div class="space-y-3">
        <div class="text-[13px]" style="color:var(--ec-text-2)">سجّل حالة حضور المدرس لهذه الحصة.</div>
        <div class="ec-form-grid">
          <Field label="الحالة">
            <select class="ec-select" bind:value={teacherStatus}>
              <option value="present">حاضر</option>
              <option value="absent">غائب</option>
              <option value="late">متأخر</option>
              <option value="excused">معذور</option>
            </select>
          </Field>
          <Field label="ملاحظة"><input class="ec-input" bind:value={teacherNotes} placeholder="اختياري" /></Field>
        </div>
        <Button variant="soft" size="sm" onclick={recordTeacher}>✓ تسجيل حضور المدرس</Button>
      </div>
    {:else}
      <div class="text-[13px]" style="color:var(--ec-text-3)">لا يوجد مدرس مرتبط بهذه الحصة.</div>
    {/if}
  </Card>
</div>

{#if sessionId}
  <div class="ec-kpi-row mt-4">
    <StatCard label="الحاضرون" value={count(tally.present)} icon="✓" accent="var(--ec-teal)" />
    <StatCard label="الغائبون" value={count(tally.absent)} icon="✕" accent="var(--ec-red)" />
    <StatCard label="المتأخرون" value={count(tally.late)} icon="⏰" accent="var(--ec-amber)" />
    <StatCard label="المعذورون" value={count(tally.excused)} icon="ℹ" accent="var(--ec-blue)" />
    <StatCard
      label="بانتظار التسجيل"
      value={count(tally.pending)}
      icon="…"
      accent={tally.pending > 0 ? '#b7791f' : 'var(--ec-teal)'}
      foot={tally.recorded > 0 ? `نسبة الالتزام ${rate}٪` : ''}
    />
  </div>

  <Card title="ورقة الحضور" classes="mt-4" pad={false}>
    <div
      class="flex flex-wrap items-center justify-between gap-3 border-b px-5 py-4"
      style="border-color:var(--ec-border)"
    >
      <div class="text-[13px]" style="color:var(--ec-text-2)">
        {#if session}
          {courseName(session.course_id)} • {date(session.session_date)} • الحصة رقم {session.seq_in_course}
        {/if}
      </div>
      <div class="flex flex-wrap gap-2">
        <Button
          variant="success"
          size="sm"
          disabled={tally.pending === 0}
          onclick={() => markAll('present')}
        >
          تسجيل الجميع حاضرين
        </Button>
        <Button
          variant="ghost"
          size="sm"
          disabled={tally.pending === 0}
          onclick={() => markAll('absent')}
        >
          تسجيل الجميع غائبين
        </Button>
      </div>
    </div>

    <div class="p-4">
      {#if loadingSheet}
        <div class="space-y-2">
          {#each Array(6) as _, i (i)}
            <div class="ec-skel h-12 rounded-lg"></div>
          {/each}
        </div>
      {:else if sheet.length === 0}
        <EmptyState
          icon="👤"
          title="لا طلاب مسجّلين في هذه الحصة"
          hint="سجّل الطلاب في دورة هذه الحصة أولاً من صفحة التسجيلات."
        />
      {:else}
        <div class="space-y-2">
          {#each sheet as row (row.student_id)}
            {@const tone = row.status ? statusInfo(row.status) : null}
            <div
              class="flex flex-wrap items-center gap-3 rounded-xl border px-4 py-2.5 transition-colors"
              style="border-color:{row.status ? 'var(--ec-border)' : 'color-mix(in srgb, var(--ec-amber) 35%, transparent)'}; background:{row.status ? 'var(--ec-surface)' : 'var(--ec-amber-soft)'}"
            >
              <span class="ec-num w-14 flex-none text-[13px] font-bold" style="color:var(--ec-text-3)">
                {row.student_no}
              </span>
              <span class="min-w-[160px] flex-1 text-[14px] font-semibold">{row.full_name}</span>

              <span class="w-24 flex-none">
                {#if tone}
                  <Badge status={row.status ?? ''} />
                {:else}
                  <span class="ec-badge ec-badge-warn">لم يُسجَّل</span>
                {/if}
              </span>

              <span class="flex flex-none gap-1.5">
                {#each STATUSES as st (st.id)}
                  <button
                    type="button"
                    class="ec-btn {st.cls} ec-btn-sm"
                    disabled={savingId === row.student_id}
                    class:opacity-100={row.status === st.id}
                    class:ring-2={row.status === st.id}
                    style={row.status === st.id
                      ? 'box-shadow: 0 0 0 2px var(--ec-navy);'
                      : 'opacity:.62;'}
                    onclick={() => mark(row, st.id)}
                  >
                    {st.label}
                  </button>
                {/each}
              </span>

              {#if row.recorded_at}
                <span class="flex-none text-[11.5px]" style="color:var(--ec-text-3)">
                  {dateTime(row.recorded_at)}
                </span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </Card>
{/if}
