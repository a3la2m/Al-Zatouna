<script lang="ts">
  /// شعار الزيتونة — حبة زيتون مستوحاة من الهوية البصرية للبرنامج.
  /// `icon` = رمز فقط، `full` = رمز + الاسم، `light` = للخلفيات الداكنة.
  let {
    size = 40,
    variant = 'full',
    light = false
  }: {
    size?: number;
    variant?: 'icon' | 'full' | 'mark';
    light?: boolean;
  } = $props();

  const id = `olive-${Math.random().toString(36).slice(2, 9)}`;
</script>

<div class="ec-brand" class:is-light={light} style="--brand-size:{size}px">
  <svg
    class="ec-brand-mark"
    width={size}
    height={size}
    viewBox="0 0 64 64"
    role="img"
    aria-label="شعار الزيتونة"
  >
    <defs>
      <radialGradient id={`${id}-body`} cx="35%" cy="28%" r="80%">
        <stop offset="0%" stop-color="#c9d94a" />
        <stop offset="45%" stop-color="#8fae20" />
        <stop offset="100%" stop-color="#4e6b0a" />
      </radialGradient>
      <linearGradient id={`${id}-leaf`} x1="0" y1="0" x2="1" y2="1">
        <stop offset="0%" stop-color="#7fa825" />
        <stop offset="100%" stop-color="#3f6410" />
      </linearGradient>
      <linearGradient id={`${id}-shine`} x1="0" y1="0" x2="0" y2="1">
        <stop offset="0%" stop-color="#ffffff" stop-opacity=".92" />
        <stop offset="100%" stop-color="#ffffff" stop-opacity=".15" />
      </linearGradient>
    </defs>

    <!-- ورقة صغيرة أعلى الحبة -->
    <path
      d="M38 9 C45 4 53 6 55 8 C50 15 43 18 37 16 Z"
      fill={`url(#${id}-leaf)`}
    />
    <path d="M39 12 C44 10 49 9 53 9" stroke="#cfe08a" stroke-width="1" fill="none" opacity=".6" />

    <!-- جسم الحبة -->
    <path
      d="M32 14 C44 14 55 25 55 37 C55 50 45 59 32 59 C19 59 9 50 9 37 C9 25 20 14 32 14 Z"
      fill={`url(#${id}-body)`}
    />

    <!-- تجويف صغير أعلى الحبة (كما في الصورة) -->
    <ellipse cx="42" cy="22" rx="6.5" ry="4" transform="rotate(-28 42 22)" fill="#3a5208" />

    <!-- لمعة بيضاء على شكل قطرة -->
    <path
      d="M19 28 C21 22 27 19 31 20 C32 22 27 26 24 32 C21 34 19 32 19 28 Z"
      fill="#ffffff"
      opacity=".92"
    />
    <ellipse cx="19" cy="42" rx="2" ry="3" fill="#ffffff" opacity=".85" transform="rotate(-18 19 42)" />
  </svg>

  {#if variant !== 'icon'}
    <div class="ec-brand-text">
      <div class="ec-brand-name">الزيتونة</div>
      {#if variant === 'full'}
        <div class="ec-brand-tag">نظام إدارة المركز التعليمي</div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .ec-brand {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    direction: rtl;
  }
  .ec-brand-mark {
    flex: none;
    filter: drop-shadow(0 2px 4px rgba(40, 60, 5, 0.25));
  }
  .ec-brand-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .ec-brand-name {
    font-weight: 800;
    font-size: calc(var(--brand-size) * 0.46);
    line-height: 1.1;
    color: var(--ec-navy);
    letter-spacing: -0.02em;
  }
  .ec-brand-tag {
    font-size: calc(var(--brand-size) * 0.26);
    line-height: 1.2;
    color: var(--ec-text-3);
    white-space: nowrap;
  }
  .ec-brand.is-light .ec-brand-name { color: #fff; }
  .ec-brand.is-light .ec-brand-tag { color: rgba(255, 255, 255, 0.6); }
</style>
