<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title,
    subtitle = '',
    children
  }: { title: string; subtitle?: string; children?: Snippet } = $props();
</script>

<header class="ec-page-head">
  <div class="min-w-0">
    <h1 class="ec-title">{title}</h1>
    {#if subtitle}
      <p class="ec-sub">{subtitle}</p>
    {/if}
  </div>
  {#if children}
    <div class="flex flex-wrap items-center gap-2">{@render children()}</div>
  {/if}
</header>
