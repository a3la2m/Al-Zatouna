<script lang="ts">
  export let flipped = false;
</script>

<svg
  width="20"
  height="20"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  class:rotate-180={flipped}
>
  <path d="M9 6l6 6-6 6" />
</svg>
