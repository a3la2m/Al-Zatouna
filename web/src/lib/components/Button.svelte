<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    variant = 'primary',
    size = 'md',
    block = false,
    type = 'button',
    disabled = false,
    loading = false,
    onclick,
    children
  }: {
    variant?: 'primary' | 'success' | 'danger' | 'ghost' | 'soft';
    size?: 'sm' | 'md' | 'lg';
    block?: boolean;
    type?: 'button' | 'submit' | 'reset';
    disabled?: boolean;
    loading?: boolean;
    onclick?: (e: MouseEvent) => void;
    children?: Snippet;
  } = $props();

  const variantClass = {
    primary: 'ec-btn-primary',
    success: 'ec-btn-success',
    danger: 'ec-btn-danger',
    ghost: 'ec-btn-ghost',
    soft: 'ec-btn-soft'
  };

  const sizeClass = { sm: 'ec-btn-sm', md: '', lg: 'ec-btn-lg' };
</script>

<button
  class="ec-btn {variantClass[variant]} {sizeClass[size]} {block ? 'ec-btn-block' : ''}"
  {type}
  disabled={disabled || loading}
  {onclick}
>
  {#if loading}
    <span class="ec-spinner" aria-hidden="true"></span>
  {/if}
  {@render children?.()}
</button>
