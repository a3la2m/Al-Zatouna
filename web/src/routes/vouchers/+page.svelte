<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { goto } from '$app/navigation';
  import { sessionToken } from '$lib/stores/app';
  import { invoke } from '$lib/api';
  import {
    paymentVouchersApi,
    type PaymentVoucherRow
  } from '$lib/api/app';
  import { toast } from '$lib/toasts';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Card from '$lib/components/Card.svelte';
  import Button from '$lib/components/Button.svelte';
  import Alert from '$lib/components/Alert.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import StatCard from '$lib/components/StatCard.svelte';
  import DataTable from '$lib/components/DataTable.svelte';
  import Tabs from '$lib/components/Tabs.svelte';
  import PrintPreview from '$lib/components/PrintPreview.svelte';
  import { count, date, money } from '$lib/format';

  type ReceiptVoucher = {
    id: number;
    voucher_no: number;
    amount_minor: number;
    status: string;
    voucher_date: string;
  };

  let receipts = $state<ReceiptVoucher[]>([]);
  let payments = $state<PaymentVoucherRow[]>([]);
  let loading = $state(true);
  let error = $state('');

  let tab = $state('receipts');
  let approving = $state(0);
  let printTarget = $state<{ type: string; id: number } | null>(null);

  const TABS = [
    { id: 'receipts', label: 'سندات القبض' },
    { id: 'payments', label: 'سندات الصرف' }
  ];

  const postedTotal = $derived(
    receipts.filter((r) => r.status === 'posted').reduce((s, r) => s + r.amount_minor, 0)
  );
  const pendingCount = $derived(
    receipts.filter((r) => r.status === 'draft').length +
      payments.filter((p) => p.status === 'draft' || p.status === 'pending_approval').length
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
      const [r, p] = await Promise.all([
        invoke<ReceiptVoucher[]>('vouchers_list_receipts', { token }),
        paymentVouchersApi.list(token)
      ]);
      receipts = r;
      payments = p;
    } catch (e) {
      error = (e as Error)?.message ?? 'تعذّر تحميل السندات';
    } finally {
      loading = false;
    }
  }

  async function approve(id: number) {
    const token = get(sessionToken);
    if (!token) return;
    approving = id;
    try {
      await invoke('payment_vouchers_approve', { token, id });
      toast.ok(`تم اعتماد السند #${id}`);
      await load();
    } catch (err) {
      toast.error((err as Error)?.message ?? 'تعذّر الاعتماد');
    } finally {
      approving = 0;
    }
  }

  async function pay(id: number) {
    const token = get(sessionToken);
    if (!token) return;
    approving = id;
    try {
      await invoke('payment_vouchers_pay', { token, id, payable_account: 0, cash_account: 0 });
      toast.ok(`تم دفع السند #${id}`);
      await load();
    } catch (err) {
      toast.error((err as Error)?.message ?? 'تعذّر الدفع — قد تحتاج إلى حسابات صحيحة');
    } finally {
      approving = 0;
    }
  }

  onMount(load);
</script>

