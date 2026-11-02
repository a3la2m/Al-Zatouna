<script lang="ts">
  import { onMount } from 'svelte';
  import { sessionToken } from '$lib/stores/app';
  import { invoke } from '$lib/api';
  import { get } from 'svelte/store';

  type Expense = { id: number; amount_minor: number; beneficiary: string; description: string; status: string; category_id: number };
  let items = $state<Expense[]>([]);
  let loading = $state(false);
  let error = $state('');
  let showForm = $state(false);
  let amount = $state(0);
  let beneficiary = $state('');
  let description = $state('');
  let categoryId = $state(1);
  let accountId = $state(1);

  async function load() {
    // no list command; show empty (backend has create/approve only)
    loading = false;
  }

  async function create(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    error = '';
    try {
      await invoke('expenses_create', {
        token, branch_id: 1, category_id: categoryId, expense_date: new Date().toISOString().slice(0,10),
        amount_minor: amount, beneficiary, description, method: 'cash', account_id: accountId
      });
      amount = 0; beneficiary = ''; description = ''; showForm = false;
    } catch (e) { console.error(e); error = (e as any)?.message || 'خطأ'; }
  }

  async function approve(id: number) {
    const token = get(sessionToken);
    if (!token) return;
    try { await invoke('expenses_approve', { token, id }); }
    catch (e) { console.error(e); error = (e as any)?.message || 'خطأ'; }
  }

  onMount(load);
</script>

<section>
  <div class="mb-4 flex items-center justify-between">
    <h1 class="text-2xl font-bold text-[var(--ec-navy)]">المصروفات</h1>
    <button class="rounded-md bg-[var(--ec-blue)] px-4 py-2 text-sm font-semibold text-white hover:opacity-90" onclick={() => (showForm = !showForm)}>
      {showForm ? 'إلغاء' : 'إضافة مصروف'}
    </button>
  </div>
  {#if error}<p class="mb-3 rounded bg-red-50 px-3 py-2 text-sm text-red-700">{error}</p>{/if}
  {#if showForm}
    <form class="mb-6 space-y-3 rounded-xl bg-[var(--ec-surface)] p-4 shadow-sm" onsubmit={create}>
      <div><label class="mb-1 block text-sm font-semibold" for="a">المبلغ</label><input id="a" type="number" bind:value={amount} required class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" dir="ltr" /></div>
      <div><label class="mb-1 block text-sm font-semibold" for="b">المستفيد</label><input id="b" bind:value={beneficiary} class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" /></div>
      <div><label class="mb-1 block text-sm font-semibold" for="d">الوصف</label><input id="d" bind:value={description} class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" /></div>
      <button type="submit" class="rounded-md bg-[var(--ec-teal)] px-4 py-2 text-sm font-semibold text-white">حفظ (draft)</button>
    </form>
  {/if}
  <p class="text-sm text-[var(--ec-text)]/60">الواجهة الخلفية جاهزة (create/approve). قائمة المصروفات قيد الإضافة.</p>
</section>
