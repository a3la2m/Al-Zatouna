<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    label,
    hint = '',
    error = '',
    required = false,
    id = '',
    children
  }: {
    label: string;
    hint?: string;
    error?: string;
    required?: boolean;
    id?: string;
    children?: Snippet;
  } = $props();
</script>

<div class="ec-field">
  <label class="ec-label" for={id}>
    {label}{#if required}<span class="req">*</span>{/if}
  </label>
  {@render children?.()}
  {#if error}
    <span class="ec-err">{error}</span>
  {:else if hint}
    <span class="ec-hint">{hint}</span>
  {/if}
</div>
