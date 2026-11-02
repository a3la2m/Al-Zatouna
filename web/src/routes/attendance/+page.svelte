<script lang="ts">
  import { sessionToken } from '$lib/stores/app';
  import { invoke } from '$lib/api';
  import { get } from 'svelte/store';

  let sessionId = $state(0);
  let studentId = $state(0);
  let status = $state('present');
  let lateMinutes = $state(0);
  let error = $state('');
  let result = $state('');

  async function record() {
    const token = get(sessionToken);
    if (!token) return;
    error = ''; result = '';
    try {
      const r = await invoke<any>('attendance_record_student', {
        token, session_id: sessionId, student_id: studentId, status, late_minutes: lateMinutes
      });
      result = `تم تسجيل حضور الطالب ${r.student_id}: ${r.status}`;
    } catch (e) { console.error(e); error = (e as any)?.message || 'خطأ'; }
  }
</script>

<section>
  <h1 class="mb-4 text-2xl font-bold text-[var(--ec-navy)]">الحضور</h1>
  {#if error}<p class="mb-3 rounded bg-red-50 px-3 py-2 text-sm text-red-700">{error}</p>{/if}
  {#if result}<p class="mb-3 rounded bg-green-50 px-3 py-2 text-sm text-green-700">{result}</p>{/if}
  <form class="space-y-3 rounded-xl bg-[var(--ec-surface)] p-4 shadow-sm" onsubmit={(e) => { e.preventDefault(); record(); }}>
    <div><label class="mb-1 block text-sm font-semibold" for="sid">رقم الحصة (session_id)</label><input id="sid" type="number" bind:value={sessionId} class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" dir="ltr" /></div>
    <div><label class="mb-1 block text-sm font-semibold" for="stid">رقم الطالب (student_id)</label><input id="stid" type="number" bind:value={studentId} class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" dir="ltr" /></div>
    <div><label class="mb-1 block text-sm font-semibold" for="st">الحالة</label>
      <select id="st" bind:value={status} class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2">
        <option value="present">حاضر</option><option value="absent">غائب</option><option value="late">متأخر</option><option value="excused">معذور</option>
      </select>
    </div>
    <div><label class="mb-1 block text-sm font-semibold" for="lm">دقائق التأخير</label><input id="lm" type="number" bind:value={lateMinutes} class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2" dir="ltr" /></div>
    <button type="submit" class="rounded-md bg-[var(--ec-teal)] px-4 py-2 text-sm font-semibold text-white">تسجيل</button>
  </form>
</section>