{#snippet receiptPrint(r: ReceiptVoucher)}
  <Button variant="ghost" size="sm" onclick={() => (printTarget = { type: 'receipt', id: r.id })}>🖨 طباعة</Button>
{/snippet}

{#snippet receiptStatus(r: ReceiptVoucher)}
  <Badge status={r.status} />
{/snippet}

{#snippet receiptAmount(r: ReceiptVoucher)}
  <span class="ec-num">{money(r.amount_minor)}</span>
{/snippet}

{#snippet paymentStatus(p: PaymentVoucherRow)}
  <Badge status={p.status} />
{/snippet}

{#snippet paymentAmount(p: PaymentVoucherRow)}
  <span class="ec-num">{money(p.amount_minor)}</span>
{/snippet}

{#snippet paymentActions(p: PaymentVoucherRow)}
  <Button variant="ghost" size="sm" onclick={() => (printTarget = { type: 'payment', id: p.id })}>🖨 طباعة</Button>
  {#if p.status === 'draft' || p.status === 'pending_approval'}
    <Button variant="success" size="sm" loading={approving === p.id} onclick={() => approve(p.id)}>
      اعتماد
    </Button>
  {/if}
  {#if p.status === 'approved'}
    <Button variant="primary" size="sm" loading={approving === p.id} onclick={() => pay(p.id)}>
      دفع
    </Button>
  {/if}
{/snippet}

<PageHeader
  title="السندات"
  subtitle="سندات القبض والصرف — الاعتماد والدفع يمران بنظام «منشئ/مراجع»"
>
  <Button variant="ghost" size="sm" onclick={load} loading={loading}>↻ تحديث</Button>
  <Button variant="soft" size="sm" onclick={() => goto('/finance')}>⚖ تسجيل دفعة</Button>
</PageHeader>

{#if error}
  <div class="mb-4"><Alert kind="error" onClose={() => (error = '')}>{error}</Alert></div>
{/if}

<div class="ec-kpi-row mb-4">
  <StatCard label="سندات القبض" value={count(receipts.length)} icon="🧾" accent="var(--ec-blue)" />
  <StatCard label="المحصّل (مرحّل)" value={money(postedTotal)} icon="💰" accent="var(--ec-teal)" />
  <StatCard label="سندات الصرف" value={count(payments.length)} icon="🧾" accent="#8fae20" />
  <StatCard
    label="بانتظار المعالجة"
    value={count(pendingCount)}
    icon="⏳"
    accent={pendingCount > 0 ? 'var(--ec-amber)' : 'var(--ec-teal)'}
  />
</div>

<div class="mb-5"><Tabs items={TABS} bind:value={tab} /></div>

{#if tab === 'receipts'}
  <Card pad={false}>
    <div class="border-b px-5 py-4" style="border-color:var(--ec-border)">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 class="ec-card-title">سندات القبض</h2>
          <p class="mt-0.5 text-[12.5px]" style="color:var(--ec-text-3)">
            {count(receipts.length)} سند — إجمالي المرحّل {money(postedTotal)}
          </p>
        </div>
        <Button
          variant="ghost"
          size="sm"
          onclick={() => goto('/reports?code=rpt_receipt_vouchers')}
        >
          📊 تقرير سندات القبض
        </Button>
      </div>
    </div>

    <div class="p-4">
      <DataTable
        columns={[
          { key: 'voucher_no', label: 'الرقم', num: true, width: '100px' },
          { key: 'voucher_date', label: 'التاريخ', format: (v) => date(String(v)) },
          { key: 'amount_minor', label: 'المبلغ', num: true, render: receiptAmount },
          { key: 'status', label: 'الحالة', render: receiptStatus, width: '150px' },
          { key: 'id', label: 'المعرّف', num: true, width: '80px' }
        ]}
        rows={receipts}
        rowKey={(r) => r.id}
        loading={loading}
        emptyTitle="لا توجد سندات قبض"
        emptyHint="تُنشأ سندات القبض تلقائياً عند تسجيل الدفعات."
        emptyIcon="🧾"
        actions={receiptPrint}
      />
    </div>
  </Card>
{:else}
  <Card pad={false}>
    <div class="border-b px-5 py-4" style="border-color:var(--ec-border)">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 class="ec-card-title">سندات الصرف</h2>
          <p class="mt-0.5 text-[12.5px]" style="color:var(--ec-text-3)">
            {count(payments.length)} سند — {count(payments.filter((p) => p.status === 'approved').length)}
            معتمد جاهز للدفع
          </p>
        </div>
        <Button
          variant="ghost"
          size="sm"
          onclick={() => goto('/reports?code=rpt_payment_vouchers')}
        >
          📊 تقرير سندات الصرف
        </Button>
      </div>
    </div>

    <div class="p-4">
      <DataTable
        columns={[
          { key: 'voucher_no', label: 'الرقم', num: true, width: '100px' },
          { key: 'voucher_date', label: 'التاريخ', format: (v) => date(String(v)) },
          { key: 'beneficiary_name', label: 'المستفيد', format: (v) => String(v || '—') },
          {
            key: 'beneficiary_type',
            label: 'النوع',
            format: (v) =>
              v === 'teacher' ? 'مدرس' : v === 'student' ? 'طالب' : v === 'vendor' ? 'مورّد' : 'أخرى'
          },
          { key: 'source_type', label: 'المصدر', format: (v) => String(v || '—') },
          { key: 'amount_minor', label: 'المبلغ', num: true, render: paymentAmount },
          { key: 'status', label: 'الحالة', render: paymentStatus, width: '150px' }
        ]}
        rows={payments}
        rowKey={(p) => p.id}
        loading={loading}
        emptyTitle="لا توجد سندات صرف"
        emptyHint="تُنشأ من المصروفات والتسويات وطلبات الاسترداد."
        emptyIcon="🧾"
        actions={paymentActions}
      />
    </div>
  </Card>
{/if}

{#if printTarget}
  <PrintPreview documentType={printTarget.type} id={printTarget.id} onClose={() => (printTarget = null)} />
{/if}
