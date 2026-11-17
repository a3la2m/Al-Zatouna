<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    open = false,
    title,
    subtitle = '',
    width = '560px',
    onClose,
    children,
    footer
  }: {
    open?: boolean;
    title: string;
    subtitle?: string;
    width?: string;
    onClose: () => void;
    children?: Snippet;
    footer?: Snippet;
  } = $props();

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && open) onClose();
  }

  /// Move the overlay to <body> so position:fixed is relative to the viewport,
  /// never to a transformed ancestor (e.g. the animated .ec-page).
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return {
      destroy() {
        node.remove();
      }
    };
  }
</script>

<svelte:window onkeydown={onKey} />

{#if open}
  <div
    use:portal
    class="fixed inset-0 z-[9999] flex items-center justify-center p-4 sm:p-6"
    style="animation: ec-fade 180ms var(--ec-ease) both;"
  >
    <!-- Full-page backdrop -->
    <button
      type="button"
      class="absolute inset-0 cursor-default border-0 p-0"
      style="background: linear-gradient(180deg, rgba(8, 14, 28, 0.78), rgba(8, 14, 28, 0.86)); backdrop-filter: blur(6px) saturate(140%); -webkit-backdrop-filter: blur(6px) saturate(140%);"
      aria-label="إغلاق النافذة"
      onclick={onClose}
    ></button>

    <!-- Dialog card -->
    <div
      class="relative flex w-full flex-col overflow-hidden rounded-2xl"
      style="max-width: min({width}, 100%); max-height: calc(100vh - 2rem); background: var(--ec-surface); border: 1px solid var(--ec-border); box-shadow: 0 24px 60px -12px rgba(0, 0, 0, 0.55), 0 8px 24px -8px rgba(0, 0, 0, 0.35); animation: ec-pop 220ms var(--ec-ease) both;"
      role="dialog"
      aria-modal="true"
      aria-label={title}
      tabindex="-1"
    >
      <div class="flex flex-none items-start justify-between gap-3 border-b px-5 py-4" style="border-color: var(--ec-border); background: var(--ec-surface-2);">
        <div class="min-w-0">
          <h2 class="ec-card-title">{title}</h2>
          {#if subtitle}
            <p class="mt-0.5 text-xs" style="color: var(--ec-text-3)">{subtitle}</p>
          {/if}
        </div>
        <button
          class="ec-btn ec-btn-ghost ec-btn-sm flex-none"
          type="button"
          aria-label="إغلاق"
          onclick={onClose}
        >
          ✕
        </button>
      </div>

      <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4">
        {@render children?.()}
      </div>

      {#if footer}
        <div
          class="flex flex-none items-center justify-end gap-2 border-t px-5 py-3"
          style="border-color: var(--ec-border); background: var(--ec-surface-2)"
        >
          {@render footer()}
        </div>
      {/if}
    </div>
  </div>
{/if}
