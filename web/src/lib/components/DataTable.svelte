<script lang="ts" generics="T">
  import type { Snippet } from 'svelte';
  import type { Column } from '$lib/table';
  import EmptyState from './EmptyState.svelte';

  let {
    columns,
    rows,
    rowKey,
    loading = false,
    emptyTitle = 'لا توجد بيانات',
    emptyHint = '',
    emptyIcon = '📭',
    emptyAction,
    actions,
    onRowClick,
    caption = ''
  }: {
    columns: Column<T>[];
    rows: T[];
    rowKey: (row: T) => string | number;
    loading?: boolean;
    emptyTitle?: string;
    emptyHint?: string;
    emptyIcon?: string;
    emptyAction?: Snippet;
    actions?: Snippet<[T]>;
    onRowClick?: (row: T) => void;
    caption?: string;
  } = $props();
</script>

<div class="ec-table-wrap">
  <table class="ec-table">
    {#if caption}
      <caption class="sr-only">{caption}</caption>
    {/if}
    <thead>
      <tr>
        {#each columns as col (col.key)}
          <th
            scope="col"
            style={col.width ? `width:${col.width}` : undefined}
            class={col.align === 'end' || col.num ? 'text-left' : ''}
          >
            {col.label}
          </th>
        {/each}
        {#if actions}
          <th scope="col" class="text-left">إجراءات</th>
        {/if}
      </tr>
    </thead>
    <tbody>
      {#if loading}
        {#each Array(6) as _, i (i)}
          <tr>
            {#each columns as col (col.key)}
              <td><div class="ec-skel h-4 w-full"></div></td>
            {/each}
            {#if actions}<td><div class="ec-skel h-4 w-16"></div></td>{/if}
          </tr>
        {/each}
      {:else}
        {#each rows as row (rowKey(row))}
          <tr
            class={onRowClick ? 'cursor-pointer' : ''}
            onclick={onRowClick ? () => onRowClick(row) : undefined}
          >
            {#each columns as col (col.key)}
              <td class="{col.num ? 'num' : ''} {col.align === 'center' ? 'text-center' : ''}">
                {#if col.render}
                  {@render col.render(row)}
                {:else if col.format}
                  {col.format((row as Record<string, unknown>)[col.key], row)}
                {:else}
                  {(row as Record<string, unknown>)[col.key] ?? '—'}
                {/if}
              </td>
            {/each}
            {#if actions}
              <td>
                <div class="flex items-center justify-start gap-2">
                  {@render actions(row)}
                </div>
              </td>
            {/if}
          </tr>
        {/each}
      {/if}
    </tbody>
  </table>

  {#if !loading && rows.length === 0}
    <EmptyState
      icon={emptyIcon}
      title={emptyTitle}
      hint={emptyHint}
      action={emptyAction}
    />
  {/if}
</div>
