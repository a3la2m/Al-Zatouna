<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { goto } from '$app/navigation';
  import { sessionToken } from '$lib/stores/app';
  import {
    scheduleApi, sessionsApi, coursesApi,
    type ScheduleRule, type GeneratedSession, type Clash, type Course, type Session
  } from '$lib/api/app';
  import { peopleApi, type Room, type Teacher } from '$lib/api/people';
  import PrintPreview from '$lib/components/PrintPreview.svelte';
  import { toast } from '$lib/toasts';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Card from '$lib/components/Card.svelte';
  import Button from '$lib/components/Button.svelte';
  import Alert from '$lib/components/Alert.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import Field from '$lib/components/Field.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import DataTable from '$lib/components/DataTable.svelte';
  import Tabs from '$lib/components/Tabs.svelte';
  import { count, date, statusInfo } from '$lib/format';
  import type { Column } from '$lib/table';

  const DAYS: Record<number, string> = {
    1: 'الاثنين', 2: 'الثلاثاء', 3: 'الأربعاء', 4: 'الخميس',
    5: 'الجمعة', 6: 'السبت', 7: 'الأحد'
  };
  const DAY_IDS = [1, 2, 3, 4, 5, 6, 7];

  let rules = $state<ScheduleRule[]>([]);
  let sessions = $state<Session[]>([]);
  let courses = $state<Course[]>([]);
  let rooms = $state<Room[]>([]);
  let teachers = $state<Teacher[]>([]);
  let loading = $state(true);
  let error = $state('');
  let tab = $state('week');

  const TABS = [
    { id: 'week', label: 'الجدول الأسبوعي' },
    { id: 'rules', label: 'قواعد الجدول' },
    { id: 'sessions', label: 'الحصص' }
  ];

  // add-rule form
  let showAdd = $state(false);
  let rCourse = $state(0);
  let rDay = $state(1);
  let rStart = $state('10:00');
  let rEnd = $state('11:00');
  let rRoom = $state(0);
  let rTeacher = $state(0);
  let rFrom = $state('');
  let rTo = $state('');
  let savingRule = $state(false);
  let ruleError = $state('');
  let clashes = $state<Clash[]>([]);

  // generate form
  let printTarget = $state<{ type: string; id: number; title: string } | null>(null);
  let showGen = $state(false);
  let gCourse = $state(0);
  let gFrom = $state('');
  let gTo = $state('');
  let gPreview = $state<GeneratedSession[]>([]);
  let gLoading = $state(false);
  let genError = $state('');

  const courseName = (id: number) => courses.find((c) => c.id === id)?.name ?? `#${id}`;
  const grouped = $derived(
    DAY_IDS.map((d) => ({ day: d, rules: rules.filter((r) => r.weekday === d) }))
  );

  const ruleCols: Column<ScheduleRule>[] = [
    { key: 'course_name', label: 'الدورة' },
    { key: 'weekday', label: 'اليوم', format: (v) => DAYS[Number(v)] ?? '—' },
    { key: 'start_time', label: 'من', num: true },
    { key: 'end_time', label: 'إلى', num: true },
    { key: 'room_name', label: 'القاعة' },
    { key: 'teacher_name', label: 'المدرس' },
    { key: 'effective_from', label: 'من تاريخ', format: (v) => (v ? date(String(v)) : 'مستمر') },
    { key: 'effective_to', label: 'إلى تاريخ', format: (v) => (v ? date(String(v)) : 'مستمر') }
  ];
  const genCols: Column<GeneratedSession>[] = [
    { key: 'session_date', label: 'التاريخ', format: (v) => date(String(v)) },
    { key: 'start_time', label: 'من', num: true },
    { key: 'end_time', label: 'إلى', num: true },
    { key: 'room_name', label: 'القاعة' },
    { key: 'teacher_name', label: 'المدرس' },
    { key: 'seq_in_course', label: '#', num: true }
  ];
  const sessionCols: Column<Session>[] = [
    { key: 'course_id', label: 'الدورة', format: (v) => courseName(Number(v)) },
    { key: 'session_date', label: 'التاريخ', format: (v) => date(String(v)) },
    { key: 'start_time', label: 'من', num: true },
    { key: 'end_time', label: 'إلى', num: true },
    { key: 'status', label: 'الحالة' }
  ];

  async function load() {
    const token = get(sessionToken);
    if (!token) { loading = false; return; }
    loading = true; error = '';
    try {
      const [r, s, c, rm, t] = await Promise.all([
        scheduleApi.list(token), sessionsApi.list(token), coursesApi.list(token),
        peopleApi.rooms.list(token, 1), peopleApi.teachers.list(token, 1)
      ]);
      rules = r; sessions = s; courses = c; rooms = rm; teachers = t;
      if (!rCourse && c.length) rCourse = c[0].id;
      if (!gCourse && c.length) gCourse = c[0].id;
      if (!rRoom && rm.length) rRoom = rm[0].id;
      if (!rTeacher && t.length) rTeacher = t[0].id;
    } catch (e) {
      error = (e as any)?.message ?? 'تعذّر تحميل الجدول';
    } finally { loading = false; }
  }

  async function checkClashes() {
    const token = get(sessionToken);
    if (!token) return;
    try {
      clashes = await scheduleApi.check(token, {
        course_id: rCourse, weekday: rDay, start_time: rStart, end_time: rEnd,
        room_id: rRoom, teacher_id: rTeacher
      });
    } catch (e) { ruleError = (e as any)?.message ?? 'تعذّر فحص التعارض'; }
  }

  async function saveRule(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    ruleError = ''; savingRule = true;
    try {
      await scheduleApi.add(token, {
        course_id: rCourse, weekday: rDay, start_time: rStart, end_time: rEnd,
        room_id: rRoom, teacher_id: rTeacher, effective_from: rFrom, effective_to: rTo
      });
      toast.ok('تمت إضافة قاعدة الجدول');
      showAdd = false; clashes = [];
      await load();
    } catch (err) {
      ruleError = (err as any)?.message ?? 'تعذّر إضافة القاعدة';
    } finally { savingRule = false; }
  }

  async function removeRule(id: number) {
    if (!confirm('حذف قاعدة الجدول؟')) return;
    const token = get(sessionToken);
    if (!token) return;
    try {
      await scheduleApi.remove(token, id);
      toast.ok('تم حذف القاعدة');
      await load();
    } catch (e) { toast.error((e as any)?.message ?? 'تعذّر الحذف'); }
  }

  async function preview() {
    const token = get(sessionToken);
    if (!token) return;
    gLoading = true; genError = ''; gPreview = [];
    try {
      gPreview = await scheduleApi.generate(token, gCourse, gFrom, gTo, false);
      if (!gPreview.length) genError = 'لا توجد حصص جديدة في هذا النطاق (تحقق من القواعد والتواريخ)';
    } catch (e) { genError = (e as any)?.message ?? 'تعذّر المعاينة'; }
    finally { gLoading = false; }
  }

  async function commit() {
    const token = get(sessionToken);
    if (!token) return;
    gLoading = true;
    try {
      const made = await scheduleApi.generate(token, gCourse, gFrom, gTo, true);
      toast.ok(`تم إنشاء ${count(made.length)} حصة`);
      showGen = false; gPreview = [];
      await load();
    } catch (e) { genError = (e as any)?.message ?? 'تعذّر creating الحصص'; }
    finally { gLoading = false; }
  }

  onMount(load);
