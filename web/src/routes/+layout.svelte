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

  // Restore a remembered session on startup before any guard runs.
  onMount(() => {
    restoreSession();

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
