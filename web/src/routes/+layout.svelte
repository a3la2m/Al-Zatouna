<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import Toasts from '$lib/components/Toasts.svelte';
  import Alert from '$lib/components/Alert.svelte';
  import {
    sessionUser, sidebarCollapsed, sessionExpired,
    restoreSession, clearPersistedSession
  } from '$lib/stores/app';
  import { isTauri } from '$lib/env';
  import { invoke } from '$lib/api';

  let { children }: { children: import('svelte').Snippet } = $props();

  type UpdateInfo = {
    available: boolean;
    current: string;
    latest?: string;
    url?: string;
    notes?: string;
  };
  let updateInfo = $state<UpdateInfo | null>(null);
  let updateDismissed = $state(false);

  async function checkForUpdate() {
    if (!isTauri) return;
    try {
      const info = await invoke<UpdateInfo>('check_for_update');
      if (info?.available) {
        const dismissed = localStorage.getItem('ec.dismissed_update');
        if (dismissed !== info.latest) {
          updateInfo = info;
        }
      }
    } catch {
      // silent — update check is best-effort
    }
  }

  function dismissUpdate() {
    if (updateInfo?.latest) localStorage.setItem('ec.dismissed_update', updateInfo.latest);
    updateDismissed = true;
  }

  async function openUpdate() {
    const url = updateInfo?.url || 'https://github.com/a3la2m/Al-Zatouna/releases/latest';
    try {
      await invoke('open_release_page', { url });
    } catch {
      window.open(url, '_blank');
    }
  }

  // Restore a remembered session on startup before any guard runs.
  onMount(() => {
    restoreSession();
    void checkForUpdate();

    // Recover from accidental pinch/ctrl zoom and prevent it from getting stuck.
    if (isTauri) {
      invoke('window_reset_zoom').catch(() => {});
    }
    const resetZoom = () => {
      if (isTauri) invoke('window_reset_zoom').catch(() => {});
    };
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      const k = e.key;
      if (k === '0' || k === 'Insert') {
        e.preventDefault();
        resetZoom();
      } else if (k === '+' || k === '-' || k === '=' || k === '_') {
        // block browser/webview zoom shortcuts — UI is a fixed desktop layout
        e.preventDefault();
      }
    };
    const onWheel = (e: WheelEvent) => {
      if (e.ctrlKey || e.metaKey) e.preventDefault();
    };
    const blockGesture = (e: Event) => e.preventDefault();
    window.addEventListener('keydown', onKey);
    window.addEventListener('wheel', onWheel, { passive: false });
    document.addEventListener('gesturestart', blockGesture as EventListener, { passive: false } as any);
    document.addEventListener('gesturechange', blockGesture as EventListener, { passive: false } as any);
    document.addEventListener('gestureend', blockGesture as EventListener, { passive: false } as any);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('wheel', onWheel);
      document.removeEventListener('gesturestart', blockGesture as EventListener);
      document.removeEventListener('gesturechange', blockGesture as EventListener);
      document.removeEventListener('gestureend', blockGesture as EventListener);
    };
  });

  // A logged-out user must never see a protected page (e.g. after logout from
  // /teachers, or navigating directly to a deep link). Send them to the login.
  $effect(() => {
    if (!$sessionUser && page.url.pathname !== '/') {
      goto('/');
    }
  });

  function dismissExpired() {
    sessionExpired.set(false);
    clearPersistedSession();
  }
</script>

{#if $sessionExpired}
  <div class="fixed left-1/2 top-4 z-[4000] w-[min(520px,92vw)] -translate-x-1/2">
    <Alert kind="warn" onClose={dismissExpired}>
      انتهت الجلسة — أعد تسجيل الدخول للمتابعة.
    </Alert>
  </div>
{/if}

{#if updateInfo?.available && !updateDismissed}
  <div class="fixed bottom-4 left-4 z-[4000] w-[min(420px,calc(100vw-2rem))]" dir="rtl">
    <div class="rounded-2xl border shadow-2xl p-4" style="background:var(--ec-surface); border-color:color-mix(in srgb, var(--ec-olive) 35%, var(--ec-border));">
      <div class="flex items-start gap-3">
        <div class="mt-0.5 flex h-10 w-10 flex-none items-center justify-center rounded-xl text-lg" style="background:var(--ec-olive-soft, #edf5e3); color:var(--ec-olive, #4d7a2f);">⬆</div>
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2">
            <h3 class="text-sm font-bold" style="color:var(--ec-text)">تحديث جديد متوفر</h3>
            <span class="rounded-full px-2 py-0.5 text-[11px] font-bold" style="background:var(--ec-olive-soft, #edf5e3); color:var(--ec-olive, #4d7a2f);">v{updateInfo.latest}</span>
          </div>
          <p class="mt-1 text-xs leading-5" style="color:var(--ec-text-2)">
            إصدار جديد من الزيتونة متاح (الإصدار الحالي v{updateInfo.current}). يُنصح بالتحديث للحصول على أحدث التحسينات والإصلاحات.
          </p>
          {#if updateInfo.notes}
            <p class="mt-2 line-clamp-2 text-[11px] leading-5" style="color:var(--ec-text-3)">{updateInfo.notes}</p>
          {/if}
          <div class="mt-3 flex items-center gap-2">
            <button
              class="rounded-lg px-3 py-1.5 text-xs font-bold text-white transition hover:opacity-90"
              style="background:var(--ec-olive, #4d7a2f);"
              onclick={openUpdate}
            >تحميل التحديث</button>
            <button
              class="rounded-lg px-3 py-1.5 text-xs font-semibold transition hover:bg-black/5"
              style="color:var(--ec-text-2);"
              onclick={dismissUpdate}
            >لاحقاً</button>
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}

{#if $sessionUser}
  <div class="ec-shell">
    <Sidebar />
    <main
      class="ec-main"
      style="margin-right: {$sidebarCollapsed ? 68 : 264}px;"
    >
      <div class="ec-page">
        {@render children()}
      </div>
    </main>
  </div>
{:else}
  <div class="flex min-h-screen items-center justify-center p-6">
    {@render children()}
  </div>
{/if}

<Toasts />

{#if !isTauri}
  <div
    class="fixed bottom-3 left-3 z-40 rounded-lg px-3 py-1.5 text-xs font-semibold shadow-md"
    style="background:var(--ec-amber-soft); color:var(--ec-amber); border:1px solid color-mix(in srgb, var(--ec-amber) 30%, transparent)"
  >
    وضع المعاينة في المتصفح — الدخول الفعلي يعمل داخل التطبيق
  </div>
{/if}
