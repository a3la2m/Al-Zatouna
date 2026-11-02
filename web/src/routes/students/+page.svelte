<script lang="ts">
  import { onMount } from 'svelte';
  import { sessionToken } from '$lib/stores/app';
  import { peopleApi, type Student } from '$lib/api/people';
  import { get } from 'svelte/store';

  let students = $state<Student[]>([]);
  let loading = $state(false);
  let error = $state('');
  let showForm = $state(false);

  // form fields
  let fullName = $state('');
  let nationalId = $state('');
  let phone = $state('');

  async function load() {
    const token = get(sessionToken);
    if (!token) return;
    loading = true;
    error = '';
    try {
      students = await peopleApi.students.list(token, 1);
    } catch (e) {
      console.error('students load error', e);
      error = (e as any)?.message || (e as any)?.toString?.() || 'خطأ غير معروف';
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
      await peopleApi.students.create(token, {
        branch_id: 1,
        full_name: fullName,
        national_id: nationalId,
        birth_date: '',
        gender: null,
        phone,
        email: '',
        address: '',
        notes: ''
      });
      fullName = '';
      nationalId = '';
      phone = '';
      showForm = false;
      await load();
    } catch (e) {
      console.error('students load error', e);
      error = (e as any)?.message || (e as any)?.toString?.() || 'خطأ غير معروف';
    }
  }

  async function archive(s: Student) {
    const token = get(sessionToken);
    if (!token) return;
    const reason = prompt('سبب الأرشفة؟');
    if (!reason) return;
    try {
      await peopleApi.students.setStatus(token, s.id, 'archived', reason);
      await load();
    } catch (e) {
      console.error('students load error', e);
      error = (e as any)?.message || (e as any)?.toString?.() || 'خطأ غير معروف';
    }
  }

  onMount(load);
</script>

<section>
  <div class="mb-4 flex items-center justify-between">
    <h1 class="text-2xl font-bold text-[var(--ec-navy)]">الطلاب</h1>
    <button
      class="rounded-md bg-[var(--ec-blue)] px-4 py-2 text-sm font-semibold text-white hover:opacity-90"
      onclick={() => (showForm = !showForm)}
    >
      {showForm ? 'إلغاء' : 'إضافة طالب'}
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
        <label class="mb-1 block text-sm font-semibold" for="nid">رقم الهوية</label>
        <input id="nid" bind:value={nationalId} class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" dir="ltr" />
      </div>
      <div>
        <label class="mb-1 block text-sm font-semibold" for="ph">الهاتف</label>
        <input id="ph" bind:value={phone} class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" dir="ltr" />
      </div>
      <button type="submit" class="rounded-md bg-[var(--ec-teal)] px-4 py-2 text-sm font-semibold text-white">
        حفظ
      </button>
    </form>
  {/if}

  {#if loading}
    <p class="text-sm text-[var(--ec-text)]/60">جارٍ التحميل…</p>
  {:else if students.length === 0}
    <p class="text-sm text-[var(--ec-text)]/60">لا يوجد طلاب بعد.</p>
  {:else}
    <div class="overflow-x-auto rounded-xl bg-[var(--ec-surface)] shadow-sm">
      <table class="w-full text-sm">
        <thead class="bg-[var(--ec-navy)] text-white">
          <tr>
            <th class="px-3 py-2 text-right">#</th>
            <th class="px-3 py-2 text-right">الاسم</th>
            <th class="px-3 py-2 text-right">الهوية</th>
            <th class="px-3 py-2 text-right">الهاتف</th>
            <th class="px-3 py-2 text-right">الحالة</th>
            <th class="px-3 py-2 text-right">إجراءات</th>
          </tr>
        </thead>
        <tbody>
          {#each students as s}
            <tr class="border-t border-[var(--ec-border)]">
              <td class="px-3 py-2">{s.student_no}</td>
              <td class="px-3 py-2">{s.full_name}</td>
              <td class="px-3 py-2" dir="ltr">{s.national_id}</td>
              <td class="px-3 py-2" dir="ltr">{s.phone}</td>
              <td class="px-3 py-2">{s.status}</td>
              <td class="px-3 py-2">
                {#if s.status !== 'archived'}
                  <button class="text-xs text-red-600 hover:underline" onclick={() => archive(s)}>أرشفة</button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>
