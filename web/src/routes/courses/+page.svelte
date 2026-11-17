<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { sessionToken } from '$lib/stores/app';
  import { coursesApi, type Course } from '$lib/api/app';
  import { peopleApi, type Room, type Subject, type Teacher } from '$lib/api/people';
  import { toast } from '$lib/toasts';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Card from '$lib/components/Card.svelte';
  import Button from '$lib/components/Button.svelte';
  import Alert from '$lib/components/Alert.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import Field from '$lib/components/Field.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import DataTable from '$lib/components/DataTable.svelte';
  import EntityDetail from '$lib/components/EntityDetail.svelte';
  import { scheduleApi, type Clash } from '$lib/api/app';
  import Tabs from '$lib/components/Tabs.svelte';
  import { date, money } from '$lib/format';

  let courses = $state<Course[]>([]);
  let subjects = $state<Subject[]>([]);
  let teachers = $state<Teacher[]>([]);
  let rooms = $state<Room[]>([]);
  let loading = $state(true);
  let error = $state('');

  let detailId = $state<number | null>(null);

  let statusFilter = $state('all');
  let search = $state('');

  let showForm = $state(false);
  let name = $state('');
  let subjectId = $state(0);
  let teacherId = $state(0);
  let roomId = $state(0);
  let feeMajor = $state(0);
  let startDate = $state('');
  let endDate = $state('');
  let saving = $state(false);
  let formError = $state('');

  // weekly schedule rows entered at course creation
  type SchedRow = { day: number; start: string; end: string; room: number; teacher: number };
  let schedRows = $state<SchedRow[]>([]);
  let scheduleClashes = $state<Clash[]>([]);

  const DAYS: Record<number, string> = { 1:'الاثنين',2:'الثلاثاء',3:'الأربعاء',4:'الخميس',5:'الجمعة',6:'السبت',7:'الأحد' };

  function addSchedRow() {
    schedRows.push({ day: 1, start: '10:00', end: '11:00', room: roomId || 1, teacher: teacherId || 1 });
  }
  function removeSchedRow(i: number) { schedRows.splice(i, 1); }

  async function checkScheduleClashes() {
    const token = get(sessionToken);
    if (!token) return;
    scheduleClashes = [];
    try {
      for (const r of schedRows) {
        const c = await scheduleApi.check(token, {
          course_id: 0, weekday: r.day, start_time: r.start, end_time: r.end,
          room_id: r.room, teacher_id: r.teacher
        });
        scheduleClashes.push(...c);
      }
      if (!scheduleClashes.length) toast.ok('لا توجد تعارضات — يمكن إنشاء الدورة');
    } catch (e) { toast.error((e as any)?.message ?? 'تعذّر فحص التعارض'); }
  }

  const TABS = [
    { id: 'all', label: 'الكل' },
    { id: 'draft', label: 'مسودة' },
    { id: 'registration_open', label: 'التسجيل مفتوح' },
    { id: 'active', label: 'نشطة' },
    { id: 'completed', label: 'مكتملة' }
  ];

  const filtered = $derived(
    courses.filter(
      (c) =>
        (statusFilter === 'all' || c.status === statusFilter) &&
        (search.trim() === '' || c.name.includes(search.trim()))
    )
  );

  const nameOf = {
    subject: (id: number | null) => subjects.find((s) => s.id === id)?.name ?? '—',
    teacher: (id: number | null) => teachers.find((t) => t.id === id)?.full_name ?? '—',
    room: (id: number | null) => rooms.find((r) => r.id === id)?.name ?? '—'
  };

  async function load() {
    const token = get(sessionToken);
    if (!token) {
      loading = false;
      return;
    }
    loading = true;
    error = '';
    try {
      const [c, s, t, r] = await Promise.all([
        coursesApi.list(token),
        peopleApi.subjects.list(token, false),
        peopleApi.teachers.list(token, 1),
        peopleApi.rooms.list(token, 1)
      ]);
      courses = c;
      subjects = s;
      teachers = t;
      rooms = r;
      if (!subjectId && s.length) subjectId = s[0].id;
      if (!teacherId && t.length) teacherId = t[0].id;
      if (!roomId && r.length) roomId = r[0].id;
    } catch (e) {
      error = (e as Error)?.message ?? 'تعذّر تحميل الدورات';
    } finally {
      loading = false;
    }
  }

  async function setStatus(c: Course, new_status: string) {
    const token = get(sessionToken);
    if (!token) return;
    const reason =
      new_status === 'cancelled' || new_status === 'suspended' || new_status === 'archived'
        ? prompt('سبب؟', '') ?? ''
        : '';
    if ((new_status === 'cancelled' || new_status === 'suspended' || new_status === 'archived') && !reason)
      return;
    try {
      await coursesApi.setStatus(token, c.id, new_status, reason);
      toast.ok(`تم تحديث حالة «${c.name}»`);
      await load();
    } catch (err) {
      toast.error((err as any)?.message ?? 'تعذّر تحديث الحالة');
    }
  }

  async function create(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    formError = '';
    saving = true;
    try {
      const created = await coursesApi.create(token, {
        name: name.trim(),
        subject_id: subjectId,
        teacher_id: teacherId,
        room_id: roomId,
        fee_minor: Math.round(Number(feeMajor || 0) * 100)
      });
      for (const r of schedRows) {
        await scheduleApi.add(token, {
          course_id: created.id, weekday: r.day, start_time: r.start, end_time: r.end,
          room_id: r.room, teacher_id: r.teacher
        });
      }
      toast.ok(`تم إنشاء دورة «${name.trim()}»${schedRows.length ? ` بـ ${schedRows.length} موعد أسبوعي` : ''}`);
      schedRows = [];
      scheduleClashes = [];
      showForm = false;
      name = '';
      feeMajor = 0;
      await load();
    } catch (err) {
      console.error('courses_create failed', err);
      formError =
        (err as any)?.message ||
        (err as any)?.toString?.() ||
        'تعذّر إنشاء الدورة';
    } finally {
      saving = false;
    }
  }

  onMount(load);
