<script lang="ts">
  let {
    label,
    value,
    foot = '',
    icon = '',
    accent = 'var(--ec-blue)',
    onclick
  }: {
    label: string;
    value: string;
    foot?: string;
    icon?: string;
    accent?: string;
    onclick?: () => void;
  } = $props();
</script>

{#snippet body()}
  <div class="ec-stat-label">
    {#if icon}<span aria-hidden="true">{icon}</span>{/if}
    {label}
  </div>
  <div class="ec-stat-value">{value}</div>
  {#if foot}
    <div class="ec-stat-foot">{foot}</div>
  {/if}
{/snippet}

<!--
  A clickable card is a real <button>, not a div with a keydown handler —
  that keeps it focusable, Enter/Space-operable and announced as a button
  without any lint suppressions. Non-clickable cards stay plain divs.
-->
{#if onclick}
  <button type="button" class="ec-stat" style="--ec-accent:{accent}" {onclick}>
    {@render body()}
  </button>
{:else}
  <div class="ec-stat" style="--ec-accent:{accent}">
    {@render body()}
  </div>
{/if}
