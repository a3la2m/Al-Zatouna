<script lang="ts">
  import LoginForm from '$lib/components/LoginForm.svelte';
  import { sessionUser } from '$lib/stores/app';
  import { isTauri } from '$lib/env';
</script>

{#if $sessionUser}
  <section class="mx-auto max-w-3xl">
    <h1 class="text-2xl font-bold text-[var(--ec-navy)]">لوحة التحكم</h1>
    <p class="mt-2 text-sm text-[var(--ec-text)]/70">
      مرحباً {$sessionUser.full_name} — الصلاحيات: {$sessionUser.permissions.length}
    </p>
  </section>
{:else}
  <div class="flex min-h-[80vh] items-center justify-center">
    <div class="w-full max-w-md">
      {#if !isTauri}
        <div class="mb-4 rounded bg-amber-100 px-3 py-2 text-sm text-amber-800">
          وضع المعاينة في المتصفح — الدخول الفعلي يعمل داخل التطبيق فقط
        </div>
      {/if}
      <LoginForm />
    </div>
  </div>
{/if}
