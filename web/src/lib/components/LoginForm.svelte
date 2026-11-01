<script lang="ts">
  import { authApi, type SessionUserDto } from '$lib/api/auth';
  import { sessionToken, sessionUser } from '$lib/stores/app';

  let username = $state('');
  let password = $state('');
  let busy = $state(false);
  let error = $state('');

  // first-run admin creation fields (shown when no admin exists)
  let setupMode = $state(false);
  let newName = $state('');

  async function submit(e?: SubmitEvent) {
    e?.preventDefault();
    error = '';
    busy = true;
    try {
      if (setupMode) {
        await authApi.createAdmin(username.trim(), newName.trim(), password);
      }
      const res = await authApi.login(username.trim(), password);
      sessionToken.set(res.token);
      sessionUser.set(res.user);
      // TODO Phase 3.1: if res.must_change_password → route to change-password view
    } catch (err) {
      error = (err as Error)?.message ?? 'خطأ غير متوقع';
      const msg = error;
      // a data dir without users ⇒ offer the first-run admin setup
      if (msg.includes('لم يُهيأ') || msg.includes('wizard')) {
        setupMode = true;
      }
    } finally {
      busy = false;
    }
  }
</script>

<section class="mx-auto flex min-h-[70vh] max-w-md flex-col justify-center">
  <h1 class="mb-1 text-2xl font-bold text-[var(--ec-navy)]">
    {setupMode ? 'إنشاء المدير الأول' : 'تسجيل الدخول'}
  </h1>
  <p class="mb-6 text-sm text-[var(--ec-text)]/60">
    {setupMode
      ? 'الخطوة الأولى بعد تحديد مجلد البيانات'
      : 'نظام إدارة المركز التعليمي'}
  </p>

  <form class="card space-y-4 rounded-xl bg-[var(--ec-surface)] p-6 shadow-sm" onsubmit={submit}>
    {#if setupMode}
      <div>
        <label class="mb-1 block text-sm font-semibold" for="fn">الاسم الكامل</label>
        <input
          id="fn" bind:value={newName} required
          class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2"
          placeholder="مثال: مدير المركز"
        />
      </div>
    {/if}
    <div>
      <label class="mb-1 block text-sm font-semibold" for="un">اسم المستخدم</label>
      <input
        id="un" bind:value={username} required autocomplete="username"
        class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2 ltr:text-left"
        dir="ltr"
      />
    </div>
    <div>
      <label class="mb-1 block text-sm font-semibold" for="pw">كلمة المرور</label>
      <input
        id="pw" bind:value={password} type="password" required
        autocomplete="current-password"
        class="w-full rounded-md border border-[var(--ec-border)] px-3 py-2 ltr:text-left"
        dir="ltr"
      />
    </div>

    {#if error}
      <p class="rounded bg-red-50 px-3 py-2 text-sm text-red-700" role="alert">{error}</p>
    {/if}

    <button
      type="submit"
      disabled={busy}
      class="w-full rounded-md bg-[var(--ec-blue)] py-2 font-semibold text-white hover:opacity-90 disabled:opacity-50"
    >
      {busy ? 'جارٍ…' : setupMode ? 'إنشاء المستخدم والدخول' : 'دخول'}
    </button>
  </form>
</section>

<style>
  .card { border: 1px solid var(--ec-border); }
</style>
