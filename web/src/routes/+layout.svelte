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

  let { children }: { children: import('svelte').Snippet } = $props();

  // Restore a remembered session on startup before any guard runs.
  onMount(() => { restoreSession(); });

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
