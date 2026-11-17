<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { get } from 'svelte/store';
  import { sessionToken } from '$lib/stores/app';
  import {
    reportsApi,
    type ReportDefinition,
    type ReportResult,
    type SavedReport
  } from '$lib/api/app';
  import { toast } from '$lib/toasts';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Card from '$lib/components/Card.svelte';
  import Button from '$lib/components/Button.svelte';
  import Alert from '$lib/components/Alert.svelte';
  import Tabs from '$lib/components/Tabs.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import DataTable from '$lib/components/DataTable.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import { COLUMN_LABELS, isDateColumn, isMoneyColumn } from '$lib/reportLabels';
  import {
    count,
    date,
    dateTime,
    money,
    monthLabel,
    REPORT_CATEGORY
  } from '$lib/format';
  import type { Column } from '$lib/table';

  let defs = $state<ReportDefinition[]>([]);
  let saved = $state<SavedReport[]>([]);
  let result = $state<ReportResult | null>(null);
  let selected = $state('');
  let query = $state('');
  let category = $state('all');
  let loading = $state(true);
  let running = $state(false);
  let exporting = $state(false);
  let error = $state('');

  const CATS = [
    { id: 'all', label: 'الكل' },
    { id: 'admin', label: 'إداري' },
    { id: 'financial', label: 'مالي' },
    { id: 'control', label: 'رقابي' }
  ];

  const filtered = $derived(
    defs.filter(
      (d) =>
        (category === 'all' || d.category === category) &&
        (query.trim() === '' ||
          d.title.includes(query.trim()) ||
          d.code.includes(query.trim()))
    )
  );

  const grouped = $derived(
    (['admin', 'financial', 'control'] as const)
      .map((c) => ({ cat: c, items: filtered.filter((d) => d.category === c) }))
      .filter((g) => g.items.length > 0)
  );

  const current = $derived(defs.find((d) => d.code === selected) ?? null);

  const headerOf = (key: string) => COLUMN_LABELS[key] ?? key;

  function cellFormat(key: string) {
    return (value: unknown): string => {
      if (value === null || value === undefined) return '—';
      if (isMoneyColumn(key)) return money(Number(value ?? 0));
      if (key === 'period') return monthLabel(String(value));
      if (key === 'rate') return `${Number(value).toLocaleString('en-US')}٪`;
      if (key === 'balanced') return Number(value) ? 'متوازن' : 'غير متوازن';
      if (key === 'date' || key.endsWith('_at')) return dateTime(String(value));
      if (isDateColumn(key)) return date(String(value));
      if (typeof value === 'number') return value.toLocaleString('en-US');
      if (typeof value === 'boolean') return value ? 'نعم' : 'لا';
      return String(value);
    };
  }

  const columns = $derived<Column<Record<string, unknown>>[]>(
    (result?.columns ?? []).map((key) => ({
      key,
      label: headerOf(key),
      num: isMoneyColumn(key) || key === 'rate' || typeof result?.rows[0]?.[key] === 'number',
      format: cellFormat(key)
    }))
  );

  async function loadCatalogue() {
    const token = get(sessionToken);
    if (!token) {
      loading = false;
      return;
    }
    loading = true;
    error = '';
    try {
      const [d, s] = await Promise.all([reportsApi.definitions(token), reportsApi.list(token)]);
      defs = d;
      saved = s;
    } catch (e) {
      error = (e as Error)?.message ?? 'تعذّر تحميل التقارير';
    } finally {
      loading = false;
    }
  }

  async function run(code: string) {
    const token = get(sessionToken);
    if (!token || !code) return;
    selected = code;
    running = true;
    error = '';
    try {
      result = await reportsApi.run(token, code);
    } catch (e) {
      result = null;
      error = (e as Error)?.message ?? 'تعذّر تنفيذ التقرير';
    } finally {
      running = false;
    }
  }

  async function exportXlsx() {
    const token = get(sessionToken);
    if (!token || !selected) return;
    exporting = true;
    try {
      const path = await reportsApi.export(token, selected);
      toast.ok(`تم تصدير التقرير إلى: ${path}`);
    } catch (e) {
      toast.error((e as Error)?.message ?? 'تعذّر التصدير');
    } finally {
      exporting = false;
    }
  }

  async function saveCurrent() {
    const token = get(sessionToken);
    if (!token || !current) return;
    try {
      await reportsApi.save(token, current.title, JSON.stringify({ code: current.code }));
      saved = await reportsApi.list(token);
      toast.ok('تم حفظ التقرير في المفضلة');
    } catch (e) {
      toast.error((e as Error)?.message ?? 'تعذّر الحفظ');
    }
  }

  onMount(() => {
    loadCatalogue().then(() => {
      const pre = page.url.searchParams.get('code');
      if (pre) run(pre);
    });
  });
</script>

<PageHeader
  title="التقارير"
  subtitle="{defs.length} تقريراً جاهزاً — البيانات تُحسب من قاعدة البيانات المحلية مباشرة"
>
  <Button variant="ghost" size="sm" onclick={() => loadCatalogue()} loading={loading}>
    ↻ تحديث
  </Button>
</PageHeader>

