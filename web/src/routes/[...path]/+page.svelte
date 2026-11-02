<script lang="ts">
  import { page } from '$app/stores';
</script>

<section class="mx-auto max-w-3xl">
  <h1 class="text-2xl font-bold text-[var(--ec-navy)]">قيد التطوير</h1>
  <p class="mt-2 text-sm text-[var(--ec-text)]/60">
    الصفحة <code class="rounded bg-[var(--ec-border)] px-1">{$page.url.pathname}</code>
    قيد التطوير. الواجهة الخلفية (Rust) جاهزة ومختبرة؛ واجهة المستخدم ستُضاف لاحقاً.
  </p>
  <a href="/" class="mt-4 inline-block text-sm text-[var(--ec-blue)] hover:underline">← العودة للرئيسية</a>
</section>
