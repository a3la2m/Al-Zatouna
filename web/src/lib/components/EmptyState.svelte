<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    icon = '📭',
    title = 'لا توجد بيانات',
    hint = '',
    action
  }: {
    icon?: string;
    title?: string;
    hint?: string;
    action?: Snippet;
  } = $props();
</script>

<div class="ec-empty">
  <div class="ec-empty-icon" aria-hidden="true">{icon}</div>
  <div class="ec-empty-title">{title}</div>
  {#if hint}
    <div class="ec-empty-hint">{hint}</div>
  {/if}
  {#if action}
    <div class="mt-2">{@render action()}</div>
  {/if}
</div>
