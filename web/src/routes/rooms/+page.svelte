<script lang="ts">
  import { onMount } from 'svelte';
  import { sessionToken } from '$lib/stores/app';
  import { peopleApi, type Room } from '$lib/api/people';
  import { invoke } from '$lib/api';
  import { get } from 'svelte/store';

  let items = $state<Room[]>([]);
  let loading = $state(false);
  let error = $state('');
  let showForm = $state(false);
  let code = $state('');
  let name = $state('');
  let capacity = $state(30);

  async function load() {
    const token = get(sessionToken);
    if (!token) return;
    loading = true; error = '';
    try { items = await peopleApi.rooms.list(token, 1); }
    catch (e) { console.error(e); error = (e as any)?.message || 'خطأ'; }
    finally { loading = false; }
  }

  async function create(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    error = '';
    try {
      await invoke('rooms_create', { token, branch_id: 1, code, name, capacity, description: '' });
      code = ''; name = ''; capacity = 30; showForm = false; await load();
    } catch (e) { console.error(e); error = (e as any)?.message || 'خطأ'; }
  }

  onMount(load);
</script>

<section>
  <div class="mb-4 flex items-center justify-between">
    <h1 class="text-2xl font-bold text-[var(--ec-navy)]">القاعات</h1>
    <button class="rounded-md bg-[var(--ec-blue)] px-4 py-2 text-sm font-semibold text-white hover:opacity-90" onclick={() => (showForm = !showForm)}>
      {showForm ? 'إلغاء' : 'إضافة قاعة'}
    </button>
  </div>
  {#if error}<p class="mb-3 rounded bg-red-50 px-3 py-2 text-sm text-red-700">{error}</p>{/if}
  {#if showForm}
    <form class="mb-6 space-y-3 rounded-xl bg-[var(--ec-surface)] p-4 shadow-sm" onsubmit={create}>
      <div><label class="mb-1 block text-sm font-semibold" for="c">الكود</label><input id="c" bind:value={code} required class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" dir="ltr" /></div>
      <div><label class="mb-1 block text-sm font-semibold" for="n">الاسم</label><input id="n" bind:value={name} required class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" /></div>
      <div><label class="mb-1 block text-sm font-semibold" for="cap">السعة</label><input id="cap" type="number" bind:value={capacity} class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" dir="ltr" /></div>
      <button type="submit" class="rounded-md bg-[var(--ec-teal)] px-4 py-2 text-sm font-semibold text-white">حفظ</button>
    </form>
  {/if}
  {#if loading}<p class="text-sm text-[var(--ec-text)]/60">جارٍ التحميل…</p>
  {:else if items.length === 0}<p class="text-sm text-[var(--ec-text)]/60">لا توجد قاعات بعد.</p>
  {:else}
    <div class="overflow-x-auto rounded-xl bg-[var(--ec-surface)] shadow-sm">
      <table class="w-full text-sm">
        <thead class="bg-[var(--ec-navy)] text-white"><tr><th class="px-3 py-2 text-right">الكود</th><th class="px-3 py-2 text-right">الاسم</th><th class="px-3 py-2 text-right">السعة</th><th class="px-3 py-2 text-right">الحالة</th></tr></thead>
        <tbody>{#each items as r}<tr class="border-t border-[var(--ec-border)]"><td class="px-3 py-2" dir="ltr">{r.code}</td><td class="px-3 py-2">{r.name}</td><td class="px-3 py-2">{r.capacity}</td><td class="px-3 py-2">{r.status}</td></tr>{/each}</tbody>
      </table>
    </div>
  {/if}
</section>
