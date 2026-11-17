<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { goto } from '$app/navigation';
  import { sessionToken } from '$lib/stores/app';
  import {
    expensesApi,
    accountsApi,
    type AccountOption,
    type Expense,
    type ExpenseCategory
  } from '$lib/api/app';
  import { invoke } from '$lib/api';
  import { toast } from '$lib/toasts';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Card from '$lib/components/Card.svelte';
  import Button from '$lib/components/Button.svelte';
  import Alert from '$lib/components/Alert.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import Field from '$lib/components/Field.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import StatCard from '$lib/components/StatCard.svelte';
  import DataTable from '$lib/components/DataTable.svelte';
  import Tabs from '$lib/components/Tabs.svelte';
  import { count, date, money, monthLabel } from '$lib/format';

  let items = $state<Expense[]>([]);
  let categories = $state<ExpenseCategory[]>([]);
  let accounts = $state<AccountOption[]>([]);
  let loading = $state(true);
  let error = $state('');

  let statusFilter = $state('all');
  let search = $state('');

  let showForm = $state(false);
  let amountMajor = $state(0);
  let categoryId = $state(0);
  let beneficiary = $state('');
  let description = $state('');
  let method = $state('cash');
  let accountId = $state(0);
  let expenseDate = $state(new Date().toISOString().slice(0, 10));
  let saving = $state(false);
  let formError = $state('');
  let approving = $state(0);

  const TABS = [
    { id: 'all', label: 'الكل' },
    { id: 'draft', label: 'مسودة' },
    { id: 'approved', label: 'معتمد' },
    { id: 'paid', label: 'مدفوع' }
  ];

  const filtered = $derived(
    items.filter(
      (e) =>
        (statusFilter === 'all' || e.status === statusFilter) &&
        (search.trim() === '' ||
          e.beneficiary.includes(search.trim()) ||
          e.description.includes(search.trim()))
    )
  );

  const categoryName = (id: number) => categories.find((c) => c.id === id)?.name ?? `#${id}`;

  const totals = $derived({
    all: items.reduce((s, e) => s + e.amount_minor, 0),
    draft: items.filter((e) => e.status === 'draft').reduce((s, e) => s + e.amount_minor, 0),
    approved: items.filter((e) => e.status === 'approved').reduce((s, e) => s + e.amount_minor, 0),
    paid: items.filter((e) => e.status === 'paid').reduce((s, e) => s + e.amount_minor, 0),
    draftCount: items.filter((e) => e.status === 'draft').length
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
      const [e, c, a] = await Promise.all([
        expensesApi.list(token),
        expensesApi.categories(token),
        accountsApi.list(token)
      ]);
      items = e;
      categories = c;
      accounts = a;
      if (!categoryId && c.length) categoryId = c[0].id;
      if (!accountId && a.length) accountId = a[0].id;
    } catch (err) {
      error = (err as Error)?.message ?? 'تعذّر تحميل المصروفات';
    } finally {
      loading = false;
    }
  }

  async function create(submitEvent: SubmitEvent) {
    submitEvent.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    const amountMinor = Math.round(Number(amountMajor || 0) * 100);
    if (amountMinor <= 0) {
      toast.error('أدخل مبلغاً أكبر من صفر');
      return;
    }
    formError = '';
    saving = true;
    try {
      await invoke('expenses_create', {
        token,
        branch_id: 1,
        category_id: categoryId,
        expense_date: expenseDate,
        amount_minor: amountMinor,
        beneficiary: beneficiary.trim(),
        description: description.trim(),
        method,
        account_id: accountId
      });
      toast.ok(`تم إنشاء مصروف ${money(amountMinor)} كمسودة`);
      showForm = false;
      amountMajor = 0;
      beneficiary = '';
      description = '';
      await load();
    } catch (err) {
      formError = (err as Error)?.message ?? 'تعذّر إنشاء المصروف';
    } finally {
      saving = false;
    }
  }

  async function approve(id: number) {
    const token = get(sessionToken);
    if (!token) return;
    approving = id;
    try {
      await invoke('expenses_approve', { token, id });
      toast.ok(`تم اعتماد المصروف #${id}`);
      await load();
    } catch (err) {
      toast.error((err as Error)?.message ?? 'تعذّر الاعتماد');
    } finally {
      approving = 0;
    }
  }

  onMount(load);
</script>

