<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { sessionToken, sessionUser } from '$lib/stores/app';
  import { get } from 'svelte/store';
  import LoginForm from '$lib/components/LoginForm.svelte';
  import Logo from '$lib/components/Logo.svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Card from '$lib/components/Card.svelte';
  import StatCard from '$lib/components/StatCard.svelte';
  import Alert from '$lib/components/Alert.svelte';
  import Chart from '$lib/components/Chart.svelte';
  import Button from '$lib/components/Button.svelte';
  import { dashboardApi, reportsApi, type DashboardCharts, type DashboardKpis, type ReportDefinition } from '$lib/api/app';
  import { isTauri } from '$lib/env';
  import { count, money, moneyCompact, monthShort, dayLabel, statusInfo } from '$lib/format';

  let kpis = $state<DashboardKpis | null>(null);
  let charts = $state<DashboardCharts | null>(null);
  let shortcuts = $state<ReportDefinition[]>([]);
  let loading = $state(true);
  let error = $state('');

  const last = <T,>(arr: T[]): T | undefined => (arr.length ? arr[arr.length - 1] : undefined);

  const quickActions = [
    { icon: '👤', label: 'طالب جديد', href: '/students', tone: '#6b8e23' },
    { icon: '📝', label: 'تسجيل جديد', href: '/enrollments', tone: '#4f8a1f' },
    { icon: '💰', label: 'الصندوق', href: '/cash', tone: '#b7791f' },
    { icon: '🧾', label: 'سند قبض', href: '/vouchers', tone: '#8fae20' },
    { icon: '✓', label: 'تسجيل الحضور', href: '/attendance', tone: '#5d7d14' },
    { icon: '💸', label: 'مصروف جديد', href: '/expenses', tone: '#c2413f' },
    { icon: '📊', label: 'التقارير', href: '/reports', tone: '#33491a' },
    { icon: '⚙', label: 'الإعدادات', href: '/settings', tone: '#8894ab' }
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
      const [k, c, defs] = await Promise.all([
        dashboardApi.kpis(token),
        dashboardApi.charts(token),
        reportsApi.definitions(token)
      ]);
      kpis = k;
      charts = c;
      // a curated strip of the most-used reports, right on the dashboard
      const wanted = [
        'rpt_collections',
        'rpt_receivables',
        'rpt_daily_cash_flow',
        'rpt_attendance_rate',
        'rpt_expenses_by_category',
        'rpt_pending_ops'
      ];
      shortcuts = wanted
        .map((code) => defs.find((d) => d.code === code))
        .filter((d): d is ReportDefinition => !!d)
        .slice(0, 6);
    } catch (e) {
      error = (e as Error)?.message ?? 'تعذّر تحميل لوحة التحكم';
    } finally {
      loading = false;
    }
  }

  const cashFlow = $derived(charts?.cash_flow ?? []);

  onMount(load);
</script>

