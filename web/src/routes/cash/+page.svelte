<script lang="ts">
  import { onMount } from 'svelte';
  import { sessionToken } from '$lib/stores/app';
  import { invoke } from '$lib/api';
  import { get } from 'svelte/store';

  let registerId = $state<number | null>(null);
  let error = $state('');
  let amount = $state(0);
  let balance = $state(0);
  let movements = $state<{ kind: string; direction: string; amount_minor: number; balance_after_minor: number }[]>([]);

  async function open() {
    const token = get(sessionToken);
    if (!token) return;
    error = '';
    try {
      registerId = await invoke<number>('cash_open_register', { token, branch_id: 1, code: 'MAIN' });
    } catch (e) { console.error(e); error = (e as any)?.message || 'خطأ'; }
  }

  async function record(dir: string) {
    const token = get(sessionToken);
    if (!token || !registerId) return;
    error = '';
    try {
      const m = await invoke<any>('cash_record_movement', {
        token, register_id: registerId, kind: 'receipt', direction: dir, amount_minor: amount, ref_type: 'manual'
      });
      balance = m.balance_after_minor;
      movements = [m, ...movements];
      amount = 0;
    } catch (e) { console.error(e); error = (e as any)?.message || 'خطأ'; }
  }

  onMount(open);
</script>

<section>
  <h1 class="mb-4 text-2xl font-bold text-[var(--ec-navy)]">الصندوق</h1>
  {#if error}<p class="mb-3 rounded bg-red-50 px-3 py-2 text-sm text-red-700">{error}</p>{/if}
  {#if registerId}
    <div class="mb-4 rounded-xl bg-[var(--ec-surface)] p-4 shadow-sm">
      <p class="mb-2 text-sm">الرصيد الحالي: <strong>{balance}</strong> (minor)</p>
      <div class="flex gap-2">
        <input type="number" bind:value={amount} class="w-32 rounded-md border border-[var(--ec-border)] px-3 py-2" dir="ltr" />
        <button class="rounded-md bg-[var(--ec-teal)] px-4 py-2 text-sm font-semibold text-white" onclick={() => record('in')}>قبض</button>
        <button class="rounded-md bg-red-600 px-4 py-2 text-sm font-semibold text-white" onclick={() => record('out')}>صرف</button>
      </div>
    </div>
    {#if movements.length}
      <div class="overflow-x-auto rounded-xl bg-[var(--ec-surface)] shadow-sm">
        <table class="w-full text-sm">
          <thead class="bg-[var(--ec-navy)] text-white"><tr><th class="px-3 py-2 text-right">النوع</th><th class="px-3 py-2 text-right">الاتجاه</th><th class="px-3 py-2 text-right">المبلغ</th><th class="px-3 py-2 text-right">الرصيد بعد</th></tr></thead>
          <tbody>{#each movements as m}<tr class="border-t border-[var(--ec-border)]"><td class="px-3 py-2">{m.kind}</td><td class="px-3 py-2">{m.direction}</td><td class="px-3 py-2">{m.amount_minor}</td><td class="px-3 py-2">{m.balance_after_minor}</td></tr>{/each}</tbody>
        </table>
      </div>
    {/if}
  {:else}
    <p class="text-sm text-[var(--ec-text)]/60">جارٍ فتح الصندوق…</p>
  {/if}
</section>
