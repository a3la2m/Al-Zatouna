<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '$lib/api';

  let info = $state<any>(null);
  let error = $state('');

  onMount(async () => {
    try { info = await invoke('app_info'); }
    catch (e) { console.error(e); error = (e as any)?.message || 'خطأ'; }
  });
</script>

<section>
  <h1 class="mb-4 text-2xl font-bold text-[var(--ec-navy)]">الإعدادات</h1>
  {#if error}<p class="mb-3 rounded bg-red-50 px-3 py-2 text-sm text-red-700">{error}</p>{/if}
  {#if info}
    <div class="space-y-2 rounded-xl bg-[var(--ec-surface)] p-4 shadow-sm text-sm">
      <p><strong>التطبيق:</strong> {info.app}</p>
      <p><strong>الإصدار:</strong> {info.version}</p>
      <p><strong>مجلد البيانات:</strong> <span dir="ltr">{info.data_dir}</span></p>
      <p><strong>الحسابات:</strong> {info.accounts_seeded}</p>
      <p><strong>المستخدمون:</strong> {info.users}</p>
    </div>
  {:else}
    <p class="text-sm text-[var(--ec-text)]/60">جارٍ التحميل…</p>
  {/if}
</section>
