<script lang="ts">
  import { onMount } from 'svelte';
  import { sessionToken } from '$lib/stores/app';
  import { invoke } from '$lib/api';
  import { get } from 'svelte/store';

  type V = { id: number; voucher_no: number; amount_minor: number; status: string; voucher_date: string };
  let vouchers = $state<V[]>([]);
  let loading = $state(false);
  let error = $state('');

  async function load() {
    const token = get(sessionToken);
    if (!token) return;
    loading = true; error = '';
    try { vouchers = await invoke<V[]>('vouchers_list_receipts', { token }); }
    catch (e) { console.error(e); error = (e as any)?.message || 'خطأ'; }
    finally { loading = false; }
  }

  onMount(load);
</script>

<section>
  <h1 class="mb-4 text-2xl font-bold text-[var(--ec-navy)]">السندات (قبض)</h1>
  {#if error}<p class="mb-3 rounded bg-red-50 px-3 py-2 text-sm text-red-700">{error}</p>{/if}
  {#if loading}<p class="text-sm text-[var(--ec-text)]/60">جارٍ التحميل…</p>
  {:else if vouchers.length === 0}<p class="text-sm text-[var(--ec-text)]/60">لا توجد سندات بعد.</p>
  {:else}
    <div class="overflow-x-auto rounded-xl bg-[var(--ec-surface)] shadow-sm">
      <table class="w-full text-sm">
        <thead class="bg-[var(--ec-navy)] text-white"><tr><th class="px-3 py-2 text-right">#</th><th class="px-3 py-2 text-right">الرقم</th><th class="px-3 py-2 text-right">المبلغ</th><th class="px-3 py-2 text-right">الحالة</th><th class="px-3 py-2 text-right">التاريخ</th></tr></thead>
        <tbody>{#each vouchers as v}<tr class="border-t border-[var(--ec-border)]"><td class="px-3 py-2">{v.id}</td><td class="px-3 py-2">{v.voucher_no}</td><td class="px-3 py-2">{v.amount_minor}</td><td class="px-3 py-2">{v.status}</td><td class="px-3 py-2">{v.voucher_date}</td></tr>{/each}</tbody>
      </table>
    </div>
  {/if}
</section>
