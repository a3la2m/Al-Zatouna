<script lang="ts">
  import { authApi } from '$lib/api/auth';
  import { invoke } from '$lib/api';
  import { sessionToken, sessionUser, persistSession } from '$lib/stores/app';
  import { toast } from '$lib/toasts';
  import Button from '$lib/components/Button.svelte';
  import Field from '$lib/components/Field.svelte';
  import Card from '$lib/components/Card.svelte';
  import Alert from '$lib/components/Alert.svelte';

  let username = $state('');
  let password = $state('');
  let remember = $state(false);
  let busy = $state(false);
  let error = $state('');
  let info = $state('');

  // first-run admin creation
  let setupMode = $state(false);
  let newName = $state('');

  // recovery mode
  let recoverMode = $state(false);
  let recoveryCode = $state('');
  let newPassword = $state('');
  let newPassword2 = $state('');

  async function submit(e?: SubmitEvent) {
    e?.preventDefault();
    error = ''; info = ''; busy = true;
    try {
      if (setupMode) await authApi.createAdmin(username.trim(), newName.trim(), password);
      const res = await authApi.login(username.trim(), password, remember);
      sessionToken.set(res.token);
      sessionUser.set(res.user);
      if (remember) persistSession(res.token, res.user);
      if (res.must_change_password) {
        info = 'يجب تغيير كلمة المرور من صفحة الإعدادات ← حسابي.';
      }
    } catch (err) {
      error = (err as Error)?.message ?? 'خطأ غير متوقع';
      const msg = error;
      if (msg.includes('لم يُهيأ') || msg.includes('wizard')) setupMode = true;
    } finally {
      busy = false;
    }
  }

  async function recover(e: SubmitEvent) {
    e.preventDefault();
    error = ''; info = '';
    if (newPassword !== newPassword2) {
      error = 'تأكيد كلمة المرور غير مطابق';
      return;
    }
    busy = true;
    try {
      await invoke('auth_recover_password', {
        username: username.trim(), code: recoveryCode.trim(),
        new_password: newPassword
      });
      info = 'تمت إعادة تعيين كلمة المرور — سجّل الدخول بها الآن.';
      recoverMode = false;
      recoveryCode = ''; newPassword = ''; newPassword2 = '';
    } catch (err) {
      error = (err as Error)?.message ?? 'تعذّر الاسترداد';
    } finally { busy = false; }
  }
</script>

<section class="mx-auto flex min-h-[70vh] w-full max-w-md flex-col justify-center">
  <div class="mb-5 text-center">
    <h1 class="text-2xl font-bold" style="color:var(--ec-navy)">
      {setupMode ? 'إنشاء المدير الأول' : recoverMode ? 'استرداد كلمة المرور' : 'تسجيل الدخول'}
    </h1>
    <p class="mt-1 text-sm" style="color:var(--ec-text-3)">
      {setupMode ? 'الخطوة الأولى بعد تحديد مجلد البيانات' : recoverMode ? 'أدخل رمز الاسترداد وكلمة المرور الجديدة' : 'نظام إدارة المركز التعليمي'}
    </p>
  </div>

  <Card>
    {#if recoverMode}
      <form class="space-y-4" onsubmit={recover}>
        {#if error}<Alert kind="error">{error}</Alert>{/if}
        {#if info}<Alert kind="ok">{info}</Alert>{/if}
        <Field label="اسم المستخدم" required>
          <input class="ec-input" dir="ltr" bind:value={username} autocomplete="username" required />
        </Field>
        <Field label="رمز الاسترداد" hint="أحد الرموز التي أنشأها المدير ( ZYT-XXXX-XXXX-XXXX )." required>
          <input class="ec-input" dir="ltr" bind:value={recoveryCode} placeholder="ZYT-XXXX-XXXX-XXXX" required />
        </Field>
        <Field label="كلمة المرور الجديدة" required hint="8 أحرف على الأقل.">
          <input class="ec-input" type="password" dir="ltr" bind:value={newPassword} required minlength="8" autocomplete="new-password" />
        </Field>
        <Field label="تأكيد كلمة المرور" required>
          <input class="ec-input" type="password" dir="ltr" bind:value={newPassword2} required minlength="8" autocomplete="new-password" />
        </Field>
        <Button type="submit" variant="primary" block loading={busy}>استرداد وتعيين كلمة المرور</Button>
        <button type="button" class="ec-btn ec-btn-ghost ec-btn-sm w-full" onclick={() => { recoverMode = false; error = ''; info = ''; }}>
          العودة إلى الدخول
        </button>
      </form>
    {:else}
      <form class="space-y-4" onsubmit={submit}>
        {#if error}<Alert kind="error">{error}</Alert>{/if}
        {#if info}<Alert kind="ok">{info}</Alert>{/if}
        {#if setupMode}
          <Field label="الاسم الكامل" required>
            <input class="ec-input" bind:value={newName} required placeholder="مثال: مدير المركز" />
          </Field>
        {/if}
        <Field label="اسم المستخدم" required>
          <input class="ec-input" dir="ltr" bind:value={username} autocomplete="username" required />
        </Field>
        <Field label="كلمة المرور" required>
          <input class="ec-input" type="password" dir="ltr" bind:value={password} autocomplete="current-password" required />
        </Field>

        <div class="flex items-center justify-between gap-3">
          <label class="flex cursor-pointer items-center gap-2 text-[13px]" style="color:var(--ec-text-2)">
            <input type="checkbox" bind:checked={remember} />
            تذكّرني والبقاء مسجّلاً
          </label>
          <button
            type="button"
            class="text-[13px] font-semibold hover:underline"
            style="color:var(--ec-blue)"
            onclick={() => { recoverMode = true; error = ''; info = ''; }}
          >
            نسيت كلمة المرور؟
          </button>
        </div>

        <Button type="submit" variant="primary" block loading={busy}>
          {setupMode ? 'إنشاء المستخدم والدخول' : 'دخول'}
        </Button>
      </form>
    {/if}
  </Card>

  {#if !recoverMode}
    <p class="mt-4 text-center text-[12px]" style="color:var(--ec-text-3)">
      عند تفعيل «تذكّرني» تبقى الجلسة صالحة حتى بعد إغلاق التطبيق (30 يوماً).
    </p>
  {/if}
</section>
