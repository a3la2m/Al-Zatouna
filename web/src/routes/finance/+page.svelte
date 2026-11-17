<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { goto } from '$app/navigation';
  import { sessionToken } from '$lib/stores/app';
  import {
    enrollmentsApi,
    paymentsApi,
    paymentVouchersApi,
    accountsApi,
    type AccountOption,
    type Enrollment,
    type PaymentVoucherRow
  } from '$lib/api/app';
  import { peopleApi, type Student } from '$lib/api/people';
  import { toast } from '$lib/toasts';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Card from '$lib/components/Card.svelte';
  import Button from '$lib/components/Button.svelte';
  import Alert from '$lib/components/Alert.svelte';
  import Field from '$lib/components/Field.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import StatCard from '$lib/components/StatCard.svelte';
  import DataTable from '$lib/components/DataTable.svelte';
  import Tabs from '$lib/components/Tabs.svelte';
  import { count, date, money } from '$lib/format';

  let enrollments = $state<Enrollment[]>([]);
  let students = $state<Student[]>([]);
  let vouchers = $state<PaymentVoucherRow[]>([]);
  let accounts = $state<AccountOption[]>([]);
  let loading = $state(true);
  let error = $state('');

  let tab = $state('payment');

  // payment form (entered in major units)
  let enrollmentId = $state(0);
  let amountMajor = $state(0);
  let idemKey = $state('');
  let recording = $state(false);
  let lastResult = $state('');

  const TABS = [
    { id: 'payment', label: 'تسجيل دفعة' },
    { id: 'vouchers', label: 'سندات الصرف' }
  ];

  const studentName = (id: number) => students.find((s) => s.id === id)?.full_name ?? `#${id}`;
  const enrollment = $derived(enrollments.find((e) => e.id === enrollmentId) ?? null);

  const pendingVouchers = $derived(
    vouchers.filter((v) => v.status !== 'paid' && v.status !== 'cancelled')
  );
  const paidTotal = $derived(
    vouchers.filter((v) => v.status === 'paid').reduce((s, v) => s + v.amount_minor, 0)
  );

  async function load() {
    const token = get(sessionToken);
    if (!token) {
      loading = false;
      return;
    }
    loading = true;
    error = '';
    try {
      const [e, s, v, a] = await Promise.all([
        enrollmentsApi.list(token),
        peopleApi.students.list(token, 1, undefined, undefined, 500),
        paymentVouchersApi.list(token),
        accountsApi.list(token)
      ]);
      enrollments = e;
      students = s;
      vouchers = v;
      accounts = a;
      if (!enrollmentId && e.length) enrollmentId = e[0].id;
    } catch (err) {
      error = (err as Error)?.message ?? 'تعذّر تحميل البيانات المالية';
    } finally {
      loading = false;
    }
  }

  async function record(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    const amountMinor = Math.round(Number(amountMajor || 0) * 100);
    if (amountMinor <= 0) {
      toast.error('أدخل مبلغاً أكبر من صفر');
      return;
    }
    recording = true;
    lastResult = '';
    try {
      const key = idemKey.trim() || crypto.randomUUID();
      const r = await paymentsApi.record(token, {
        enrollment_id: enrollmentId,
        amount_minor: amountMinor,
        idempotency_key: key
      });
      lastResult = `تم إنشاء سند القبض #${r.voucher_id} بمبلغ ${money(r.total_minor)}`;
      toast.ok(lastResult);
      amountMajor = 0;
      idemKey = '';
      await load();
    } catch (err) {
      toast.error((err as Error)?.message ?? 'تعذّر تسجيل الدفعة');
    } finally {
      recording = false;
    }
  }

  onMount(load);
</script>

