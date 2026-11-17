<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { sessionToken } from '$lib/stores/app';
  import { notificationsApi, type AppNotification } from '$lib/api/app';
  import { goto } from '$app/navigation';
  import { ago } from '$lib/format';
  import Logo from '$lib/components/Logo.svelte';

  let { variant = 'bell' }: { variant?: 'bell' | 'logo' } = $props();

  let open = $state(false);
  let items = $state<AppNotification[]>([]);
  let unseen = $state(0);
  let timer: ReturnType<typeof setInterval>;
  let root: HTMLDivElement | undefined = $state();

  function onDocPointerDown(e: PointerEvent) {
    if (!open || !root) return;
    if (!root.contains(e.target as Node)) open = false;
  }

  function onDocKeydown(e: KeyboardEvent) {
    if (open && e.key === 'Escape') open = false;
  }

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
    document.addEventListener('pointerdown', onDocPointerDown);
    document.addEventListener('keydown', onDocKeydown);
  });
  onDestroy(() => {
    clearInterval(timer);
    document.removeEventListener('pointerdown', onDocPointerDown);
    document.removeEventListener('keydown', onDocKeydown);
  });

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

<div class="relative" bind:this={root}>
  <button
    type="button"
    class={variant === 'logo' ? 'logo-btn' : 'bell-btn'}
    aria-label="الإشعارات"
    title={unseen > 0 ? `الإشعارات (${unseen} جديد)` : 'الإشعارات'}
    onclick={toggle}
  >
    {#if variant === 'logo'}
      <Logo size={30} variant="icon" light />
    {:else}
      <span aria-hidden="true" class="text-[17px]">🔔</span>
    {/if}
    {#if unseen > 0}
      <span class={variant === 'logo' ? 'logo-badge' : 'bell-badge'}>{unseen > 9 ? '9+' : unseen}</span>
    {/if}
  </button>

  {#if open}
    <div class="bell-panel {variant === 'logo' ? 'panel-below' : 'panel-above'}" role="dialog" aria-label="الإشعارات">
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

  /* logo-as-notification-button (sidebar header) */
  .logo-btn {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 42px;
    height: 42px;
    padding: 0;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.08);
    cursor: pointer;
    transition: background 0.15s, transform 0.12s, box-shadow 0.15s;
  }
  .logo-btn:hover {
    background: rgba(255, 255, 255, 0.16);
    box-shadow: 0 0 0 3px rgba(130, 191, 75, 0.18);
  }
  .logo-btn:active { transform: scale(0.96); }
  .logo-badge {
    position: absolute;
    top: -4px;
    left: -4px;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 9px;
    background: var(--ec-red);
    color: #fff;
    font-size: 10px;
    font-weight: 700;
    line-height: 18px;
    text-align: center;
    border: 2px solid #24370f;
    box-shadow: 0 2px 6px rgba(0,0,0,.25);
  }

  .bell-panel {
    position: absolute;
    width: 340px;
    z-index: 100;
    border-radius: 12px;
    background: var(--ec-surface);
    border: 1px solid var(--ec-border);
    box-shadow: 0 12px 40px -8px rgba(0, 0, 0, 0.35);
    overflow: hidden;
  }
  .panel-above { bottom: 42px; right: 0; }
  .panel-below { top: calc(100% + 10px); right: 0; }
</style>