</script>

{#snippet statusCell(c: Course)}
  <Badge status={c.status} />
{/snippet}

{#snippet feeCell(c: Course)}
  <span class="ec-num">{money(c.fee_minor)}</span>
{/snippet}

{#snippet actions(c: Course)}
  <Button variant="ghost" size="sm" onclick={() => (detailId = c.id)}>عرض</Button>
  {#if c.status === 'draft'}
    <Button variant="success" size="sm" onclick={() => setStatus(c, 'registration_open')}>فتح التسجيل</Button>
    <Button variant="danger" size="sm" onclick={() => setStatus(c, 'cancelled')}>إلغاء</Button>
  {:else if c.status === 'registration_open'}
    <Button variant="primary" size="sm" onclick={() => setStatus(c, 'active')}>اعتماد وتفعيل</Button>
    <Button variant="danger" size="sm" onclick={() => setStatus(c, 'cancelled')}>إلغاء</Button>
  {:else if c.status === 'active'}
    <Button variant="ghost" size="sm" onclick={() => setStatus(c, 'completed')}>إكمال</Button>
    <Button variant="ghost" size="sm" onclick={() => setStatus(c, 'suspended')}>تعليق</Button>
  {:else if c.status === 'suspended'}
    <Button variant="success" size="sm" onclick={() => setStatus(c, 'active')}>إعادة تفعيل</Button>
    <Button variant="danger" size="sm" onclick={() => setStatus(c, 'cancelled')}>إلغاء</Button>
  {/if}
{/snippet}

<PageHeader
  title="الدورات"
  subtitle={`${courses.length} دورة — الرسوم بالوحدة الكبيرة تُحوَّل تلقائياً إلى الوحدة الصغيرة`}
>
  <Button variant="ghost" size="sm" onclick={load} loading={loading}>↻ تحديث</Button>
  <Button variant="primary" size="sm" onclick={() => (showForm = true)}>+ إنشاء دورة</Button>
</PageHeader>

{#if error}
  <div class="mb-4"><Alert kind="error" onClose={() => (error = '')}>{error}</Alert></div>
{/if}

<Card pad={false}>
  <div class="border-b px-5 py-4" style="border-color:var(--ec-border)">
    <div class="ec-toolbar" style="margin-bottom:0">
      <Tabs items={TABS} bind:value={statusFilter} />
      <input
        class="ec-input grow"
        type="search"
        placeholder="ابحث باسم الدورة…"
        bind:value={search}
        aria-label="بحث في الدورات"
      />
      <span class="ec-muted text-[13px]">{filtered.length} نتيجة</span>
    </div>
  </div>

  <div class="p-4">
    <DataTable
      columns={[
        { key: 'course_no', label: 'الرقم', num: true, width: '80px' },
        { key: 'name', label: 'اسم الدورة' },
        { key: 'subject_id', label: 'المادة', format: (v) => nameOf.subject(Number(v)) },
        { key: 'teacher_id', label: 'المدرس', format: (v) => nameOf.teacher(Number(v)) },
        { key: 'room_id', label: 'القاعة', format: (v) => nameOf.room(Number(v)) },
        { key: 'start_date', label: 'البداية', format: (v) => date(String(v)) },
        { key: 'fee_minor', label: 'الرسوم', num: true, render: feeCell },
        { key: 'status', label: 'الحالة', render: statusCell, width: '150px' }
      ]}
      rows={filtered}
      rowKey={(c) => c.id}
      loading={loading}
      emptyTitle="لا توجد دورات"
      emptyHint="أنشئ دورة، ثم سجّل الطلاب فيها من صفحة التسجيلات."
      emptyIcon="📚"
      actions={actions}
    />
  </div>
</Card>

<Modal
  open={showForm}
  title="إنشاء دورة جديدة"
  subtitle="تبدأ الدورة كمسودة ثم تُعتمد لفتح التسجيل."
  onClose={() => (showForm = false)}
  width="640px"
>
  <form class="space-y-4" onsubmit={create}>
    {#if formError}
      <Alert kind="error">{formError}</Alert>
    {/if}
    <Field label="اسم الدورة" required>
      <input class="ec-input" bind:value={name} required placeholder="دورة الرياضيات — الفصل الأول" />
    </Field>
    <div class="ec-form-grid">
      <Field label="المادة" required>
        <select class="ec-select" bind:value={subjectId}>
          {#each subjects as s (s.id)}
            <option value={s.id}>{s.name}</option>
          {/each}
        </select>
      </Field>
      <Field label="المدرس" required>
        <select class="ec-select" bind:value={teacherId}>
          {#each teachers as t (t.id)}
            <option value={t.id}>{t.full_name}</option>
          {/each}
        </select>
      </Field>
    </div>
    <div class="ec-form-grid">
      <Field label="القاعة" required>
        <select class="ec-select" bind:value={roomId}>
          {#each rooms as r (r.id)}
            <option value={r.id}>{r.name}</option>
          {/each}
        </select>
      </Field>
      <Field label="الرسوم" hint="بالعملة — تُحوَّل إلى الوحدة الصغيرة تلقائياً.">
        <input
          class="ec-input ec-num"
          type="number"
          min="0"
          step="0.01"
          bind:value={feeMajor}
          dir="ltr"
        />
      </Field>
    </div>
    <div class="ec-form-grid">
      <Field label="تاريخ البداية">
        <input class="ec-input" type="date" dir="ltr" bind:value={startDate} />
      </Field>
      <Field label="تاريخ النهاية">
        <input class="ec-input" type="date" dir="ltr" bind:value={endDate} />
      </Field>
    </div>
    <div class="ec-divider"></div>
    <div class="flex items-center justify-between">
      <h3 class="ec-card-title">المواعيد الأسبوعية (اختياري)</h3>
      <Button variant="soft" size="sm" onclick={addSchedRow}>+ موعد</Button>
    </div>
    {#each schedRows as r, i (i)}
      <div class="ec-form-grid rounded-lg p-3" style="background:var(--ec-surface-2)">
        <Field label="اليوم">
          <select class="ec-select" bind:value={r.day}>
            {#each Object.entries(DAYS) as [k, v]}<option value={Number(k)}>{v}</option>{/each}
          </select>
        </Field>
        <Field label="من"><input class="ec-input" type="time" bind:value={r.start} dir="ltr" /></Field>
        <Field label="إلى"><input class="ec-input" type="time" bind:value={r.end} dir="ltr" /></Field>
        <Field label="القاعة">
          <select class="ec-select" bind:value={r.room}>
            {#each rooms as rm (rm.id)}<option value={rm.id}>{rm.name}</option>{/each}
          </select>
        </Field>
        <Field label="المدرس">
          <select class="ec-select" bind:value={r.teacher}>
            {#each teachers as t (t.id)}<option value={t.id}>{t.full_name}</option>{/each}
          </select>
        </Field>
        <div class="flex items-end"><Button variant="ghost" size="sm" onclick={() => removeSchedRow(i)}>✕</Button></div>
      </div>
    {/each}
    {#if schedRows.length}
      <Button variant="soft" size="sm" onclick={checkScheduleClashes}>🔍 فحص تعارض المواعيد</Button>
      {#if scheduleClashes.length}
        <Alert kind="warn">
          {#each scheduleClashes as c, i (i)}<div>• {c.message} — {DAYS[c.weekday]} {c.start_time} ({c.with_course})</div>{/each}
        </Alert>
      {/if}
    {/if}

    <div class="flex justify-end gap-2">
      <Button variant="ghost" onclick={() => (showForm = false)}>إلغاء</Button>
      <Button type="submit" variant="primary" loading={saving} disabled={!subjects.length}>
        إنشاء الدورة
      </Button>
    </div>
    {#if !subjects.length || !teachers.length || !rooms.length}
      <Alert kind="warn">
        أضف أولاً: {!subjects.length ? 'مادة ' : ''}{!teachers.length ? 'مدرس ' : ''}{!rooms.length
          ? 'قاعة'
          : ''}
      </Alert>
    {/if}
  </form>
</Modal>

{#if detailId !== null}
  <EntityDetail kind="course" id={detailId} onClose={() => (detailId = null)} />
{/if}