{#snippet voucherStatus(v: PaymentVoucherRow)}
  <Badge status={v.status} />
{/snippet}

{#snippet voucherAmount(v: PaymentVoucherRow)}
  <span class="ec-num">{money(v.amount_minor)}</span>
{/snippet}

<PageHeader
  title="المحرك المالي"
  subtitle="تسجيل الدفعات وإنشاء سندات القبض — كل عملية تُرحَّل في دفتر الأستاذ تلقائياً"
>
  <Button variant="ghost" size="sm" onclick={load} loading={loading}>↻ تحديث</Button>
  <Button variant="soft" size="sm" onclick={() => goto('/reports?code=rpt_receivables')}>
    📊 الذمم المدينة
  </Button>
  <Button variant="soft" size="sm" onclick={() => goto('/reports?code=rpt_income_statement')}>
    📊 قائمة الدخل
  </Button>
</PageHeader>

{#if error}
  <div class="mb-4"><Alert kind="error" onClose={() => (error = '')}>{error}</Alert></div>
{/if}

<div class="ec-kpi-row mb-4">
  <StatCard label="التسجيلات النشطة" value={count(enrollments.filter((e) => e.status === 'active').length)} icon="📝" accent="var(--ec-blue)" />
  <StatCard label="سندات صرف معلّقة" value={count(pendingVouchers.length)} icon="⏳" accent="var(--ec-amber)" />
  <StatCard label="مدفوع" value={money(paidTotal)} icon="✓" accent="var(--ec-teal)" />
  <StatCard label="حسابات قابلة للترحيل" value={count(accounts.length)} icon="⚖" accent="#8fae20" />
</div>

<div class="mb-5"><Tabs items={TABS} bind:value={tab} /></div>

{#if tab === 'payment'}
  <div class="ec-grid ec-cols-2">
    <Card title="تسجيل دفعة">
      <form class="space-y-4" onsubmit={record}>
        <Field label="التسجيل" required hint="الطالب والدورة وصافي الرسوم تظهر أدناه.">
          <select class="ec-select" bind:value={enrollmentId}>
            {#each enrollments as e (e.id)}
              <option value={e.id}>
                {studentName(e.student_id)} — {e.id} ({money(e.net_fee_minor)})
              </option>
            {/each}
          </select>
        </Field>

        {#if enrollment}
          <div
            class="rounded-xl px-4 py-3 text-sm"
            style="background:var(--ec-surface-2); border:1px solid var(--ec-border)"
          >
            <div class="flex justify-between">
              <span style="color:var(--ec-text-2)">صافي الرسوم</span>
              <span class="ec-num font-semibold">{money(enrollment.net_fee_minor)}</span>
            </div>
            <div class="mt-1 flex justify-between">
              <span style="color:var(--ec-text-2)">الحالة</span>
              <Badge status={enrollment.status} />
            </div>
          </div>
        {/if}

        <Field label="المبلغ" hint="بالعملة — يُحوَّل إلى الوحدة الصغيرة تلقائياً.">
          <input
            class="ec-input ec-num"
            type="number"
            min="0"
            step="0.01"
            bind:value={amountMajor}
            dir="ltr"
            placeholder="0.00"
            required
          />
        </Field>

        <div class="flex flex-wrap gap-2">
          {#each [50, 100, 200, 500, 1000] as q (q)}
            <button
              type="button"
              class="ec-btn ec-btn-ghost ec-btn-sm"
              onclick={() => (amountMajor = q)}
            >
              {q}
            </button>
          {/each}
        </div>

        <Field
          label="مفتاح منع التكرار"
          hint="يمنع إنشاء سند مكرر عند الضغط المتكرر — اتركه فارغاً لتوليد تلقائي."
        >
          <input class="ec-input" dir="ltr" bind:value={idemKey} placeholder="تلقائي" />
        </Field>

        <Button type="submit" variant="primary" block loading={recording} disabled={!enrollment}>
          تسجيل الدفعة وإنشاء السند
        </Button>

        {#if lastResult}
          <Alert kind="ok">{lastResult}</Alert>
        {/if}
      </form>
    </Card>

    <Card title="كيف تعمل الدفعة؟">
      <ol class="space-y-3 text-[13.5px]" style="color:var(--ec-text-2)">
        <li class="flex gap-3">
          <span class="ec-badge ec-badge-info no-dot">1</span>
          <span>يُخصم المبلغ من صافي رسوم التسجيل ويُحدَّث رصيد الطالب.</span>
        </li>
        <li class="flex gap-3">
          <span class="ec-badge ec-badge-info no-dot">2</span>
          <span>يُنشأ سند قبض برقم متسلسل وحالة «مرحّل».</span>
        </li>
        <li class="flex gap-3">
          <span class="ec-badge ec-badge-info no-dot">3</span>
          <span>يُنشأ قيد مزدوج في دفتر الأستاذ: مدين الصندوق / دائن الإيراد.</span>
        </li>
        <li class="flex gap-3">
          <span class="ec-badge ec-badge-info no-dot">4</span>
          <span>المفتاح الفريد يمنع التكرار لو تكرر الضغط على الزر.</span>
        </li>
      </ol>

      <div class="ec-divider"></div>

      <div class="flex flex-wrap gap-2">
        <Button variant="ghost" size="sm" onclick={() => goto('/cash')}>💰 الصندوق</Button>
        <Button variant="ghost" size="sm" onclick={() => goto('/enrollments')}>📝 التسجيلات</Button>
        <Button variant="ghost" size="sm" onclick={() => goto('/reports?code=rpt_collections')}>
          📊 التحصيل
        </Button>
      </div>
    </Card>
  </div>
{:else}
  <Card pad={false}>
    <div
      class="flex flex-wrap items-center justify-between gap-3 border-b px-5 py-4"
      style="border-color:var(--ec-border)"
    >
      <div>
        <h2 class="ec-card-title">سندات الصرف</h2>
        <p class="mt-0.5 text-[12.5px]" style="color:var(--ec-text-3)">
          {count(vouchers.length)} سند — {count(pendingVouchers.length)} بانتظار الاعتماد
        </p>
      </div>
      <Button variant="ghost" size="sm" onclick={() => goto('/reports?code=rpt_payment_vouchers')}>
        📊 تقرير السندات
      </Button>
    </div>

    <div class="p-4">
      <DataTable
        columns={[
          { key: 'voucher_no', label: 'الرقم', num: true, width: '90px' },
          { key: 'voucher_date', label: 'التاريخ', format: (v) => date(String(v)) },
          { key: 'beneficiary_name', label: 'المستفيد', format: (v) => String(v || '—') },
          {
            key: 'beneficiary_type',
            label: 'النوع',
            format: (v) =>
              v === 'teacher' ? 'مدرس' : v === 'student' ? 'طالب' : v === 'vendor' ? 'مورّد' : 'أخرى'
          },
          { key: 'source_type', label: 'المصدر', format: (v) => String(v || '—') },
          { key: 'method', label: 'طريقة الدفع', format: (v) => String(v) },
          { key: 'amount_minor', label: 'المبلغ', num: true, render: voucherAmount },
          { key: 'status', label: 'الحالة', render: voucherStatus, width: '150px' }
        ]}
        rows={vouchers}
        rowKey={(v) => v.id}
        loading={loading}
        emptyTitle="لا توجد سندات صرف"
        emptyHint="تُنشأ سندات الصرف من المصروفات والتسويات وطلبات الاسترداد."
        emptyIcon="🧾"
      />
    </div>
  </Card>
{/if}
