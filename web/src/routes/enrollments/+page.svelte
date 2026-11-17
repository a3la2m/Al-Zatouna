<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { sessionToken } from '$lib/stores/app';
  import {
    enrollmentsApi,
    coursesApi,
    type Course,
    type Enrollment
  } from '$lib/api/app';
  import { peopleApi, type Student } from '$lib/api/people';
  import { toast } from '$lib/toasts';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Card from '$lib/components/Card.svelte';
  import Button from '$lib/components/Button.svelte';
  import Alert from '$lib/components/Alert.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import Field from '$lib/components/Field.svelte';
  import Tabs from '$lib/components/Tabs.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import StatCard from '$lib/components/StatCard.svelte';
  import PrintPreview from '$lib/components/PrintPreview.svelte';
  import DataTable from '$lib/components/DataTable.svelte';
  import { count, date, money } from '$lib/format';

  let items = $state<Enrollment[]>([]);
  let students = $state<Student[]>([]);
  let courses = $state<Course[]>([]);
  let loading = $state(true);
  let error = $state('');

  let showForm = $state(false);
  let studentId = $state(0);
  let courseId = $state(0);
  let baseFeeMajor = $state(0);
  let discountMajor = $state(0);
  let waiverMajor = $state(0);
  let enrolledDate = $state(new Date().toISOString().slice(0, 10));
  let notes = $state('');

  // auto-fill the base fee from the selected course (minor -> major units)
  $effect(() => {
    const c = courses.find((x) => x.id === courseId);
    if (c) baseFeeMajor = Number((c.fee_minor / 100).toFixed(2));
  });
  let saving = $state(false);
  let formError = $state('');
  let printTarget = $state<number | null>(null);
  let lastEnrollment = $state<number | null>(null);

  let statusFilter = $state('all');
  let courseFilter = $state(0);
  let search = $state('');

  const TABS = [
    { id: 'all', label: 'الكل' },
    { id: 'active', label: 'نشط' },
    { id: 'completed', label: 'مكتمل' },
    { id: 'withdrawn', label: 'منسحب' },
    { id: 'frozen', label: 'مجمّد' },
    { id: 'cancelled', label: 'ملغى' }
  ];

  const studentName = (id: number) => students.find((s) => s.id === id)?.full_name ?? `#${id}`;
  const courseName = (id: number) => courses.find((c) => c.id === id)?.name ?? `#${id}`;

  const filtered = $derived(
    items.filter(
      (e) =>
        (statusFilter === 'all' || e.status === statusFilter) &&
        (courseFilter === 0 || e.course_id === courseFilter) &&
        (search.trim() === '' ||
          studentName(e.student_id).includes(search.trim()) ||
          courseName(e.course_id).includes(search.trim()))
    )
  );

  const byStatus = $derived({
    all: items.length,
    active: items.filter((e) => e.status === 'active').length,
    completed: items.filter((e) => e.status === 'completed').length,
    withdrawn: items.filter((e) => e.status === 'withdrawn').length,
    frozen: items.filter((e) => e.status === 'frozen').length,
    cancelled: items.filter((e) => e.status === 'cancelled').length
  });

  const totals = $derived({
    net: items.reduce((s, e) => s + e.net_fee_minor, 0),
    discount: items.reduce((s, e) => s + e.discount_minor, 0),
    waiver: items.reduce((s, e) => s + e.waiver_minor, 0)
  });

  async function load() {
    const token = get(sessionToken);
    if (!token) {
      loading = false;
      return;
    }
    loading = true;
    error = '';
    try {
      const [e, s, c] = await Promise.all([
        enrollmentsApi.list(token),
        peopleApi.students.list(token, 1, undefined, undefined, 500),
        coursesApi.list(token)
      ]);
      items = e;
      students = s;
      courses = c;
    } catch (err) {
      error = (err as Error)?.message ?? 'تعذّر تحميل التسجيلات';
    } finally {
      loading = false;
    }
  }

  async function freeze(e: Enrollment) {
    const token = get(sessionToken);
    if (!token) return;
    const reason = prompt('سبب التجميد؟', '') ?? '';
    if (!reason) return;
    try {
      // no dedicated command yet — surface a clear message instead of failing silently
      toast.info('تغيير حالة التسجيل سيُضاف في تحديث لاحق — السبب مسجّل: ' + reason);
    } catch (err) {
      toast.error((err as Error)?.message ?? 'تعذّر التنفيذ');
    }
  }

  async function enroll(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    if (!studentId || !courseId) {
      formError = 'اختر طالباً ودورة';
      return;
    }
    formError = '';
    saving = true;
    try {
      await enrollmentsApi.enroll(token, {
        student_id: studentId,
        course_id: courseId,
        base_fee_minor: Math.round(Number(baseFeeMajor || 0) * 100),
        discount_minor: Math.round(Number(discountMajor || 0) * 100),
        waiver_minor: Math.round(Number(waiverMajor || 0) * 100),
        enrolled_date: enrolledDate,
        notes
      });
      const created = await enrollmentsApi.list(token);
      const newest = created[0];
      if (newest) lastEnrollment = newest.id;
      toast.ok('تم تسجيل الطالب — أنشئ سند قبض وسند دفع بانتظار الطباعة');
      showForm = false;
      baseFeeMajor = 0; discountMajor = 0; waiverMajor = 0; notes = '';
      await load();
    } catch (err) {
      formError = (err as any)?.message ?? 'تعذّر التسجيل';
    } finally {
      saving = false;
    }
  }

  onMount(load);
