<script lang="ts">
  import { onMount } from 'svelte';
  import { sessionToken } from '$lib/stores/app';
  import { invoke } from '$lib/api';
  import { get } from 'svelte/store';

  type Report = { id: number; name: string; definition_json: string };
  let reports = $state<Report[]>([]);
  let loading = $state(false);
  let error = $state('');
  let name = $state('');

  async function load() {
    const token = get(sessionToken);
    if (!token) return;
    loading = true; error = '';
    try { reports = await invoke<Report[]>('reports_list', { token }); }
    catch (e) { console.error(e); error = (e as any)?.message || 'خطأ'; }
    finally { loading = false; }
  }

  async function save() {
    const token = get(sessionToken);
    if (!token || !name) return;
    error = '';
    try {
      await invoke('reports_save', { token, name, definition_json: '{"columns":["full_name"]}' });
      name = ''; await load();
    } catch (e) { console.error(e); error = (e as any)?.message || 'خطأ'; }
  }

  onMount(load);
</script>

<section>
  <h1 class="mb-4 text-2xl font-bold text-[var(--ec-navy)]">التقارير</h1>
  {#if error}<p class="mb-3 rounded bg-red-50 px-3 py-2 text-sm text-red-700">{error}</p>{/if}
  <div class="mb-4 flex gap-2">
    <input bind:value={name} placeholder="اسم التقرير" class="flex-1 rounded-md border border-[var(--ec-border)] px-3 py-2" />
    <button class="rounded-md bg-[var(--ec-blue)] px-4 py-2 text-sm font-semibold text-white" onclick={save}>حفظ</button>
  </div>
  {#if loading}<p class="text-sm text-[var(--ec-text)]/60">جارٍ التحميل…</p>
  {:else if reports.length === 0}<p class="text-sm text-[var(--ec-text)]/60">لا توجد تقارير محفوظة.</p>
  {:else}
    <ul class="space-y-2">
      {#each reports as r}<li class="rounded-xl bg-[var(--ec-surface)] p-3 shadow-sm">{r.name}</li>{/each}
    </ul>
  {/if}
</section>
