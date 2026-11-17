<script lang="ts">
  import { toasts, dismiss } from '$lib/toasts';

  const STYLE = {
    ok: { bg: 'var(--ec-teal-soft)', fg: 'var(--ec-teal)', icon: '✓' },
    error: { bg: 'var(--ec-red-soft)', fg: 'var(--ec-red)', icon: '✕' },
    info: { bg: 'var(--ec-blue-soft)', fg: 'var(--ec-blue)', icon: 'ℹ' }
  };
</script>

<div
  class="pointer-events-none fixed inset-x-0 top-4 z-[60] flex flex-col items-center gap-2 px-4"
  aria-live="polite"
>
  {#each $toasts as t (t.id)}
    <div
      class="pointer-events-auto flex max-w-[560px] items-center gap-2.5 rounded-xl px-4 py-2.5 text-sm font-semibold shadow-lg"
      style="background:{STYLE[t.kind].bg}; color:{STYLE[t.kind].fg}; border:1px solid color-mix(in srgb, {STYLE[t.kind].fg} 22%, transparent); animation: ec-fade 180ms var(--ec-ease) both;"
      role="status"
    >
      <span aria-hidden="true" class="font-bold">{STYLE[t.kind].icon}</span>
      <span class="flex-1">{t.text}</span>
      <button
        type="button"
        class="opacity-60 hover:opacity-100"
        aria-label="إغلاق"
        onclick={() => dismiss(t.id)}
      >
        ✕
      </button>
    </div>
  {/each}
</div>
