<script lang="ts">
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { sessionToken, sessionUser, sidebarCollapsed, clearPersistedSession } from '$lib/stores/app';
  import { authApi } from '$lib/api/auth';
  import { toast } from '$lib/toasts';
  import ChevronIcon from '$lib/components/ChevronIcon.svelte';
  import Logo from '$lib/components/Logo.svelte';
  import NotificationBell from '$lib/components/NotificationBell.svelte';

  interface NavItem {
    code: string;
    label: string;
    icon: string;
    href: string;
  }

  const sections: { title: string; items: NavItem[] }[] = [
    {
      title: 'الرئيسية',
      items: [{ code: 'dashboard', label: 'لوحة التحكم', icon: '◧', href: '/' }]
    },
    {
      title: 'بيانات بشرية',
      items: [
        { code: 'students', label: 'الطلاب', icon: '👤', href: '/students' },
        { code: 'teachers', label: 'المدرسون', icon: '👔', href: '/teachers' },
        { code: 'enrollments', label: 'التسجيلات', icon: '📝', href: '/enrollments' }
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
      title: 'المالية',
      items: [
        { code: 'finance', label: 'المحرك المالي', icon: '⚖', href: '/finance' },
        { code: 'cash', label: 'الصندوق', icon: '💰', href: '/cash' },
        { code: 'vouchers', label: 'السندات', icon: '🧾', href: '/vouchers' },
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

  let collapsed = $derived($sidebarCollapsed);
  const toggle = () => sidebarCollapsed.update((v) => !v);

  const path = $derived(page.url.pathname);
  const isActive = (href: string) => (href === '/' ? path === '/' : path.startsWith(href));

  async function logout() {
    const token = $sessionToken;
    try {
      if (token) await authApi.logout(token);
    } catch {
      /* session may already be gone */
    }
    sessionToken.set(null);
    sessionUser.set(null);
    clearPersistedSession();
    toast.ok('تم تسجيل الخروج');
    // leave the protected page — otherwise the layout renders the current
    // route (e.g. /teachers) inside the centered login container
    await goto('/');
  }
</script>

<aside
  class="fixed inset-y-0 right-0 z-30 flex flex-col text-white transition-[width] duration-200"
  style="width:{collapsed ? 68 : 264}px; background: linear-gradient(180deg, #24370f 0%, #1a2809 100%); border-inline-start: 1px solid rgba(255,255,255,.07);"
>
  <!-- brand / notifications trigger (logo) -->
  <div class="flex items-center gap-2.5 px-4 py-4" style="border-bottom:1px solid rgba(255,255,255,.08)">
    {#if $sessionUser}
      <NotificationBell variant="logo" />
    {:else}
      <div class="flex-none rounded-xl p-0.5" style="background:rgba(255,255,255,.08)">
        <Logo size={30} variant="icon" light />
      </div>
    {/if}
    {#if !collapsed}
      <div class="min-w-0 flex-1">
        <div class="truncate text-[16px] font-extrabold leading-tight" style="color:#eaf2d8">الزيتونة</div>
        <div class="truncate text-[11px] text-white/55">
          {$sessionUser ? 'اضغط الشعار لعرض الإشعارات' : 'نظام إدارة المركز التعليمي'}
        </div>
      </div>
    {/if}
    <button
      class="flex h-7 w-7 flex-none items-center justify-center rounded-md text-white/70 hover:bg-white/10 hover:text-white"
      title={collapsed ? 'توسيع القائمة' : 'طي القائمة'}
      aria-label={collapsed ? 'توسيع القائمة' : 'طي القائمة'}
      onclick={toggle}
    >
      <ChevronIcon flipped={!collapsed} />
    </button>
  </div>

  <!-- navigation -->
  <nav class="flex-1 overflow-y-auto overflow-x-hidden px-2 py-2">
    {#each sections as section (section.title)}
      <div class="mt-3">
        {#if !collapsed}
          <div class="px-3 pb-1.5 text-[11px] font-bold tracking-wide text-white/40">
            {section.title}
          </div>
        {/if}
        <ul class="space-y-0.5">
          {#each section.items as item (item.code)}
            <li>
              <a
                href={item.href}
                class="group relative flex items-center gap-3 rounded-lg px-3 py-2 text-[13.5px] transition-colors"
                class:justify-center={collapsed}
                class:font-semibold={isActive(item.href)}
                style={isActive(item.href)
                  ? 'background: rgba(255,255,255,.13); color:#fff;'
                  : 'color: rgba(255,255,255,.72);'}
                title={collapsed ? item.label : undefined}
                aria-current={isActive(item.href) ? 'page' : undefined}
              >
                {#if isActive(item.href)}
                  <span
                    class="absolute inset-y-1 right-0 w-[3px] rounded-full"
                    style="background:#82bf4b"
                    aria-hidden="true"
                  ></span>
                {/if}
                <span class="text-base leading-none" aria-hidden="true">{item.icon}</span>
                {#if !collapsed}
                  <span class="truncate">{item.label}</span>
                {/if}
              </a>
            </li>
          {/each}
        </ul>
      </div>
    {/each}
  </nav>

  <!-- user + logout -->
  <div class="px-2 py-3" style="border-top:1px solid rgba(255,255,255,.08)">
    {#if !collapsed && $sessionUser}
      <div class="mb-2 flex items-center gap-2.5 px-2">
        <div
          class="flex h-8 w-8 flex-none items-center justify-center rounded-full text-[13px] font-bold"
          style="background:rgba(255,255,255,.14)"
          aria-hidden="true"
        >
          {$sessionUser.full_name.trim().charAt(0) || '؟'}
        </div>
        <div class="min-w-0 flex-1">
          <div class="truncate text-[13px] font-semibold">{$sessionUser.full_name}</div>
          <div class="truncate text-[11px] text-white/50">@{$sessionUser.username}</div>
        </div>
      </div>
    {/if}
    <button
      class="flex w-full items-center justify-center gap-2 rounded-lg px-3 py-2 text-[13px] font-semibold transition-colors"
      class:justify-center={collapsed}
      style="background:rgba(255,255,255,.08); color:rgba(255,255,255,.82)"
      title={collapsed ? 'تسجيل الخروج' : undefined}
      onclick={logout}
    >
      <span aria-hidden="true">⇥</span>
      {#if !collapsed}<span>تسجيل الخروج</span>{/if}
    </button>
    {#if !collapsed}
      <div class="mt-2 px-2 text-center text-[10.5px] text-white/35">
        الزيتونة 0.1.0 • يعمل محلياً
      </div>
    {/if}
  </div>
</aside>