{#if error}
  <div class="mb-4"><Alert kind="error" onClose={() => (error = '')}>{error}</Alert></div>
{/if}

<div class="ec-grid" style="grid-template-columns: minmax(0, 340px) minmax(0, 1fr);">
  <!-- ------------------------- catalogue ------------------------- -->
  <div class="space-y-4">
    <Card title="فهرس التقارير" pad={false}>
      <div class="space-y-3 p-4">
        <input
          class="ec-input"
          type="search"
          placeholder="ابحث عن تقرير…"
          bind:value={query}
          aria-label="بحث في التقارير"
        />
        <Tabs items={CATS} bind:value={category} />
      </div>

      <div class="max-h-[58vh] overflow-y-auto px-4 pb-4">
        {#if loading}
          <div class="space-y-2">
            {#each Array(8) as _, i (i)}
              <div class="ec-skel h-11 rounded-lg"></div>
            {/each}
          </div>
        {:else if grouped.length === 0}
          <EmptyState icon="🔍" title="لا توجد نتائج" hint="جرّب كلمة أخرى أو غيّر التصنيف" />
        {:else}
          {#each grouped as g (g.cat)}
            <div
              class="mb-2 mt-3 text-[11.5px] font-bold"
              style="color:var(--ec-text-3)"
            >
              {REPORT_CATEGORY[g.cat]?.[0] ?? g.cat}
            </div>
            <ul class="space-y-1">
              {#each g.items as d (d.code)}
                <li>
                  <button
                    type="button"
                    class="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-right text-[13.5px] transition-colors"
                    class:font-semibold={selected === d.code}
                    style={selected === d.code
                      ? 'background: var(--ec-blue-soft); color: var(--ec-blue);'
                      : 'color: var(--ec-text-2);'}
                    onclick={() => run(d.code)}
                  >
                    <span class="min-w-0 flex-1 truncate">{d.title}</span>
                    <span class="flex-none text-[11px] opacity-60">{d.columns.length}</span>
                  </button>
                </li>
              {/each}
            </ul>
          {/each}
        {/if}
      </div>
    </Card>

    {#if saved.length}
      <Card title="تقارير محفوظة" pad={false}>
        <ul class="max-h-52 overflow-y-auto p-3">
          {#each saved as s (s.id)}
            <li class="mb-1 flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-[13px]">
              <span aria-hidden="true">★</span>
              <span class="min-w-0 flex-1 truncate" style="color:var(--ec-text-2)">{s.name}</span>
              {#if s.is_favorite}
                <Badge label="مفضل" tone="warn" dot={false} />
              {/if}
            </li>
          {/each}
        </ul>
      </Card>
    {/if}
  </div>

  <!-- ------------------------- viewer ------------------------- -->
  <div class="min-w-0">
    {#if !selected}
      <Card>
        <EmptyState
          icon="📊"
          title="اختر تقريراً من الفهرس"
          hint="سيتم تنفيذ التقرير فوراً وعرض نتائجه في جدول قابل للتصدير إلى Excel."
        />
      </Card>
    {:else}
      <Card pad={false}>
        <div class="flex flex-wrap items-center justify-between gap-3 border-b px-5 py-4"
          style="border-color:var(--ec-border)">
          <div class="min-w-0">
            <div class="flex flex-wrap items-center gap-2">
              <h2 class="ec-card-title">{current?.title ?? selected}</h2>
              {#if current}
                <Badge
                  label={REPORT_CATEGORY[current.category]?.[0] ?? current.category}
                  tone={current.category === 'financial' ? 'ok' : current.category === 'control' ? 'accent' : 'info'}
                  dot={false}
                />
              {/if}
            </div>
            <p class="mt-1 text-[12.5px]" style="color:var(--ec-text-3)">
              {running ? 'جارٍ التنفيذ…' : `${count(result?.row_count ?? 0)} صف • ${result?.columns.length ?? 0} عمود`}
              <span class="ec-mono ms-2 opacity-70">{selected}</span>
            </p>
          </div>
          <div class="flex flex-wrap items-center gap-2">
            <Button variant="ghost" size="sm" onclick={saveCurrent} disabled={!current}>
              ★ حفظ
            </Button>
            <Button variant="ghost" size="sm" onclick={() => run(selected)} loading={running}>
              ↻ إعادة تنفيذ
            </Button>
            <Button
              variant="primary"
              size="sm"
              onclick={exportXlsx}
              loading={exporting}
              disabled={!result}
            >
              ⬇ تصدير Excel
            </Button>
          </div>
        </div>

        <div class="p-4">
          {#if running && !result}
            <div class="space-y-2">
              {#each Array(7) as _, i (i)}
                <div class="ec-skel h-9 rounded-md"></div>
              {/each}
            </div>
          {:else if result}
            <DataTable
              columns={columns}
              rows={result.rows}
              rowKey={(r) => JSON.stringify(r)}
              emptyTitle="لا توجد بيانات مطابقة"
              emptyHint="لم يُعثر على أي صف يطابق هذا التقرير في قاعدة البيانات الحالية."
              caption={result.title}
            />
          {/if}
        </div>
      </Card>
    {/if}
  </div>
</div>