{#if !$sessionUser}
  <div class="w-full max-w-md">
    {#if !isTauri}
      <Alert kind="warn">وضع المعاينة في المتصفح — الدخول الفعلي يعمل داخل التطبيق فقط</Alert>
    {/if}
    <div class="mb-2 flex justify-center"><Logo size={56} variant="icon" /></div>
    <div class="mt-1"><LoginForm /></div>
  </div>
{:else}
  <PageHeader title="لوحة التحكم" subtitle="مرحباً {$sessionUser.full_name} — نظرة شاملة على اليوم">
    <Button variant="ghost" size="sm" onclick={() => goto('/reports')}>📊 كل التقارير</Button>
    <Button variant="primary" size="sm" onclick={() => goto('/students')}>+ طالب جديد</Button>
  </PageHeader>

  {#if error}
    <div class="mb-4"><Alert kind="error" onClose={() => (error = '')}>{error}</Alert></div>
  {/if}

  {#if loading && !kpis}
    <div class="ec-kpi-row">
      {#each Array(6) as _, i (i)}
        <div class="ec-skel h-[104px] rounded-2xl"></div>
      {/each}
    </div>
    <div class="ec-grid ec-cols-2 mt-4">
      {#each Array(2) as _, i (i)}
        <div class="ec-skel h-[260px] rounded-2xl"></div>
      {/each}
    </div>
  {:else if kpis}
    <!-- ---------- headline KPIs ---------- -->
    <div class="ec-kpi-row">
      <StatCard
        label="الطلاب النشطون"
        value={count(kpis.students_active)}
        foot={`من ${count(kpis.students_total)} طالب مسجّل`}
        icon="👤"
        accent="var(--ec-blue)"
        onclick={() => goto('/students')}
      />
      <StatCard
        label="التحصيل هذا الشهر"
        value={moneyCompact(kpis.collected_month_minor)}
        foot={`اليوم: ${money(kpis.collected_today_minor)}`}
        icon="💰"
        accent="var(--ec-teal)"
        onclick={() => goto('/cash')}
      />
      <StatCard
        label="الذمم المدينة"
        value={moneyCompact(kpis.receivable_minor)}
        foot={`${count(kpis.enrollments_active)} تسجيل نشط`}
        icon="⚖"
        accent="var(--ec-amber)"
        onclick={() => goto('/finance')}
      />
      <StatCard
        label="رصيد الصندوق"
        value={moneyCompact(kpis.cash_balance_minor)}
        foot={`${count(kpis.today_sessions)} حصة اليوم`}
        icon="🏦"
        accent="#8fae20"
        onclick={() => goto('/cash')}
      />
      <StatCard
        label="المصروفات هذا الشهر"
        value={moneyCompact(kpis.expenses_month_minor)}
        foot={`${count(kpis.courses_active)} دورة نشطة`}
        icon="💸"
        accent="#c2413f"
        onclick={() => goto('/expenses')}
      />
      <StatCard
        label="قيد المعالجة"
        value={count(kpis.pending_refunds + kpis.pending_shortfalls + kpis.receipts_pending)}
        foot={`${count(kpis.pending_refunds)} استرداد • ${count(kpis.pending_shortfalls)} عجز`}
        icon="⏳"
        accent="#b7791f"
        onclick={() => goto('/reports')}
      />
    </div>

    <!-- ---------- quick shortcuts ---------- -->
    <Card title="اختصارات" pad={false} classes="mt-4">
      <div class="grid grid-cols-2 gap-2 p-4 sm:grid-cols-4 lg:grid-cols-8">
        {#each quickActions as a (a.href + a.label)}
          <button
            type="button"
            class="group flex flex-col items-center gap-1.5 rounded-xl border px-2 py-3 text-center transition-all hover:-translate-y-0.5 hover:shadow-md"
            style="border-color:var(--ec-border); background:var(--ec-surface-2)"
            onclick={() => goto(a.href)}
          >
            <span
              class="flex h-9 w-9 items-center justify-center rounded-lg text-lg"
              style="background: color-mix(in srgb, {a.tone} 14%, transparent)"
              aria-hidden="true"
            >
              {a.icon}
            </span>
            <span class="text-[12.5px] font-semibold" style="color:var(--ec-text-2)">
              {a.label}
            </span>
          </button>
        {/each}
      </div>
    </Card>

    <!-- ---------- charts ---------- -->
    <div class="ec-grid ec-cols-2 mt-4">
      <Card title="التحصيل مقابل المصروفات (آخر 6 أشهر)">
        {#if charts}
          <Chart
            type="bar"
            height={260}
            series={[
              {
                name: 'التحصيل',
                color: '#6b8e23',
                data: charts.collections_by_month.map((p) => ({
                  label: monthShort(p.label),
                  value: p.value
                }))
              },
              {
                name: 'المصروفات',
                color: '#c2413f',
                data: charts.expenses_by_month.map((p) => ({
                  label: monthShort(p.label),
                  value: p.value
                }))
              }
            ]}
            format={(v) => (v / 100).toLocaleString('en-US', { maximumFractionDigits: 0 })}
          />
        {:else}
          <div class="ec-skel h-[250px] rounded-xl"></div>
        {/if}
      </Card>

      <Card title="التدفق النقدي (آخر 14 يوماً)">
        {#if charts}
          <Chart
            type="line"
            data={charts.cash_flow.map((p) => ({ label: dayLabel(p.date), value: p.balance_minor }))}
            color="var(--ec-teal)"
            format={(v) => (v / 100).toLocaleString('en-US', { maximumFractionDigits: 0 })}
            height={250}
            showValues={false}
          />
          <div class="mt-3 grid grid-cols-3 gap-3 text-center text-[13px]">
            <div>
              <div class="ec-muted text-[12px]">وارد اليوم</div>
              <div class="ec-num font-bold" style="color:var(--ec-teal)">
                {money(last(cashFlow)?.in_minor ?? 0)}
              </div>
            </div>
            <div>
              <div class="ec-muted text-[12px]">صادر اليوم</div>
              <div class="ec-num font-bold" style="color:var(--ec-red)">
                {money(last(cashFlow)?.out_minor ?? 0)}
              </div>
            </div>
            <div>
              <div class="ec-muted text-[12px]">الرصيد</div>
              <div class="ec-num font-bold" style="color:var(--ec-navy)">
                {money(last(cashFlow)?.balance_minor ?? 0)}
              </div>
            </div>
          </div>
        {:else}
          <div class="ec-skel h-[250px] rounded-xl"></div>
        {/if}
      </Card>
    </div>

    <div class="ec-grid ec-cols-3 mt-4">
      <Card title="توزيع الطلاب">
        {#if charts}
          <Chart
            type="donut"
            data={charts.students_by_status.map((p) => ({ label: p.label, value: p.value }))}
            format={(v) => count(v)}
            height={200}
          />
        {/if}
      </Card>

      <Card title="الحضور (جميع الحالات)">
        {#if charts}
          <Chart
            type="hbar"
            data={charts.attendance_by_status.map((p) => ({
              label: statusInfo(p.label).label,
              value: p.value
            }))}
            format={(v) => count(v)}
            colors={['#4f8a1f', '#c2413f', '#b7791f', '#6b8e23']}
            height={200}
          />
        {/if}
      </Card>

      <Card title="الدورات الأعلى تسجيلاً">
        {#if charts}
          <Chart
            type="hbar"
            data={charts.top_courses}
            format={(v) => count(v)}
            colors={['#6b8e23']}
            height={200}
          />
        {/if}
      </Card>
    </div>

    <!-- ---------- report shortcuts ---------- -->
    <Card title="تقرير سريع" pad={false} classes="mt-4">
      <div class="grid grid-cols-1 gap-2 p-4 sm:grid-cols-2 lg:grid-cols-3">
        {#each shortcuts as r (r.code)}
          <button
            type="button"
            class="flex items-center gap-3 rounded-xl border px-3.5 py-3 text-right transition-all hover:-translate-y-0.5 hover:shadow-md"
            style="border-color:var(--ec-border); background:var(--ec-surface-2)"
            onclick={() => goto(`/reports?code=${r.code}`)}
          >
            <span class="text-lg" aria-hidden="true">📈</span>
            <span class="min-w-0 flex-1">
              <span class="block truncate text-[13.5px] font-semibold" style="color:var(--ec-navy)">
                {r.title}
              </span>
              <span class="block text-[11.5px]" style="color:var(--ec-text-3)">
                {r.category === 'financial' ? 'مالي' : r.category === 'control' ? 'رقابي' : 'إداري'}
                • {r.columns.length} أعمدة
              </span>
            </span>
            <span aria-hidden="true" style="color:var(--ec-text-3)">‹</span>
          </button>
        {/each}
      </div>
    </Card>

    <!-- ---------- attention list ---------- -->
    {#if kpis.pending_refunds + kpis.pending_shortfalls > 0}
      <Card title="يحتاج انتباهك" classes="mt-4">
        <div class="flex flex-wrap gap-3">
          {#if kpis.pending_refunds > 0}
            <div class="flex items-center gap-2 rounded-lg px-3 py-2" style="background:var(--ec-amber-soft); color:var(--ec-amber)">
              <span class="font-bold">{count(kpis.pending_refunds)}</span>
              طلب استرداد بانتظار الاعتماد
            </div>
          {/if}
          {#if kpis.pending_shortfalls > 0}
            <div class="flex items-center gap-2 rounded-lg px-3 py-2" style="background:var(--ec-red-soft); color:var(--ec-red)">
              <span class="font-bold">{count(kpis.pending_shortfalls)}</span>
              قرار عجز بانتظار المراجعة
            </div>
          {/if}
          {#if kpis.courses_needing_settlement > 0}
            <div class="flex items-center gap-2 rounded-lg px-3 py-2" style="background:var(--ec-blue-soft); color:var(--ec-blue)">
              <span class="font-bold">{count(kpis.courses_needing_settlement)}</span>
              دورة مكتملة بانتظار التسوية
            </div>
          {/if}
        </div>
      </Card>
    {/if}
  {/if}
{/if}
