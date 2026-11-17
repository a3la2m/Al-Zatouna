<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { sessionToken } from '$lib/stores/app';
  import { notificationsApi, type AppNotification } from '$lib/api/app';
  import { goto } from '$app/navigation';
  import { ago } from '$lib/format';

  let open = $state(false);
  let items = $state<AppNotification[]>([]);
  let unseen = $state(0);
  let timer: ReturnType<typeof setInterval>;

  async function refresh() {
    const token = get(sessionToken);
    if (!token) return;
    try {
      const [list, count] = await Promise.all([
        notificationsApi.list(token, 60), notificationsApi.unseen(token)
      ]);
      items = list;
      unseen = count;
    } catch { /* ignore — not logged in yet */ }
  }

  onMount(() => {
    refresh();
    timer = setInterval(refresh, 15000); // poll every 15s
  });
  onDestroy(() => clearInterval(timer));

  async function toggle() {
    open = !open;
    if (open) await refresh();
  }

  async function markAll() {
    const token = get(sessionToken);
    if (!token) return;
    try {
      await notificationsApi.markAll(token);
      await refresh();
    } catch { /* ignore */ }
  }

  async function clearAll() {
    const token = get(sessionToken);
    if (!token) return;
    if (!confirm('حذف كل الإشعارات من القائمة؟')) return;
    try {
      await notificationsApi.clear(token);
      await refresh();
    } catch { /* ignore */ }
  }

  async function goTo(n: AppNotification) {
    open = false;
    const token = get(sessionToken);
    if (token && !n.seen) {
      try { await notificationsApi.markSeen(token, n.id); } catch { /* ignore */ }
    }
    unseen = Math.max(0, unseen - (n.seen ? 0 : 1));
    if (n.ref_type === 'service_request') goto('/settings?tab=telegram');
    else if (n.ref_type === 'receipt_voucher') goto('/vouchers');
  }
</script>

<div class="relative">
  <button
    type="button"
    class="bell-btn"
    aria-label="الإشعارات"
    title="الإشعارات"
    onclick={toggle}
  >
    <span aria-hidden="true" class="text-[17px]">🔔</span>
    {#if unseen > 0}
      <span class="bell-badge">{unseen > 9 ? '9+' : unseen}</span>
    {/if}
  </button>

  {#if open}
    <div class="bell-panel" role="dialog" aria-label="الإشعارات">
      <div class="flex items-center justify-between border-b px-4 py-3" style="border-color:var(--ec-border)">
        <span class="font-bold text-[14px]">الإشعارات</span>
        <span class="flex items-center gap-3">
          {#if unseen > 0}
            <button type="button" class="text-[12px] font-semibold" style="color:var(--ec-blue)" onclick={markAll}>
              تحديد الكل كمقروء
            </button>
          {/if}
          {#if items.length}
            <button type="button" class="text-[12px] font-semibold" style="color:var(--ec-red)" onclick={clearAll}>
              حذف الكل
            </button>
          {/if}
        </span>
      </div>
      <div class="max-h-[380px] overflow-y-auto">
        {#if !items.length}
          <div class="py-10 text-center text-[13px]" style="color:var(--ec-text-3)">لا توجد إشعارات بعد</div>
        {:else}
          {#each items as n (n.id)}
            <button
              type="button"
              class="flex w-full gap-3 border-b px-4 py-3 text-start hover:bg-[var(--ec-surface-2)]"
              style="border-color:var(--ec-border); background:{n.seen ? 'transparent' : 'var(--ec-blue-soft)'}"
              onclick={() => goTo(n)}
            >
              <span class="text-[18px]">{n.kind === 'payment' ? '💰' : n.kind === 'attendance' ? '📋' : n.kind === 'service_request' ? '📢' : '🔔'}</span>
              <span class="min-w-0 flex-1">
                <span class="block text-[13px] font-bold">{n.title}</span>
                <span class="block text-[12px]" style="color:var(--ec-text-2)">{n.body}</span>
                <span class="block text-[11px]" style="color:var(--ec-text-3)">{ago(n.created_at)}</span>
              </span>
              {#if !n.seen}<span class="mt-1 h-2 w-2 flex-none rounded-full" style="background:var(--ec-blue)"></span>{/if}
            </button>
          {/each}
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .bell-btn {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.1);
    cursor: pointer;
  }
  .bell-btn:hover { background: rgba(255, 255, 255, 0.16); }
  .bell-badge {
    position: absolute;
    top: -5px;
    left: -5px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--ec-amber);
    color: #fff;
    font-size: 10px;
    font-weight: 700;
    line-height: 16px;
    text-align: center;
  }
  .bell-panel {
    position: absolute;
    bottom: 42px;
    right: 0;
    width: 340px;
    z-index: 100;
    border-radius: 12px;
    background: var(--ec-surface);
    border: 1px solid var(--ec-border);
    box-shadow: 0 12px 40px -8px rgba(0, 0, 0, 0.35);
    overflow: hidden;
  }
</style>
