<script lang="ts">
  /// Dependency-free SVG charts (bar / line / donut / horizontal bar).
  ///
  /// Chosen over a charting library because the app ships as a desktop
  /// bundle: no network, no extra 300 kB, and full RTL control over labels.

  export type ChartType = 'bar' | 'line' | 'donut' | 'hbar';
  export type ChartDatum = { label: string; value: number };
  export type ChartSeries = { name: string; data: ChartDatum[]; color: string };

  let {
    type = 'bar',
    data = [],
    series = [],
    height = 240,
    color = 'var(--ec-blue)',
    colors = ['#6b8e23', '#4f8a1f', '#b7791f', '#8fae20', '#c2413f', '#5d7d14', '#a3b84a'],
    format = (v: number) => v.toLocaleString('en-US'),
    showValues = true,
    showLegend = true,
    empty = 'لا توجد بيانات لعرضها بعد'
  }: {
    type?: ChartType;
    data?: ChartDatum[];
    series?: ChartSeries[];
    height?: number;
    color?: string;
    colors?: string[];
    format?: (v: number) => string;
    showValues?: boolean;
    showLegend?: boolean;
    empty?: string;
  } = $props();

  const W = 640;
  /// Donut canvas width (the legend sits beside it and wraps on narrow panes).
  const width = 216;
  const PAD = { top: 16, right: 10, bottom: 30, left: 48 };
  const plotW = W - PAD.left - PAD.right;

  /// When `series` is supplied the caller is doing a multi-series chart; the
  /// plain `data` prop then serves as the category axis.
  const multi = $derived(series.length > 0);
  const categories = $derived(multi ? series[0].data.map((d) => d.label) : data.map((d) => d.label));
  const slotCount = $derived(Math.max(1, categories.length));
  const slotW = $derived(plotW / slotCount);

  const allValues = $derived(
    multi ? series.flatMap((s) => s.data.map((d) => Math.abs(d.value))) : data.map((d) => Math.abs(d.value))
  );
  const max = $derived(Math.max(1, ...allValues));
  const hasData = $derived(allValues.some((v) => v !== 0));

  const barCount = $derived(multi ? Math.max(1, series.length) : 1);
  const groupW = $derived(Math.min(56, slotW * 0.72));
  const barW = $derived(groupW / barCount);

  const plotH = $derived(height - PAD.top - PAD.bottom);
  const ticks = $derived([0, 0.25, 0.5, 0.75, 1].map((f) => ({ f, v: max * f })));

  function y(v: number) {
    return PAD.top + plotH - (Math.abs(v) / max) * plotH;
  }
  function h(v: number) {
    return (Math.abs(v) / max) * plotH;
  }
  function xAt(i: number, j = 0) {
    const groupX = PAD.left + slotW * i + (slotW - groupW) / 2;
    return groupX + j * barW;
  }
  function xCenter(i: number) {
    return PAD.left + slotW * i + slotW / 2;
  }

  /* ---------- line geometry ---------- */
  function polylineOf(values: number[]) {
    return values.map((v, i) => `${xCenter(i)},${y(v)}`).join(' ');
  }
  function areaOf(values: number[]) {
    if (!values.length) return '';
    const first = xCenter(0);
    const last = xCenter(values.length - 1);
    const base = PAD.top + plotH;
    return `${first},${base} ${polylineOf(values)} ${last},${base}`;
  }

  /* ---------- donut geometry ---------- */
  const donutValues = $derived(
    multi ? series[0].data.map((d) => d.value) : data.map((d) => d.value)
  );
  const total = $derived(Math.max(1, donutValues.reduce((s, v) => s + Math.abs(v), 0)));
  const R = 74;
  const C = 2 * Math.PI * R;
  const donutSegments = $derived(
    donutValues.reduce<{ offset: number; segs: { frac: number; start: number; color: string }[] }>(
      (acc, v, i) => {
        const frac = Math.abs(v) / total;
        acc.segs.push({ frac, start: acc.offset, color: colors[i % colors.length] });
        acc.offset += frac;
        return acc;
      },
      { offset: 0, segs: [] }
    ).segs
  );
  const donutLabels = $derived(
    multi ? series[0].data.map((d) => d.label) : data.map((d) => d.label)
  );

  const hbData = $derived(multi ? series[0].data : data);

  const legend = $derived(
    multi
      ? series.map((s) => ({ label: s.name, color: s.color }))
      : type === 'donut'
        ? donutLabels.map((l, i) => ({ label: l, color: colors[i % colors.length] }))
        : []
  );
</script>

