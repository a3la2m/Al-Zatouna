<script lang="ts">
  import { font } from '$lib/stores/app';
  import ChevronIcon from '$lib/components/ChevronIcon.svelte';

  interface NavItem {
    code: string;
    label: string;
    icon: string;
    href: string;
  }

  const sections: { title: string; items: NavItem[] }[] = [
    {
      title: 'الرئيسية',
      items: [
        { code: 'dashboard', label: 'لوحة التحكم', icon: '◧', href: '/' }
      ]
    },
    {
      title: 'بيانات بشرية',
      items: [
        { code: 'students', label: 'الطلاب', icon: '👤', href: '/students' },
        { code: 'teachers', label: 'المدرسون', icon: '👔', href: '/teachers' }
      ]
    },
    {
      title: 'التعليم',
      items: [
        { code: 'subjects', label: 'المواد', icon: '✔', href: '/subjects' },
        { code: 'courses', label: 'الدورات', icon: '📚', href: '/courses' },
        { code: 'rooms', label: 'القاعات', icon: '▦', href: '/rooms' },
        { code: 'schedule', label: 'الجدول', icon: '🗓', href: '/schedule' },
        { code: 'attendance', label: 'الحضور', icon: '✓', href: '/attendance' }
      ]
    },
    {
      title: 'المال',
      items: [
        { code: 'finance', label: 'المحرك المالي', icon: '⚖', href: '/finance' },
        { code: 'vouchers', label: 'السندات', icon: '🧾', href: '/vouchers' },
        { code: 'cash', label: 'الصندوق', icon: '💰', href: '/cash' },
        { code: 'expenses', label: 'المصروفات', icon: '💸', href: '/expenses' }
      ]
    },
    {
      title: 'النظام',
      items: [
        { code: 'reports', label: 'التقارير', icon: '📊', href: '/reports' },
        { code: 'settings', label: 'الإعدادات', icon: '⚙', href: '/settings' }
      ]
    }
  ];

  let collapsed = $state(false);
  const active = $derived(collapsed); // placeholder-driven reactivity example
</script>

<aside
  class="right-0 h-full border-l border-[var(--ec-border)] bg-[var(--ec-navy)] text-white transition-all duration-200"
  class:w-[260px]={!collapsed}
  class:w-[64px]={collapsed}
>
  <div class="flex items-center justify-between px-3 py-4">
    {#if !collapsed}
      <div class="text-lg font-bold tracking-tight">المركز التعليمي</div>
    {/if}
    <button
      class="rounded p-1 hover:bg-white/10"
      title={collapsed ? 'توسيع القائمة' : 'طي القائمة'}
      onclick={() => (collapsed = !collapsed)}
    >
      <ChevronIcon flipped={!collapsed} />
    </button>
  </div>

  <nav class="px-2">
    {#each sections as section}
      <div class="mt-3">
        {#if !collapsed}
          <div class="px-3 pb-1 text-xs font-semibold text-white/50">
            {section.title}
          </div>
        {/if}
        {#each section.items as item}
          <a
            href={item.href}
            class="flex items-center gap-3 rounded-md px-3 py-2 text-sm hover:bg-white/10"
            class:justify-center={collapsed}
            title={collapsed ? item.label : undefined}
          >
            <span aria-hidden="true">{item.icon}</span>
            {#if !collapsed}
              <span>{item.label}</span>
            {/if}
          </a>
        {/each}
      </div>
    {/each}
  </nav>

  {#if !collapsed}
    <div class="absolute bottom-3 px-3 text-[10px] text-white/40">
      {font} • إصدار 0.1.0
    </div>
  {/if}
</aside>

<style>
  aside { position: fixed; inset-block: 0; }
</style>
