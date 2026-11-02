<script lang="ts">
  import { onMount } from 'svelte';
  import { sessionToken } from '$lib/stores/app';
  import { peopleApi, type Teacher } from '$lib/api/people';
  import { get } from 'svelte/store';

  let teachers = $state<Teacher[]>([]);
  let loading = $state(false);
  let error = $state('');
  let showForm = $state(false);
  let fullName = $state('');
  let specialty = $state('');

  async function load() {
    const token = get(sessionToken);
    if (!token) return;
    loading = true;
    error = '';
    try {
      teachers = await peopleApi.teachers.list(token, 1);
    } catch (e) {
      console.error('teachers error', e);
      error = (e as any)?.message || 'خطأ غير معروف';
    } finally {
      loading = false;
    }
  }

  async function create(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    error = '';
    try {
      await peopleApi.teachers.create
        ? null
        : null;
      // peopleApi has no teachers.create wrapper; call invoke directly
      const { invoke } = await import('$lib/api');
      await invoke('teachers_create', {
        token,
        branch_id: 1,
        full_name: fullName,
        phone: '',
        email: '',
        specialty,
        qualifications: '',
        address: '',
        hire_date: '',
        notes: ''
      });
      fullName = '';
      specialty = '';
      showForm = false;
      await load();
    } catch (e) {
      console.error('teachers error', e);
      error = (e as any)?.message || 'خطأ غير معروف';
    }
  }

  onMount(load);
</script>

<section>
  <div class="mb-4 flex items-center justify-between">
    <h1 class="text-2xl font-bold text-[var(--ec-navy)]">المدرسون</h1>
    <button
      class="rounded-md bg-[var(--ec-blue)] px-4 py-2 text-sm font-semibold text-white hover:opacity-90"
      onclick={() => (showForm = !showForm)}
    >
      {showForm ? 'إلغاء' : 'إضافة مدرس'}
    </button>
  </div>

  {#if error}
    <p class="mb-3 rounded bg-red-50 px-3 py-2 text-sm text-red-700">{error}</p>
  {/if}

  {#if showForm}
    <form class="mb-6 space-y-3 rounded-xl bg-[var(--ec-surface)] p-4 shadow-sm" onsubmit={create}>
      <div>
        <label class="mb-1 block text-sm font-semibold" for="fn">الاسم الكامل</label>
        <input id="fn" bind:value={fullName} required class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" />
      </div>
      <div>
        <label class="mb-1 block text-sm font-semibold" for="sp">التخصص</label>
        <input id="sp" bind:value={specialty} class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" />
      </div>
      <button type="submit" class="rounded-md bg-[var(--ec-teal)] px-4 py-2 text-sm font-semibold text-white">حفظ</button>
    </form>
  {/if}

  {#if loading}
    <p class="text-sm text-[var(--ec-text)]/60">جارٍ التحميل…</p>
  {:else if teachers.length === 0}
    <p class="text-sm text-[var(--ec-text)]/60">لا يوجد مدرسون بعد.</p>
  {:else}
    <div class="overflow-x-auto rounded-xl bg-[var(--ec-surface)] shadow-sm">
      <table class="w-full text-sm">
        <thead class="bg-[var(--ec-navy)] text-white">
          <tr>
            <th class="px-3 py-2 text-right">#</th>
            <th class="px-3 py-2 text-right">الاسم</th>
            <th class="px-3 py-2 text-right">التخصص</th>
            <th class="px-3 py-2 text-right">الهاتف</th>
            <th class="px-3 py-2 text-right">الحالة</th>
          </tr>
        </thead>
        <tbody>
          {#each teachers as t}
            <tr class="border-t border-[var(--ec-border)]">
              <td class="px-3 py-2">{t.teacher_no}</td>
              <td class="px-3 py-2">{t.full_name}</td>
              <td class="px-3 py-2">{t.specialty}</td>
              <td class="px-3 py-2" dir="ltr">{t.phone}</td>
              <td class="px-3 py-2">{t.status}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>