</script>

{#snippet statusCell(e: Enrollment)}
  <Badge status={e.status} />
{/snippet}

{#snippet rowActions(e: Enrollment)}
  <Button variant="ghost" size="sm" onclick={() => (printTarget = e.id)}>🖨 إيصال</Button>
{/snippet}

{#snippet moneyCell(e: Enrollment)}
  <span class="ec-num">{money(e.net_fee_minor)}</span>
{/snippet}

<PageHeader
  title="التسجيلات"
  subtitle={`${items.length} تسجيل — صافي الرسوم بعد الخصم والإعفاء والرصيد المستخدم`}
>
  <Button variant="ghost" size="sm" onclick={load} loading={loading}>↻ تحديث</Button>
  <Button variant="primary" size="sm" onclick={() => (showForm = true)}>+ تسجيل طالب في دورة</Button>
</PageHeader>

{#if error}
  <div class="mb-4"><Alert kind="error" onClose={() => (error = '')}>{error}</Alert></div>
{/if}

<div class="ec-kpi-row mb-4">
  <StatCard label="إجمالي التسجيلات" value={count(items.length)} icon="📝" accent="var(--ec-blue)" />
  <StatCard
    label="التسجيلات النشطة"
    value={count(byStatus.active)}
    icon="✓"
    accent="var(--ec-teal)"
    onclick={() => (statusFilter = 'active')}
  />
  <StatCard label="صافي الرسوم" value={money(totals.net)} icon="₪" accent="#8fae20" />
  <StatCard label="إجمالي الخصومات" value={money(totals.discount)} icon="٪" accent="var(--ec-amber)" />
  <StatCard label="الإعفاءات" value={money(totals.waiver)} icon="🎁" accent="#5d7d14" />
</div>

<Card pad={false}>
  <div class="border-b px-5 py-4" style="border-color:var(--ec-border)">
    <div class="ec-toolbar" style="margin-bottom:0">
      <select
        class="ec-select"
        style="max-width:250px"
        value={courseFilter}
        onchange={(e) => (courseFilter = Number(e.currentTarget.value))}
        aria-label="تصفية حسب الدورة"
      >
        <option value={0}>كل الدورات</option>
        {#each courses as c (c.id)}
          <option value={c.id}>{c.name}</option>
        {/each}
      </select>
      <input
        class="ec-input grow"
        type="search"
        placeholder="ابحث باسم الطالب أو الدورة…"
        bind:value={search}
        aria-label="بحث في التسجيلات"
      />
      <span class="ec-muted text-[13px]">{filtered.length} نتيجة</span>
    </div>
    <div class="mt-3"><Tabs items={TABS} bind:value={statusFilter} /></div>
  </div>

  <div class="p-4">
    <DataTable
      columns={[
        { key: 'id', label: '#', num: true, width: '70px' },
        { key: 'student_id', label: 'الطالب', format: (v) => studentName(Number(v)) },
        { key: 'course_id', label: 'الدورة', format: (v) => courseName(Number(v)) },
        { key: 'enrolled_date', label: 'تاريخ التسجيل', format: (v) => date(String(v)) },
        { key: 'base_fee_minor', label: 'الرسوم', num: true, format: (v) => money(Number(v)) },
        { key: 'discount_minor', label: 'الخصم', num: true, format: (v) => money(Number(v)) },
        { key: 'waiver_minor', label: 'الإعفاء', num: true, format: (v) => money(Number(v)) },
        { key: 'credit_applied_minor', label: 'الرصيد', num: true, format: (v) => money(Number(v)) },
        { key: 'net_fee_minor', label: 'الصافي', num: true, render: moneyCell },
        { key: 'status', label: 'الحالة', render: statusCell, width: '130px' }
      ]}
      rows={filtered}
      rowKey={(e) => e.id}
      loading={loading}
      emptyTitle="لا توجد تسجيلات"
      emptyHint="سجّل الطلاب في الدورات من هنا لتفعيل الرسوم والحضور."
      emptyIcon="📝"
      actions={rowActions}
    />
  </div>
</Card>

<Modal
  open={showForm}
  title="تسجيل طالب في دورة"
  subtitle="ربط الطالب بدورة — الرسوم والخصم والإعفاء بالعملة (تُحوَّل تلقائياً)."
  onClose={() => (showForm = false)}
  width="620px"
>
  <form class="space-y-4" onsubmit={enroll}>
    {#if formError}
      <Alert kind="error">{formError}</Alert>
    {/if}
    <div class="ec-form-grid">
      <Field label="الطالب" required>
        <select class="ec-select" bind:value={studentId}>
          <option value={0}>— اختر طالباً —</option>
          {#each students as st (st.id)}
            <option value={st.id}>{st.full_name} (#{st.student_no})</option>
          {/each}
        </select>
      </Field>
      <Field label="الدورة" required>
        <select class="ec-select" bind:value={courseId}>
          <option value={0}>— اختر دورة —</option>
          {#each courses as c (c.id)}
            <option value={c.id}>{c.name} ({c.status})</option>
          {/each}
        </select>
      </Field>
    </div>
    <div class="ec-form-grid">
      <Field label="الرسوم (الأساس)" hint="بالعملة">
        <input class="ec-input ec-num" type="number" min="0" step="0.01" bind:value={baseFeeMajor} dir="ltr" />
      </Field>
      <Field label="الخصم" hint="بالعملة">
        <input class="ec-input ec-num" type="number" min="0" step="0.01" bind:value={discountMajor} dir="ltr" />
      </Field>
    </div>
    <div class="ec-form-grid">
      <Field label="الإعفاء" hint="بالعملة">
        <input class="ec-input ec-num" type="number" min="0" step="0.01" bind:value={waiverMajor} dir="ltr" />
      </Field>
      <Field label="تاريخ التسجيل" required>
        <input class="ec-input" type="date" dir="ltr" bind:value={enrolledDate} required />
      </Field>
    </div>
    <Field label="ملاحظات">
      <textarea class="ec-textarea" bind:value={notes}></textarea>
    </Field>
    <div class="flex justify-end gap-2">
      <Button variant="ghost" onclick={() => (showForm = false)}>إلغاء</Button>
      <Button type="submit" variant="primary" loading={saving} disabled={!studentId || !courseId}>تسجيل</Button>
    </div>
  </form>
</Modal>

{#if printTarget}
  <PrintPreview documentType="enrollment" id={printTarget} title="إيصال تسجيل الطالب" onClose={() => (printTarget = null)} />
{/if}
