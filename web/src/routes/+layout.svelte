<script lang="ts">
  import '../app.css';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import { isTauri } from '$lib/env';
  import { sidebarCollapsed } from '$lib/stores/app';
</script>

<div class="flex h-screen w-full bg-[var(--ec-bg)] text-[var(--ec-text)]">
  <Sidebar />
  <main
    class="m-0 flex-1 overflow-auto p-6 transition-all"
    class:mr-[260px]={!$sidebarCollapsed}
    class:mr-[64px]={$sidebarCollapsed}
  >
    <slot />
  </main>
</div>

{#if !isTauri}
  <div
    class="fixed bottom-2 left-2 rounded bg-amber-500 px-2 py-1 text-xs text-white shadow"
  >
    وضع المعاينة في المتصفح — البيانات المباشرة غير متاحة خارج التطبيق
  </div>
{/if}
