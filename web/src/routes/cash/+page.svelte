<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { get } from 'svelte/store';
  import { sessionToken } from '$lib/stores/app';
  import { invoke } from '$lib/api';
  import {
    cashApi,
    accountsApi,
    expensesApi,
    paymentsApi,
    type AccountOption,
    type CashMovementRow,
    type CashRegisterRow,
    type CashRegisterSummary
  } from '$lib/api/app';
  import { toast } from '$lib/toasts';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Card from '$lib/components/Card.svelte';
  import StatCard from '$lib/components/StatCard.svelte';
  import Button from '$lib/components/Button.svelte';
  import Alert from '$lib/components/Alert.svelte';
  import Field from '$lib/components/Field.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import DataTable from '$lib/components/DataTable.svelte';
  import { KIND_LABELS, count, dateTime, directionLabel, money } from '$lib/format';
  import { peopleApi, type Teacher } from '$lib/api/people';
  import Modal from '$lib/components/Modal.svelte';
  import type { Column } from '$lib/table';

  let registers = $state<CashRegisterRow[]>([]);
  let registerId = $state(0);
  let summary = $state<CashRegisterSummary | null>(null);
  let movements = $state<CashMovementRow[]>([]);
  let accounts = $state<AccountOption[]>([]);
  let loading = $state(true);
  let error = $state('');

  // movement form — entered in major units, stored in minor units
  let direction = $state<'in' | 'out'>('in');
  let kind = $state('receipt');
  let amountMajor = $state(0);
  let note = $state('');
  let recording = $state(false);

  // closing form
  let countedMajor = $state(0);
  let openingMajor = $state(0);
  let cashAccountId = $state(0);
  let varianceAccountId = $state(0);
  let closing = $state(false);
  let showClose = $state(false);

  // teacher expense (linked to the teacher's payable file)
  let showTeacherExpense = $state(false);
  let teachers = $state<Teacher[]>([]);
  let expTeacherId = $state(0);
  let expCategoryId = $state(0);
  let expCategories = $state<{ id: number; code: string; name: string }[]>([]);
  let expAmountMajor = $state(0);
  let expDate = $state(new Date().toISOString().slice(0, 10));
  let expDesc = $state('');
  let expSaving = $state(false);
  let expError = $state('');

  // other services (subject summary, printing, book…) sold as receipt vouchers
  let showService = $state(false);
  let svcPayer = $state('');
  let svcDesc = $state('');
  let svcAmountMajor = $state(0);
  let svcMethod = $state('cash');
  let svcSaving = $state(false);
  let svcError = $state('');
  const SERVICE_PRESETS = ['ملخص مادة', 'طباعة', 'كتاب', 'تصوير', 'دورة مكثفة', 'أخرى'];

  const register = $derived(registers.find((r) => r.id === registerId) ?? null);
  const balance = $derived(summary?.balance_minor ?? 0);
  const amountMinor = $derived(Math.round(Number(amountMajor || 0) * 100));
  const previewBalance = $derived(
    direction === 'in' ? balance + amountMinor : balance - amountMinor
  );
  const overdrawn = $derived(direction === 'out' && amountMinor > balance);
  const varianceMinor = $derived(
    Math.round(Number(countedMajor || 0) * 100) - balance
  );

  const REPORTS = [
    { code: 'rpt_cash_register', title: 'أرصدة الصندوق', icon: '🏦' },
    { code: 'rpt_daily_cash_flow', title: 'التدفق النقدي اليومي', icon: '📉' },
    { code: 'rpt_cash_variance', title: 'فروقات الإغلاق', icon: '⚖' },
    { code: 'rpt_receipt_vouchers', title: 'سندات القبض', icon: '🧾' },
    { code: 'rpt_payment_vouchers', title: 'سندات الصرف', icon: '🧾' },
    { code: 'rpt_expenses', title: 'المصروفات', icon: '💸' }
  ];

  async function load() {
    const token = get(sessionToken);
    if (!token) {
      loading = false;
      return;
    }
    loading = true;
    error = '';
    try {
      const [regs, accs] = await Promise.all([cashApi.registers(token), accountsApi.list(token)]);
      registers = regs;
      accounts = accs;
      if (!cashAccountId && accs.length) {
        cashAccountId = accs.find((a) => a.code === '1000')?.id ?? accs[0].id;
        varianceAccountId = accs.find((a) => a.code === '7000')?.id ?? accs[1]?.id ?? accs[0].id;
      }
      if (!registerId && regs.length) registerId = regs[0].id;
      if (registerId) await refresh(registerId);
    } catch (e) {
      error = (e as Error)?.message ?? 'تعذّر تحميل الصندوق';
    } finally {
      loading = false;
    }
  }

  async function refresh(id: number) {
    const token = get(sessionToken);
    if (!token || !id) return;
    try {
      const [s, m] = await Promise.all([cashApi.summary(token, id), cashApi.movements(token, id, 300)]);
      summary = s;
      movements = m;
      if (!openingMajor) openingMajor = s.opening_balance_minor / 100;
      if (!countedMajor) countedMajor = s.balance_minor / 100;
    } catch (e) {
      error = (e as Error)?.message ?? 'تعذّر تحميل ملخّص الصندوق';
    }
  }

  async function selectRegister(id: number) {
    registerId = id;
    summary = null;
    movements = [];
    await refresh(id);
  }

  async function openRegister() {
    const token = get(sessionToken);
    if (!token || !register) return;
    try {
      await cashApi.open(token, register.branch_id, register.code);
      toast.ok('تم فتح الصندوق');
      await load();
    } catch (e) {
      toast.error((e as Error)?.message ?? 'تعذّر فتح الصندوق');
    }
  }

  async function record(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token || !registerId) return;
    if (amountMinor <= 0) {
      toast.error('أدخل مبلغاً أكبر من صفر');
      return;
    }
    if (overdrawn) {
      toast.error('المبلغ أكبر من رصيد الصندوق');
      return;
    }
    recording = true;
    try {
      const res = await cashApi.record(token, {
        register_id: registerId,
        kind,
        direction,
        amount_minor: amountMinor,
        ref_type: note ? `manual:${note.slice(0, 40)}` : 'manual'
      });
      toast.ok(
        `تم ${directionLabel(direction)} ${money(amountMinor)} — الرصيد الجديد ${money(res.balance_after_minor)}`
      );
      amountMajor = 0;
      note = '';
      await refresh(registerId);
    } catch (err) {
      toast.error((err as Error)?.message ?? 'تعذّر تسجيل الحركة');
    } finally {
      recording = false;
    }
  }

  async function closeRegister(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token || !registerId) return;
    closing = true;
    try {
      await cashApi.close(token, {
        register_id: registerId,
        opening_balance_minor: Math.round(Number(openingMajor || 0) * 100),
        counted_balance_minor: Math.round(Number(countedMajor || 0) * 100),
        cash_account: cashAccountId,
        cash_variance_account: varianceAccountId
      });
      toast.ok(`تم إغلاق الصندوق — الفرق ${money(varianceMinor)}`);
      showClose = false;
      await refresh(registerId);
    } catch (err) {
      toast.error((err as Error)?.message ?? 'تعذّر إغلاق الصندوق');
    } finally {
      closing = false;
    }
  }

  const movementCols: Column<CashMovementRow>[] = [
    { key: 'occurred_at', label: 'التاريخ', format: (v) => dateTime(String(v)) },
    {
      key: 'kind',
      label: 'النوع',
      format: (v) => KIND_LABELS[String(v)] ?? String(v)
    },
    {
      key: 'direction',
      label: 'الاتجاه',
      format: (v) => directionLabel(String(v))
    },
    {
      key: 'amount_minor',
      label: 'المبلغ',
      num: true,
      format: (v) => money(Number(v))
    },
    {
      key: 'balance_after_minor',
      label: 'الرصيد بعد',
      num: true,
      format: (v) => money(Number(v))
    },
    { key: 'ref_type', label: 'المرجع', format: (v) => String(v || '—') }
  ];

  async function openTeacherExpense() {
    showTeacherExpense = true;
    expError = '';
    const token = get(sessionToken);
    if (!token) return;
    try {
      const [t, c] = await Promise.all([
        peopleApi.teachers.list(token, 1),
        expensesApi.categories(token)
      ]);
      teachers = t;
      expCategories = c as any;
      if (!expTeacherId && t.length) expTeacherId = t[0].id;
      if (!expCategoryId && c.length) expCategoryId = c[0].id;
    } catch (e) { expError = (e as any)?.message ?? 'تعذّر التحميل'; }
  }

  async function saveTeacherExpense(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    expError = '';
    expSaving = true;
    try {
      await invoke('expenses_create_teacher', {
        token, branch_id: 1, category_id: expCategoryId, teacher_id: expTeacherId,
        expense_date: expDate, amount_minor: Math.round(Number(expAmountMajor || 0) * 100),
        description: expDesc || 'صرف للمدرس', method: 'cash',
        account_id: accounts.find((a) => a.code === '5000')?.id ?? accounts[0]?.id ?? 0
      });
      toast.ok('تم صرف المبلغ وربطه بملف المدرس');
      showTeacherExpense = false;
      expAmountMajor = 0; expDesc = '';
      await load();
    } catch (err) { expError = (err as any)?.message ?? 'تعذّر الصرف'; }
    finally { expSaving = false; }
  }

  function openService() {
    showService = true;
    svcError = '';
    svcPayer = '';
    svcDesc = '';
    svcAmountMajor = 0;
    svcMethod = 'cash';
  }

  function pickService(name: string) {
    svcDesc = name === 'أخرى' ? '' : name;
  }

  async function saveService(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    svcError = '';
    const amount = Math.round(Number(svcAmountMajor || 0) * 100);
    if (amount <= 0) {
      svcError = 'أدخل مبلغاً أكبر من صفر';
      return;
    }
    if (!svcPayer.trim() || !svcDesc.trim()) {
      svcError = 'اسم الدافع ووصف الخدمة مطلوبان';
      return;
    }
    svcSaving = true;
    try {
      const key = `svc-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
      const id = await paymentsApi.recordService(token, {
        payer_name: svcPayer.trim(),
        description: svcDesc.trim(),
        amount_minor: amount,
        method: svcMethod,
        idempotency_key: key
      });
      toast.ok(`تم تسجيل سند قبض خدمة رقم ${id} بمبلغ ${money(amount)}`);
      showService = false;
      await refresh(registerId);
    } catch (err) {
      svcError = (err as any)?.message ?? 'تعذّر تسجيل بيع الخدمة';
    } finally {
      svcSaving = false;
    }
  }

  onMount(load);
</script>

<PageHeader
  title="الصندوق"
  subtitle="الأرصدة تُحسب تلقائياً من كل حركة — الرصيد المعروض هو آخر رصيد مسجّل وليس أعلى رصيد"
>
  <Button variant="ghost" size="sm" onclick={load} loading={loading}>↻ تحديث</Button>
  <Button variant="soft" size="sm" onclick={openService}>🛒 بيع خدمة</Button>
  <Button variant="soft" size="sm" onclick={openTeacherExpense}>👔 صرف لمدرس</Button>
  <Button variant="soft" size="sm" onclick={() => goto('/reports?code=rpt_daily_cash_flow')}>
    📊 تقارير الصندوق
  </Button>
</PageHeader>

{#if error}
  <div class="mb-4"><Alert kind="error" onClose={() => (error = '')}>{error}</Alert></div>
{/if}

{#if loading && registers.length === 0}
  <div class="ec-kpi-row">
    {#each Array(5) as _, i (i)}
      <div class="ec-skel h-[104px] rounded-2xl"></div>
    {/each}
  </div>
{:else}
  <!-- register picker -->
  <div class="ec-toolbar">
    <label class="ec-label" for="reg">الصندوق</label>
    <select
      id="reg"
      class="ec-select"
      style="max-width:320px"
      value={registerId}
      onchange={(e) => selectRegister(Number(e.currentTarget.value))}
    >
      {#each registers as r (r.id)}
        <option value={r.id}>{r.name} ({r.code})</option>
      {/each}
    </select>
    {#if register}
      <Badge status={register.status === 'active' ? 'active' : 'closed'} />
      {#if register.status !== 'active'}
        <Button variant="success" size="sm" onclick={openRegister}>فتح الصندوق</Button>
      {/if}
    {/if}
  </div>

  <!-- auto-calculated totals -->
  <div class="ec-kpi-row">
    <StatCard
      label="الرصيد الحالي"
      value={money(balance)}
      foot={summary ? `${count(summary.movement_count)} حركة مسجّلة` : ''}
      icon="🏦"
      accent="var(--ec-teal)"
    />
    <StatCard
      label="وارد اليوم"
      value={money(summary?.today_in_minor ?? 0)}
      foot="محسوب من حركات اليوم"
      icon="↓"
      accent="var(--ec-blue)"
    />
    <StatCard
      label="صادر اليوم"
      value={money(summary?.today_out_minor ?? 0)}
      foot="محسوب من حركات اليوم"
      icon="↑"
      accent="var(--ec-red)"
    />
    <StatCard
      label="إجمالي القبض"
      value={money(summary?.total_in_minor ?? 0)}
      foot="منذ فتح الصندوق"
      icon="＋"
      accent="#8fae20"
    />
    <StatCard
      label="إجمالي الصرف"
      value={money(summary?.total_out_minor ?? 0)}
      foot="منذ فتح الصندوق"
      icon="－"
      accent="#b7791f"
    />
  </div>

  <div class="mt-4 grid gap-4" style="grid-template-columns: minmax(0, 420px) minmax(0, 1fr);">
    <!-- ---------------- record a movement ---------------- -->
    <Card title="تسجيل حركة">
      <form class="space-y-4" onsubmit={record}>
        <div class="grid grid-cols-2 gap-2">
          <button
            type="button"
            class="ec-btn {direction === 'in' ? 'ec-btn-success' : 'ec-btn-ghost'}"
            onclick={() => (direction = 'in')}
          >
            ↓ قبض (وارد)
          </button>
          <button
            type="button"
            class="ec-btn {direction === 'out' ? 'ec-btn-danger' : 'ec-btn-ghost'}"
            onclick={() => (direction = 'out')}
          >
            ↑ صرف (صادر)
          </button>
        </div>

        <Field label="نوع الحركة">
          <select class="ec-select" bind:value={kind}>
            <option value="receipt">قبض</option>
            <option value="payment">صرف</option>
            <option value="adjustment">تسوية</option>
            <option value="transfer_in">تحويل وارد</option>
            <option value="transfer_out">تحويل صادر</option>
            <option value="opening">رصيد افتتاحي</option>
          </select>
        </Field>

        <Field label="المبلغ" hint="بالعملة (2 منزلة عشرية) — يُحوَّل تلقائياً إلى الوحدة الصغيرة.">
          <input
            class="ec-input ec-num"
            type="number"
            min="0"
            step="0.01"
            bind:value={amountMajor}
            dir="ltr"
            placeholder="0.00"
          />
        </Field>

        <div class="flex flex-wrap gap-2">
          {#each [10, 25, 50, 100, 200, 500] as q (q)}
            <button
              type="button"
              class="ec-btn ec-btn-ghost ec-btn-sm"
              onclick={() => (amountMajor = q)}
            >
              {q}
            </button>
          {/each}
        </div>

        <Field label="ملاحظة (اختياري)">
          <input class="ec-input" bind:value={note} placeholder="مثال: قبض نقدي من طالب" />
        </Field>

        <!-- live preview: this is the auto-calculation -->
        <div
          class="rounded-xl px-4 py-3 text-sm"
          style="background:var(--ec-surface-2); border:1px dashed var(--ec-border-strong)"
        >
          <div class="flex justify-between">
            <span style="color:var(--ec-text-2)">الرصيد الحالي</span>
            <span class="ec-num font-semibold">{money(balance)}</span>
          </div>
          <div class="mt-1 flex justify-between">
            <span style="color:var(--ec-text-2)">
              {direction === 'in' ? '＋' : '－'} {money(amountMinor)}
            </span>
            <span class="ec-num font-semibold" style="color:{overdrawn ? 'var(--ec-red)' : 'var(--ec-text)'}">
              {direction === 'in' ? '+' : '-'}{money(amountMinor)}
            </span>
          </div>
          <div class="ec-divider" style="margin:8px 0"></div>
          <div class="flex justify-between text-[15px] font-bold">
            <span>الرصيد بعد العملية</span>
            <span class="ec-num" style="color:{overdrawn ? 'var(--ec-red)' : 'var(--ec-teal)'}">
              {money(previewBalance)}
            </span>
          </div>
          {#if overdrawn}
            <p class="ec-err mt-1.5">المبلغ أكبر من رصيد الصندوق — لن تُقبل العملية.</p>
          {/if}
        </div>

        <Button type="submit" variant="primary" block loading={recording} disabled={overdrawn}>
          تسجيل الحركة
        </Button>
      </form>
    </Card>

    <!-- ---------------- ledger + closing ---------------- -->
    <div class="space-y-4">
      <Card title="آخر الحركات" pad={false}>
        <div class="p-4">
          <DataTable
            columns={movementCols}
            rows={movements}
            rowKey={(m) => m.id}
            emptyTitle="لا توجد حركات بعد"
            emptyHint="سجّل أول حركة من النموذج المجاور."
            emptyIcon="💵"
          />
        </div>
      </Card>

      <Card title="إقفال الصندوق">
        {#if summary?.last_closing}
          <div
            class="mb-3 rounded-lg px-3 py-2 text-[13px]"
            style="background:var(--ec-surface-2); border:1px solid var(--ec-border)"
          >
            آخر إغلاق: {dateTime(summary.last_closing.closed_at)} — الفرق
            <span
              class="ec-num font-bold"
              style="color:{summary.last_closing.variance_minor === 0
                ? 'var(--ec-teal)'
                : 'var(--ec-red)'}"
            >
              {money(summary.last_closing.variance_minor)}
            </span>
            <Badge status={summary.last_closing.status} />
          </div>
        {/if}

        <Button variant="ghost" size="sm" onclick={() => (showClose = !showClose)}>
          {showClose ? 'إخفاء نموذج الإغلاق' : 'إغلاق الصندوق…'}
        </Button>

        {#if showClose}
          <form class="mt-4 space-y-3" onsubmit={closeRegister}>
            <div class="ec-form-grid">
              <Field label="الرصيد الافتتاحي">
                <input
                  class="ec-input ec-num"
                  type="number"
                  step="0.01"
                  min="0"
                  bind:value={openingMajor}
                  dir="ltr"
                />
              </Field>
              <Field label="الرصيد المعدود فعلياً">
                <input
                  class="ec-input ec-num"
                  type="number"
                  step="0.01"
                  min="0"
                  bind:value={countedMajor}
                  dir="ltr"
                />
              </Field>
            </div>

            <div class="ec-form-grid">
              <Field label="حساب الصندوق">
                <select class="ec-select" bind:value={cashAccountId}>
                  {#each accounts as a (a.id)}
                    <option value={a.id}>{a.code} — {a.name}</option>
                  {/each}
                </select>
              </Field>
              <Field label="حساب فروقات الصندوق">
                <select class="ec-select" bind:value={varianceAccountId}>
                  {#each accounts as a (a.id)}
                    <option value={a.id}>{a.code} — {a.name}</option>
                  {/each}
                </select>
              </Field>
            </div>

            <div
              class="rounded-xl px-4 py-3 text-sm"
              style="background:var(--ec-surface-2); border:1px solid var(--ec-border)"
            >
              <div class="flex justify-between">
                <span style="color:var(--ec-text-2)">الرصيد الدفتري (محسوب تلقائياً)</span>
                <span class="ec-num font-semibold">{money(summary?.balance_minor ?? 0)}</span>
              </div>
              <div class="mt-1 flex justify-between">
                <span style="color:var(--ec-text-2)">الرصيد المعدود</span>
                <span class="ec-num font-semibold">
                  {money(Math.round(Number(countedMajor || 0) * 100))}
                </span>
              </div>
              <div class="mt-2 flex justify-between border-t pt-2 text-[15px] font-bold"
                style="border-color:var(--ec-border)">
                <span>الفرق (يُرحَّل تلقائياً)</span>
                <span class="ec-num" style="color:{varianceMinor === 0 ? 'var(--ec-teal)' : 'var(--ec-amber)'}">
                  {money(varianceMinor)}
                </span>
              </div>
            </div>

            <Alert kind="warn">
              عند الإغلاق يُنشأ قيد محاسبي بالفرق تلقائياً ويُعتمد السند.
            </Alert>

            <Button type="submit" variant="primary" loading={closing}>
              إغلاق الصندوق
            </Button>
          </form>
        {/if}
      </Card>
    </div>
  </div>

  <!-- ---------------- report shortcuts ---------------- -->
  <Card title="تقارير الصندوق" classes="mt-4" pad={false}>
    <div class="grid grid-cols-1 gap-2 p-4 sm:grid-cols-2 lg:grid-cols-3">
      {#each REPORTS as r (r.code)}
        <button
          type="button"
          class="flex items-center gap-3 rounded-xl border px-3.5 py-3 text-right transition-all hover:-translate-y-0.5 hover:shadow-md"
          style="border-color:var(--ec-border); background:var(--ec-surface-2)"
          onclick={() => goto(`/reports?code=${r.code}`)}
        >
          <span class="text-lg" aria-hidden="true">{r.icon}</span>
          <span class="min-w-0 flex-1 truncate text-[13.5px] font-semibold" style="color:var(--ec-navy)">
            {r.title}
          </span>
          <span aria-hidden="true" style="color:var(--ec-text-3)">‹</span>
        </button>
      {/each}
    </div>
  </Card>
{/if}

<Modal
  open={showTeacherExpense}
  title="صرف للمدرس"
  subtitle="يُنشأ سند صرف مرتبط بملف المدرس — يظهر في «المستحق» على ملفه."
  onClose={() => (showTeacherExpense = false)}
  width="560px"
>
  <form class="space-y-4" onsubmit={saveTeacherExpense}>
    {#if expError}<Alert kind="error">{expError}</Alert>{/if}
    <Field label="المدرس" required>
      <select class="ec-select" bind:value={expTeacherId}>
        {#each teachers as t (t.id)}<option value={t.id}>{t.full_name}</option>{/each}
      </select>
    </Field>
    <div class="ec-form-grid">
      <Field label="بند المصروف" required>
        <select class="ec-select" bind:value={expCategoryId}>
          {#each expCategories as c (c.id)}<option value={c.id}>{c.code} — {c.name}</option>{/each}
        </select>
      </Field>
      <Field label="المبلغ" required>
        <input class="ec-input ec-num" type="number" min="0" step="0.01" bind:value={expAmountMajor} dir="ltr" required />
      </Field>
    </div>
    <div class="ec-form-grid">
      <Field label="التاريخ"><input class="ec-input" type="date" dir="ltr" bind:value={expDate} /></Field>
      <Field label="الوصف"><input class="ec-input" bind:value={expDesc} placeholder="مكافأة / راتب…" /></Field>
    </div>
    <div class="flex justify-end gap-2">
      <Button variant="ghost" onclick={() => (showTeacherExpense = false)}>إلغاء</Button>
      <Button type="submit" variant="primary" loading={expSaving} disabled={!expTeacherId || !expCategoryId}>تسجيل الصرف</Button>
    </div>
  </form>
</Modal>

<Modal
  open={showService}
  title="بيع خدمة إضافية"
  subtitle="ملخص مادة، طباعة، كتاب… يُسجَّل سند قبض ويُرحَّل إلى حساب الإيرادات تلقائياً."
  onClose={() => (showService = false)}
  width="560px"
>
  <form class="space-y-4" onsubmit={saveService}>
    {#if svcError}<Alert kind="error">{svcError}</Alert>{/if}

    <div class="ec-field">
      <span class="ec-label">الخدمة</span>
      <div class="flex flex-wrap gap-2">
        {#each SERVICE_PRESETS as p (p)}
          <button
            type="button"
            class="rounded-lg border px-3 py-1.5 text-[13px] font-semibold transition-colors"
            style={svcDesc === p
              ? 'background:var(--ec-blue-soft); border-color:var(--ec-blue); color:var(--ec-blue)'
              : 'background:var(--ec-surface-2); border-color:var(--ec-border); color:var(--ec-text-2)'}
            onclick={() => pickService(p)}
          >
            {p}
          </button>
        {/each}
      </div>
    </div>

    <div class="ec-form-grid">
      <Field label="اسم الدافع" required>
        <input class="ec-input" bind:value={svcPayer} placeholder="اسم الطالب أو ولي الأمر" required />
      </Field>
      <Field label="المبلغ" required>
        <input
          class="ec-input ec-num"
          type="number"
          min="0"
          step="0.01"
          dir="ltr"
          bind:value={svcAmountMajor}
          required
        />
      </Field>
    </div>

    <Field label="وصف الخدمة" required hint="يظهر في السند وفي تقارير الإيرادات.">
      <input class="ec-input" bind:value={svcDesc} placeholder="مثال: ملخص مادة الرياضيات — 20 ورقة" required />
    </Field>

    <Field label="طريقة الدفع">
      <select class="ec-select" bind:value={svcMethod}>
        <option value="cash">نقداً</option>
        <option value="card">بطاقة</option>
        <option value="transfer">حوالة</option>
        <option value="wallet">محفظة إلكترونية</option>
      </select>
    </Field>

    <div
      class="rounded-xl px-4 py-3 text-sm"
      style="background:var(--ec-surface-2); border:1px solid var(--ec-border)"
    >
      <div class="flex justify-between">
        <span style="color:var(--ec-text-2)">القيد المحاسبي</span>
        <span class="ec-num font-semibold">مدين الصندوق 1000 / دائن الإيرادات 4000</span>
      </div>
      <div class="mt-1 flex justify-between">
        <span style="color:var(--ec-text-2)">المبلغ</span>
        <span class="ec-num font-semibold">{money(Math.round(Number(svcAmountMajor || 0) * 100))}</span>
      </div>
    </div>

    <div class="flex justify-end gap-2">
      <Button variant="ghost" onclick={() => (showService = false)}>إلغاء</Button>
      <Button type="submit" variant="primary" loading={svcSaving}>تسجيل سند القبض</Button>
    </div>
  </form>
</Modal>