{#snippet statusCell(e: Expense)}
  <Badge status={e.status} />
{/snippet}

{#snippet amountCell(e: Expense)}
  <span class="ec-num">{money(e.amount_minor)}</span>
{/snippet}

{#snippet methodCell(e: Expense)}
  {e.method === 'cash' ? 'نقدي' : e.method === 'card' ? 'بطاقة' : e.method === 'transfer' ? 'تحويل' : 'محفظة'}
{/snippet}

{#snippet actions(e: Expense)}
  {#if e.status === 'draft'}
    <Button variant="success" size="sm" loading={approving === e.id} onclick={() => approve(e.id)}>
      اعتماد
    </Button>
  {/if}
{/snippet}

<PageHeader
  title="المصروفات"
  subtitle="المسودة تُرحَّل بعد الاعتماد — المصروفات تدخل قائمة الدخل وترتيب أولوية السداد"
>
  <Button variant="ghost" size="sm" onclick={load} loading={loading}>↻ تحديث</Button>
  <Button variant="soft" size="sm" onclick={() => goto('/reports?code=rpt_expenses_by_category')}>
    📊 حسب البند
  </Button>
  <Button variant="primary" size="sm" onclick={() => (showForm = true)}>+ مصروف جديد</Button>
</PageHeader>

{#if error}
  <div class="mb-4"><Alert kind="error" onClose={() => (error = '')}>{error}</Alert></div>
{/if}

<div class="ec-kpi-row mb-4">
  <StatCard label="إجمالي المصروفات" value={money(totals.all)} icon="💸" accent="var(--ec-red)" />
  <StatCard label="مسودات بانتظار الاعتماد" value={money(totals.draft)} foot={`${count(totals.draftCount)} بند`} icon="⏳" accent="var(--ec-amber)" />
  <StatCard label="معتمد" value={money(totals.approved)} icon="✓" accent="var(--ec-blue)" />
  <StatCard label="مدفوع" value={money(totals.paid)} icon="💰" accent="var(--ec-teal)" />
</div>

<Card pad={false}>
  <div class="border-b px-5 py-4" style="border-color:var(--ec-border)">
    <div class="ec-toolbar" style="margin-bottom:0">
      <input
        class="ec-input grow"
        type="search"
        placeholder="ابحث بالمستفيد أو الوصف…"
        bind:value={search}
        aria-label="بحث في المصروفات"
      />
      <span class="ec-muted text-[13px]">{filtered.length} نتيجة</span>
    </div>
    <div class="mt-3"><Tabs items={TABS} bind:value={statusFilter} /></div>
  </div>

  <div class="p-4">
    <DataTable
      columns={[
        { key: 'id', label: '#', num: true, width: '70px' },
        { key: 'expense_date', label: 'التاريخ', format: (v) => date(String(v)) },
        { key: 'category_id', label: 'البند', format: (v) => categoryName(Number(v)) },
        { key: 'beneficiary', label: 'المستفيد', format: (v) => String(v || '—') },
        { key: 'description', label: 'الوصف', format: (v) => String(v || '—') },
        { key: 'method', label: 'الطريقة', render: methodCell },
        { key: 'amount_minor', label: 'المبلغ', num: true, render: amountCell },
        { key: 'status', label: 'الحالة', render: statusCell, width: '130px' }
      ]}
      rows={filtered}
      rowKey={(e) => e.id}
      loading={loading}
      emptyTitle="لا توجد مصروفات"
      emptyHint="أنشئ أول مصروف — يبقى مسودة حتى يُعتمد."
      emptyIcon="💸"
      actions={actions}
    />
  </div>
</Card>

<Modal
  open={showForm}
  title="مصروف جديد"
  subtitle="يُنشأ كمسودة ثم يُعتمد ليُرحَّل في دفتر الأستاذ."
  onClose={() => (showForm = false)}
  width="640px"
>
  <form class="space-y-4" onsubmit={create}>
    {#if formError}
      <Alert kind="error">{formError}</Alert>
    {/if}
    <div class="ec-form-grid">
      <Field label="المبلغ" required hint="بالعملة — يُحوَّل تلقائياً.">
        <input
          class="ec-input ec-num"
          type="number"
          min="0"
          step="0.01"
          bind:value={amountMajor}
          dir="ltr"
          required
          placeholder="0.00"
        />
      </Field>
      <Field label="التاريخ" required>
        <input class="ec-input" type="date" dir="ltr" bind:value={expenseDate} required />
      </Field>
    </div>
    <div class="ec-form-grid">
      <Field label="بند المصروف" required>
        <select class="ec-select" bind:value={categoryId}>
          {#each categories as c (c.id)}
            <option value={c.id}>{c.code} — {c.name}</option>
          {/each}
        </select>
      </Field>
      <Field label="طريقة الدفع" required>
        <select class="ec-select" bind:value={method}>
          <option value="cash">نقدي</option>
          <option value="card">بطاقة</option>
          <option value="transfer">تحويل</option>
          <option value="wallet">محفظة</option>
        </select>
      </Field>
    </div>
    <div class="ec-form-grid">
      <Field label="المستفيد">
        <input class="ec-input" bind:value={beneficiary} placeholder="اسم المورّد أو الموظف" />
      </Field>
      <Field label="الحساب المحاسبي">
        <select class="ec-select" bind:value={accountId}>
          {#each accounts as a (a.id)}
            <option value={a.id}>{a.code} — {a.name}</option>
          {/each}
        </select>
      </Field>
    </div>
    <Field label="الوصف">
      <textarea class="ec-textarea" bind:value={description} placeholder="تفصيل المصروف…"></textarea>
    </Field>

    {#if !categories.length}
      <Alert kind="warn">لا توجد بنود مصروفات مزروعة — تحقق من إعدادات قاعدة البيانات.</Alert>
    {/if}

    <div class="flex justify-end gap-2">
      <Button variant="ghost" onclick={() => (showForm = false)}>إلغاء</Button>
      <Button type="submit" variant="primary" loading={saving} disabled={!categories.length}>
        إنشاء المصروف
      </Button>
    </div>
  </form>
</Modal>
