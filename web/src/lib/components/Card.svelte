<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title = '',
    actions,
    pad = true,
    accent,
    classes = '',
    children
  }: {
    title?: string;
    actions?: Snippet;
    pad?: boolean;
    accent?: string;
    classes?: string;
    children?: Snippet;
  } = $props();
</script>

<section class="ec-card {classes}" style={accent ? `--ec-accent:${accent}` : undefined}>
  {#if title}
    <div class="ec-card-head">
      <h2 class="ec-card-title">{title}</h2>
      {#if actions}
        <div class="flex items-center gap-2">{@render actions()}</div>
      {/if}
    </div>
  {/if}
  <div class={pad ? 'ec-card-pad' : ''}>
    {@render children?.()}
  </div>
</section>
