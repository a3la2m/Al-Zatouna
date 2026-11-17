<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    kind = 'info',
    onClose,
    children
  }: {
    kind?: 'info' | 'error' | 'ok' | 'warn';
    onClose?: () => void;
    children?: Snippet;
  } = $props();

  const ICON = { info: 'ℹ', error: '✕', ok: '✓', warn: '!' };
  const CLS = {
    info: 'ec-alert-info',
    error: 'ec-alert-error',
    ok: 'ec-alert-ok',
    warn: 'ec-alert-warn'
  };
</script>

<div class="ec-alert {CLS[kind]}" role={kind === 'error' ? 'alert' : 'status'}>
  <span aria-hidden="true" class="font-bold">{ICON[kind]}</span>
  <div class="flex-1">{@render children?.()}</div>
  {#if onClose}
    <button type="button" class="opacity-70 hover:opacity-100" aria-label="إخفاء" onclick={onClose}>
      ✕
    </button>
  {/if}
</div>
