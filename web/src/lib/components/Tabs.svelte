<script lang="ts">
  export type TabItem = { id: string; label: string; badge?: number };

  let {
    items,
    value = $bindable(''),
    onchange
  }: {
    items: TabItem[];
    value?: string;
    onchange?: (id: string) => void;
  } = $props();
</script>

<div class="ec-tabs" role="tablist">
  {#each items as item (item.id)}
    <button
      type="button"
      role="tab"
      aria-selected={value === item.id}
      class="ec-tab"
      class:is-active={value === item.id}
      onclick={() => {
        value = item.id;
        onchange?.(item.id);
      }}
    >
      {item.label}
      {#if item.badge !== undefined && item.badge > 0}
        <span
          class="ms-1 rounded-full px-1.5 py-0.5 text-[11px]"
          style="background:var(--ec-blue-soft); color:var(--ec-blue)"
        >
          {item.badge}
        </span>
      {/if}
    </button>
  {/each}
</div>
