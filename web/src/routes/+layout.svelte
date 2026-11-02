<script lang="ts">
  import '../app.css';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import { sessionUser } from '$lib/stores/app';
  import { isTauri } from '$lib/env';
</script>

{#if $sessionUser}
  <div class="flex h-screen w-full bg-[var(--ec-bg)] text-[var(--ec-text)]">
    <Sidebar />
    <main class="mr-[260px] flex-1 overflow-auto p-6 transition-all">
      <slot />
    </main>
  </div>
{:else}
  <div class="flex min-h-screen items-center justify-center bg-[var(--ec-bg)] p-6">
    <slot />
  </div>
{/if}

{#if !isTauri}
  <div class="fixed bottom-2 left-2 rounded bg-amber-500 px-2 py-1 text-xs text-white shadow">
    وضع المعاينة في المتصفح — البيانات المباشرة غير متاحة خارج التطبيق
  </div>
{/if}
