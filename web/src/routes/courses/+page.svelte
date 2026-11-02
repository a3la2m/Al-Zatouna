<script lang="ts">
  import { onMount } from 'svelte';
  import { sessionToken } from '$lib/stores/app';
  import { peopleApi, type Subject, type Teacher, type Room } from '$lib/api/people';
  import { invoke } from '$lib/api';
  import { get } from 'svelte/store';

  type Course = { id: number; course_no: number; name: string; status: string; fee_minor: number };
  let courses = $state<Course[]>([]);
  let subjects = $state<Subject[]>([]);
  let teachers = $state<Teacher[]>([]);
  let rooms = $state<Room[]>([]);
  let loading = $state(false);
  let error = $state('');
  let showForm = $state(false);
  let name = $state('');
  let subjectId = $state(0);
  let teacherId = $state(0);
  let roomId = $state(0);
  let fee = $state(0);

  async function load() {
    const token = get(sessionToken);
    if (!token) return;
    loading = true; error = '';
    try {
      courses = await invoke<Course[]>('courses_list', { token, branch_id: 1, status: null });
      subjects = await peopleApi.subjects.list(token, false);
      teachers = await peopleApi.teachers.list(token, 1);
      rooms = await peopleApi.rooms.list(token, 1);
      if (!subjectId && subjects.length) subjectId = subjects[0].id;
      if (!teacherId && teachers.length) teacherId = teachers[0].id;
      if (!roomId && rooms.length) roomId = rooms[0].id;
    } catch (e) { console.error(e); error = (e as any)?.message || 'خطأ'; }
    finally { loading = false; }
  }

  async function create(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    error = '';
    try {
      await invoke('courses_create', {
        token, branch_id: 1, name, subject_id: subjectId, teacher_id: teacherId, room_id: roomId, fee_minor: fee, is_private: false
      });
      name = ''; showForm = false; await load();
    } catch (e) { console.error(e); error = (e as any)?.message || 'خطأ'; }
  }

  onMount(load);
</script>

<section>
  <div class="mb-4 flex items-center justify-between">
    <h1 class="text-2xl font-bold text-[var(--ec-navy)]">الدورات</h1>
    <button class="rounded-md bg-[var(--ec-blue)] px-4 py-2 text-sm font-semibold text-white hover:opacity-90" onclick={() => (showForm = !showForm)}>
      {showForm ? 'إلغاء' : 'إضافة دورة'}
    </button>
  </div>
  {#if error}<p class="mb-3 rounded bg-red-50 px-3 py-2 text-sm text-red-700">{error}</p>{/if}
  {#if showForm}
    <form class="mb-6 space-y-3 rounded-xl bg-[var(--ec-surface)] p-4 shadow-sm" onsubmit={create}>
      <div><label class="mb-1 block text-sm font-semibold" for="n">الاسم</label><input id="n" bind:value={name} required class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" /></div>
      <div><label class="mb-1 block text-sm font-semibold" for="s">المادة</label>
        <select id="s" bind:value={subjectId} class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2">
          {#each subjects as s}<option value={s.id}>{s.name}</option>{/each}
        </select>
      </div>
      <div><label class="mb-1 block text-sm font-semibold" for="t">المدرس</label>
        <select id="t" bind:value={teacherId} class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2">
          {#each teachers as t}<option value={t.id}>{t.full_name}</option>{/each}
        </select>
      </div>
      <div><label class="mb-1 block text-sm font-semibold" for="r">القاعة</label>
        <select id="r" bind:value={roomId} class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2">
          {#each rooms as r}<option value={r.id}>{r.name}</option>{/each}
        </select>
      </div>
      <div><label class="mb-1 block text-sm font-semibold" for="f">الرسوم (minor)</label><input id="f" type="number" bind:value={fee} class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" dir="ltr" /></div>
      <button type="submit" class="rounded-md bg-[var(--ec-teal)] px-4 py-2 text-sm font-semibold text-white">حفظ</button>
    </form>
  {/if}
  {#if loading}<p class="text-sm text-[var(--ec-text)]/60">جارٍ التحميل…</p>
  {:else if courses.length === 0}<p class="text-sm text-[var(--ec-text)]/60">لا توجد دورات بعد.</p>
  {:else}
    <div class="overflow-x-auto rounded-xl bg-[var(--ec-surface)] shadow-sm">
      <table class="w-full text-sm">
        <thead class="bg-[var(--ec-navy)] text-white"><tr><th class="px-3 py-2 text-right">#</th><th class="px-3 py-2 text-right">الاسم</th><th class="px-3 py-2 text-right">الحالة</th><th class="px-3 py-2 text-right">الرسوم</th></tr></thead>
        <tbody>{#each courses as c}<tr class="border-t border-[var(--ec-border)]"><td class="px-3 py-2">{c.course_no}</td><td class="px-3 py-2">{c.name}</td><td class="px-3 py-2">{c.status}</td><td class="px-3 py-2">{c.fee_minor}</td></tr>{/each}</tbody>
      </table>
    </div>
  {/if}
</section>