{#if !hasData}
  <div class="ec-empty" style="padding:28px 12px">
    <div class="ec-empty-icon" aria-hidden="true">📊</div>
    <div class="ec-empty-hint">{empty}</div>
  </div>
{:else if type === 'donut'}
  <div class="flex flex-wrap items-center justify-center gap-6">
    <svg {width} {height} viewBox="0 0 {width} {height}" role="img" aria-label="رسم بياني دائري">
      <g transform="translate({width / 2},{height / 2})">
        <circle r={R} fill="none" stroke="var(--ec-bg-2)" stroke-width="26" />
        {#each donutSegments as seg, i (i)}
          <circle
            r={R}
            fill="none"
            stroke={seg.color}
            stroke-width="26"
            stroke-dasharray="{seg.frac * C} {C}"
            stroke-dashoffset={-seg.start * C}
            transform="rotate(-90)"
          >
            <title>{donutLabels[i]}: {format(donutValues[i])}</title>
          </circle>
        {/each}
        <text text-anchor="middle" y="-4" font-size="21" font-weight="700" fill="var(--ec-navy)">
          {format(donutValues.reduce((s, v) => s + Math.abs(v), 0))}
        </text>
        <text text-anchor="middle" y="18" font-size="12" fill="var(--ec-text-3)">الإجمالي</text>
      </g>
    </svg>
    <ul class="space-y-1.5 text-[13px]">
      {#each donutLabels as label, i (label)}
        <li class="flex items-center gap-2">
          <span
            class="inline-block h-2.5 w-2.5 flex-none rounded-sm"
            style="background:{colors[i % colors.length]}"
          ></span>
          <span class="flex-1" style="color:var(--ec-text-2)">{label}</span>
          <span class="ec-num font-semibold">{format(donutValues[i])}</span>
        </li>
      {/each}
    </ul>
  </div>
{:else if type === 'hbar'}
  <div class="space-y-3">
    {#each hbData as d, i (d.label)}
      {@const w = (Math.abs(d.value) / max) * 100}
      <div>
        <div class="mb-1 flex items-baseline justify-between gap-3 text-[13px]">
          <span class="truncate" style="color:var(--ec-text-2)">{d.label}</span>
          {#if showValues}
            <span class="ec-num flex-none font-semibold" style="color:var(--ec-navy)">
              {format(d.value)}
            </span>
          {/if}
        </div>
        <div class="h-2.5 w-full overflow-hidden rounded-full" style="background:var(--ec-bg-2)">
          <div
            class="h-full rounded-full"
            style="width:{w}%; background:{(multi ? series[0].color : colors[i % colors.length])}; transition: width 500ms var(--ec-ease);"
          ></div>
        </div>
      </div>
    {/each}
  </div>
{:else}
  <svg
    width="100%"
    {height}
    viewBox="0 0 {W} {height}"
    preserveAspectRatio="xMidYMid meet"
    role="img"
    aria-label={type === 'line' ? 'رسم بياني خطي' : 'رسم بياني أعمدة'}
  >
    {#each ticks as t (t.f)}
      <line
        x1={PAD.left}
        x2={W - PAD.right}
        y1={PAD.top + plotH - t.f * plotH}
        y2={PAD.top + plotH - t.f * plotH}
        stroke="var(--ec-border)"
        stroke-width="1"
        stroke-dasharray={t.f === 0 ? '0' : '3 4'}
      />
      <text
        x={PAD.left - 8}
        y={PAD.top + plotH - t.f * plotH + 4}
        text-anchor="end"
        font-size="10.5"
        fill="var(--ec-text-3)"
      >
        {Math.round(t.v).toLocaleString('en-US')}
      </text>
    {/each}

    {#if type === 'bar'}
      {#if multi}
        {#each categories as cat, i (cat)}
          {#each series as s, j (s.name)}
            <rect
              x={xAt(i, j)}
              y={y(s.data[i]?.value ?? 0)}
              width={Math.max(2, barW - 3)}
              height={Math.max(1.5, h(s.data[i]?.value ?? 0))}
              rx="4"
              fill={s.color}
              opacity="0.92"
            >
              <title>{cat} — {s.name}: {format(s.data[i]?.value ?? 0)}</title>
            </rect>
          {/each}
        {/each}
      {:else}
        {#each data as d, i (d.label)}
          <rect
            x={xAt(i)}
            y={y(d.value)}
            width={Math.max(2, barW - 3)}
            height={Math.max(1.5, h(d.value))}
            rx="5"
            fill={color}
            opacity="0.92"
          >
            <title>{d.label}: {format(d.value)}</title>
          </rect>
          {#if showValues && d.value !== 0}
            <text
              x={xCenter(i)}
              y={y(d.value) - 6}
              text-anchor="middle"
              font-size="10.5"
              font-weight="600"
              fill="var(--ec-text-2)"
            >
              {format(d.value)}
            </text>
          {/if}
        {/each}
      {/if}
    {:else}
      {#if multi}
        {#each series as s (s.name)}
          <polyline
            points={polylineOf(categories.map((_, i) => s.data[i]?.value ?? 0))}
            fill="none"
            stroke={s.color}
            stroke-width="2.5"
            stroke-linejoin="round"
            stroke-linecap="round"
          />
          {#each categories as cat, i (cat)}
            <circle
              cx={xCenter(i)}
              cy={y(s.data[i]?.value ?? 0)}
              r="3.5"
              fill="var(--ec-surface)"
              stroke={s.color}
              stroke-width="2.5"
            >
              <title>{cat} — {s.name}: {format(s.data[i]?.value ?? 0)}</title>
            </circle>
          {/each}
        {/each}
      {:else}
        <polygon points={areaOf(data.map((d) => d.value))} fill={color} opacity="0.12" />
        <polyline
          points={polylineOf(data.map((d) => d.value))}
          fill="none"
          stroke={color}
          stroke-width="2.5"
          stroke-linejoin="round"
          stroke-linecap="round"
        />
        {#each data as d, i (d.label)}
          <circle
            cx={xCenter(i)}
            cy={y(d.value)}
            r="4"
            fill="var(--ec-surface)"
            stroke={color}
            stroke-width="2.5"
          >
            <title>{d.label}: {format(d.value)}</title>
          </circle>
        {/each}
      {/if}
    {/if}

    {#each categories as cat, i (cat)}
      <text
        x={xCenter(i)}
        y={height - 10}
        text-anchor="middle"
        font-size="10.5"
        fill="var(--ec-text-3)"
      >
        {cat}
      </text>
    {/each}
  </svg>

  {#if showLegend && legend.length > 1}
    <div class="mt-2 flex flex-wrap items-center justify-center gap-4 text-[12.5px]" style="color:var(--ec-text-2)">
      {#each legend as l (l.label)}
        <span class="flex items-center gap-1.5">
          <span class="inline-block h-2.5 w-2.5 rounded-sm" style="background:{l.color}"></span>
          {l.label}
        </span>
      {/each}
    </div>
  {/if}
{/if}
