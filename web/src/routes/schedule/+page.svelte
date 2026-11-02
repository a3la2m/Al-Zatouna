<script lang="ts">
  import { onMount } from 'svelte';
  import { sessionToken } from '$lib/stores/app';
  import { invoke } from '$lib/api';
  import { get } from 'svelte/store';

  type Session = { id: number; course_id: number; session_date: string; start_time: string; end_time: string; status: string; seq_in_course: number };
  let sessions = $state<Session[]>([]);
  let loading = $state(false);
  let error = $state('');

  async function load() {
    const token = get(sessionToken);
    if (!token) return;
    loading = true; error = '';
    try { sessions = await invoke<Session[]>('sessions_list', { token, course_id: null }); }
    catch (e) { console.error(e); error = (e as any)?.message || 'خطأ'; }
    finally { loading = false; }
  }

  async function cancel(s: Session) {
    const token = get(sessionToken);
    if (!token) return;
    const reason = prompt('سبب الإلغاء؟');
    if (!reason) return;
    try { await invoke('sessions_set_status', { token, id: s.id, status: 'cancelled', reason }); await load(); }
    catch (e) { console.error(e); error = (e as any)?.message || 'خطأ'; }
  }

  onMount(load);
</script>

<section>
  <h1 class="mb-4 text-2xl font-bold text-[var(--ec-navy)]">الجدول / الحصص</h1>
  {#if error}<p class="mb-3 rounded bg-red-50 px-3 py-2 text-sm text-red-700">{error}</p>{/if}
  {#if loading}<p class="text-sm text-[var(--ec-text)]/60">جارٍ التحميل…</p>
  {:else if sessions.length === 0}<p class="text-sm text-[var(--ec-text)]/60">لا توجد حصص بعد. أنشئ دورة ثم أضف حصصاً.</p>
  {:else}
    <div class="overflow-x-auto rounded-xl bg-[var(--ec-surface)] shadow-sm">
      <table class="w-full text-sm">
        <thead class="bg-[var(--ec-navy)] text-white"><tr><th class="px-3 py-2 text-right">#</th><th class="px-3 py-2 text-right">الدورة</th><th class="px-3 py-2 text-right">التاريخ</th><th class="px-3 py-2 text-right">الوقت</th><th class="px-3 py-2 text-right">الحالة</th><th class="px-3 py-2 text-right">إجراءات</th></tr></thead>
        <tbody>{#each sessions as s}<tr class="border-t border-[var(--ec-border)]"><td class="px-3 py-2">{s.seq_in_course}</td><td class="px-3 py-2">{s.course_id}</td><td class="px-3 py-2">{s.session_date}</td><td class="px-3 py-2">{s.start_time}–{s.end_time}</td><td class="px-3 py-2">{s.status}</td><td class="px-3 py-2">{#if s.status === 'scheduled'}<button class="text-xs text-red-600 hover:underline" onclick={() => cancel(s)}>إلغاء</button>{/if}</td></tr>{/each}</tbody>
      </table>
    </div>
  {/if}
</section>
