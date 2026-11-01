<script lang="ts">
  import LoginForm from '$lib/components/LoginForm.svelte';
  import { authApi } from '$lib/api/auth';
  import { sessionToken, sessionUser, font } from '$lib/stores/app';
  import { isTauri } from '$lib/env';
  import { get } from 'svelte/store';
</script>

<section class="mx-auto max-w-3xl">
  {#if $sessionUser}
    <h1 class="text-2xl font-bold text-[var(--ec-navy)]">لوحة التحكم</h1>
    <p class="mt-2 text-sm text-[var(--ec-text)]/70">
      مرحباً {$sessionUser.full_name} — الصلاحيات: {$sessionUser.permissions.length}
    </p>
    <button
      class="mt-6 rounded-md bg-[var(--ec-navy)] px-4 py-2 text-sm text-white"
      onclick={async () => {
        const t = get(sessionToken);
        if (t) await authApi.logout(t);
        sessionToken.set(null);
        sessionUser.set(null);
      }}
    >
      تسجيل الخروج
    </button>
  {:else}
    {#if !isTauri}
      <div class="mb-4 rounded bg-amber-100 px-3 py-2 text-sm text-amber-800">
        وضع المعاينة في المتصفح — الدخول الفعلي يعمل داخل التطبيق فقط
      </div>
      <p class="text-sm text-[var(--ec-text)]/50">{font}</p>
    {/if}
    <LoginForm />
  {/if}
</section>