</script>

<PageHeader title="الجدول الأسبوعي والحصص" subtitle="عرّف مواعيد الدورات، تحقق من حجز القاعات والمدرسين، ثم ولّد الحصص">
  <Button variant="ghost" size="sm" onclick={load} loading={loading}>↻ تحديث</Button>
  <Button variant="primary" size="sm" onclick={() => (showGen = true)}>📅 توليد الحصص</Button>
  <Button variant="soft" size="sm" onclick={() => (showAdd = true)}>+ قاعدة جدول</Button>
</PageHeader>

{#if error}<div class="mb-4"><Alert kind="error" onClose={() => (error = '')}>{error}</Alert></div>{/if}

<div class="mb-5"><Tabs items={TABS} bind:value={tab} /></div>

{#if tab === 'week'}
  <Card title="الجدول الأسبوعي">
    {#if loading}
      <div class="ec-skel h-64 rounded-xl"></div>
    {:else if !rules.length}
      <div class="py-10 text-center text-sm" style="color:var(--ec-text-3)">
        لا توجد قواعد جدول بعد — أضف قاعدة لترى الجدول هنا.
      </div>
    {:else}
      <div class="space-y-3">
        {#each grouped as g (g.day)}
          {#if g.rules.length}
            <div class="overflow-hidden rounded-xl border" style="border-color:var(--ec-border)">
              <div class="flex items-center justify-between px-4 py-2 text-[13px] font-bold" style="background:var(--ec-surface-2)">
                <span>{DAYS[g.day]}</span>
                <span style="color:var(--ec-text-3)">{g.rules.length} حصة</span>
              </div>
              <div class="divide-y" style="border-color:var(--ec-border)">
                {#each g.rules as r (r.id)}
                  <div class="grid grid-cols-5 items-center gap-2 px-4 py-2.5 text-[13px]">
                    <span class="ec-num font-bold" style="color:var(--ec-blue)">{r.start_time}–{r.end_time}</span>
                    <span class="font-semibold">{r.course_name}</span>
                    <span>▦ {r.room_name}</span>
                    <span>👔 {r.teacher_name}</span>
                    <div class="flex justify-end">
                      <Button variant="ghost" size="sm" onclick={() => removeRule(r.id)}>حذف</Button>
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          {/if}
        {/each}
      </div>
    {/if}
  </Card>

  <Card title="إشغال القاعات" classes="mt-4" pad={false}>
    <div class="p-4">
      <div class="grid gap-3" style="grid-template-columns:repeat(auto-fill,minmax(230px,1fr))">
        {#each rooms as rm (rm.id)}
          {@const used = rules.filter((r) => r.room_id === rm.id)}
          <div class="rounded-xl border p-3" style="border-color:var(--ec-border)">
            <div class="flex items-center justify-between text-[13px] font-bold">
              <span>▦ {rm.name}</span>
              <div class="flex items-center gap-1">
                <span style="color:var(--ec-text-3)">{used.length} حجز</span>
                <Button variant="ghost" size="sm" onclick={() => (printTarget = { type: 'room_schedule', id: rm.id, title: `جدول القاعة: ${rm.name}` })}>🖨</Button>
              </div>
            </div>
            <div class="mt-2 space-y-1">
              {#each used as u (u.id)}
                <div class="text-[12px]" style="color:var(--ec-text-2)">{DAYS[u.weekday]} {u.start_time} — {u.course_name}</div>
              {/each}
              {#if !used.length}<div class="text-[12px]" style="color:var(--ec-text-3)">متاحة</div>{/if}
            </div>
          </div>
        {/each}
      </div>
    </div>
  </Card>

  <Card title="جداول المدرسين" classes="mt-4">
    <div class="flex flex-wrap gap-2">
      {#each teachers as t (t.id)}
        <Button variant="ghost" size="sm" onclick={() => (printTarget = { type: 'teacher_schedule', id: t.id, title: `جدول المدرس: ${t.full_name}` })}>
          🖨 {t.full_name}
        </Button>
      {/each}
      {#if !teachers.length}<span class="text-[13px]" style="color:var(--ec-text-3)">لا يوجد مدرسون</span>{/if}
    </div>
  </Card>
{:else if tab === 'rules'}
  <Card pad={false}>
    <div class="p-4">
      <DataTable columns={ruleCols} rows={rules} rowKey={(r) => r.id} loading={loading}
        emptyTitle="لا توجد قواعد" emptyHint="أضف قاعدة جدول لربط دورة بقاعة ومدرس وموعد أسبوعي." emptyIcon="🗓">
        {#snippet actions(r: ScheduleRule)}
          <Button variant="ghost" size="sm" onclick={() => removeRule(r.id)}>حذف</Button>
        {/snippet}
      </DataTable>
    </div>
  </Card>
{:else}
  <Card pad={false}>
    <div class="p-4">
      <DataTable columns={sessionCols} rows={sessions} rowKey={(s) => s.id} loading={loading}
        emptyTitle="لا توجد حصص" emptyHint="ولّد الحصص من الجدول الأسبوعي أو أنشئها يدوياً." emptyIcon="📚">
        {#snippet actions(s: Session)}
          <Button variant="ghost" size="sm" onclick={() => goto('/attendance')}>الحضور</Button>
        {/snippet}
      </DataTable>
    </div>
  </Card>
{/if}

<Modal open={showAdd} title="قاعدة جدول جديدة" subtitle="سيُفحص تعارض القاعة والمدرس قبل الحفظ." onClose={() => (showAdd = false)} width="680px">
  <form class="space-y-4" onsubmit={saveRule}>
    {#if ruleError}<Alert kind="error">{ruleError}</Alert>{/if}
    <div class="ec-form-grid">
      <Field label="الدورة" required>
        <select class="ec-select" bind:value={rCourse}>
          {#each courses as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
        </select>
      </Field>
      <Field label="اليوم" required>
        <select class="ec-select" bind:value={rDay}>
          {#each DAY_IDS as d}<option value={d}>{DAYS[d]}</option>{/each}
        </select>
      </Field>
    </div>
    <div class="ec-form-grid">
      <Field label="من" required><input class="ec-input" type="time" bind:value={rStart} dir="ltr" required /></Field>
      <Field label="إلى" required><input class="ec-input" type="time" bind:value={rEnd} dir="ltr" required /></Field>
    </div>
    <div class="ec-form-grid">
      <Field label="القاعة" required>
        <select class="ec-select" bind:value={rRoom}>
          {#each rooms as rm (rm.id)}<option value={rm.id}>{rm.name}</option>{/each}
        </select>
      </Field>
      <Field label="المدرس" required>
        <select class="ec-select" bind:value={rTeacher}>
          {#each teachers as t (t.id)}<option value={t.id}>{t.full_name}</option>{/each}
        </select>
      </Field>
    </div>
    <div class="ec-form-grid">
      <Field label="ساري من" hint="اتركه فارغاً للبدء الفوري"><input class="ec-input" type="date" bind:value={rFrom} dir="ltr" /></Field>
      <Field label="ساري إلى" hint="اتركه فارغاً لغير محدد"><input class="ec-input" type="date" bind:value={rTo} dir="ltr" /></Field>
    </div>

    <Button variant="soft" size="sm" onclick={checkClashes}>🔍 فحص التعارض</Button>
    {#if clashes.length}
      <Alert kind="warn">
        <div class="space-y-1">
          {#each clashes as c, i (i)}
            <div>• {c.message} (دورة {c.with_course}، {DAYS[c.weekday]} {c.start_time}–{c.end_time})</div>
          {/each}
        </div>
      </Alert>
    {:else if rCourse && rRoom && rTeacher}
      <Alert kind="ok">لا يوجد تعارض — يمكن حفظ القاعدة.</Alert>
    {/if}

    <div class="flex justify-end gap-2">
      <Button variant="ghost" onclick={() => (showAdd = false)}>إلغاء</Button>
      <Button type="submit" variant="primary" loading={savingRule}>حفظ القاعدة</Button>
    </div>
  </form>
</Modal>

<Modal open={showGen} title="توليد الحصص من الجدول" subtitle="عاين الحصص أولاً، ثم أنشئها — تتخطى الحصص الموجودة واليومات المحجوزة." onClose={() => (showGen = false)} width="860px">
  <div class="space-y-4">
    {#if genError}<Alert kind="error">{genError}</Alert>{/if}
    <div class="ec-form-grid">
      <Field label="الدورة" required>
        <select class="ec-select" bind:value={gCourse}>
          {#each courses as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
        </select>
      </Field>
      <Field label="من تاريخ" required><input class="ec-input" type="date" bind:value={gFrom} dir="ltr" required /></Field>
      <Field label="إلى تاريخ" required><input class="ec-input" type="date" bind:value={gTo} dir="ltr" required /></Field>
    </div>
    <div class="flex gap-2">
      <Button variant="soft" size="sm" loading={gLoading} onclick={preview}>🔍 معاينة</Button>
      <Button variant="primary" size="sm" loading={gLoading} disabled={!gPreview.length} onclick={commit}>✓ إنشاء الحصص</Button>
    </div>
    {#if gPreview.length}
      <DataTable columns={genCols} rows={gPreview} rowKey={(g) => `${g.session_date}${g.start_time}`} emptyTitle="" />
    {/if}
  </div>
</Modal>

{#if printTarget}
  <PrintPreview documentType={printTarget.type} id={printTarget.id} title={printTarget.title} onClose={() => (printTarget = null)} />
{/if}
