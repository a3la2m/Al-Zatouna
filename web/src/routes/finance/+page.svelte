<script lang="ts">
  import { sessionToken } from '$lib/stores/app';
  import { invoke } from '$lib/api';
  import { get } from 'svelte/store';

  let enrollmentId = $state(0);
  let amount = $state(0);
  let key = $state('');
  let error = $state('');
  let result = $state('');

  async function record() {
    const token = get(sessionToken);
    if (!token) return;
    error = ''; result = '';
    if (!key) key = crypto.randomUUID();
    try {
      const r = await invoke<any>('payments_record', {
        token, enrollment_id: enrollmentId, amount_minor: amount, idempotency_key: key
      });
      result = `سند #${r.voucher_id} — ${r.total_minor} minor`;
    } catch (e) { console.error(e); error = (e as any)?.message || 'خطأ'; }
  }
</script>

<section>
  <h1 class="mb-4 text-2xl font-bold text-[var(--ec-navy)]">المحرك المالي — تسجيل دفعة</h1>
  {#if error}<p class="mb-3 rounded bg-red-50 px-3 py-2 text-sm text-red-700">{error}</p>{/if}
  {#if result}<p class="mb-3 rounded bg-green-50 px-3 py-2 text-sm text-green-700">{result}</p>{/if}
  <form class="space-y-3 rounded-xl bg-[var(--ec-surface)] p-4 shadow-sm" onsubmit={(e) => { e.preventDefault(); record(); }}>
    <div><label class="mb-1 block text-sm font-semibold" for="eid">رقم التسجيل (enrollment_id)</label><input id="eid" type="number" bind:value={enrollmentId} class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" dir="ltr" /></div>
    <div><label class="mb-1 block text-sm font-semibold" for="amt">المبلغ (minor)</label><input id="amt" type="number" bind:value={amount} class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" dir="ltr" /></div>
    <div><label class="mb-1 block text-sm font-semibold" for="key">مفتاح منع التكرار (idempotency key)</label><input id="key" bind:value={key} placeholder="اتركه فارغاً لتوليد تلقائي" class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" dir="ltr" /></div>
    <button type="submit" class="rounded-md bg-[var(--ec-teal)] px-4 py-2 text-sm font-semibold text-white">تسجيل دفعة</button>
  </form>
</section>
