<script lang="ts">
  let { open }: { open: boolean } = $props();
</script>

<svg class="chevron" class:open viewBox="0 0 10 10" aria-hidden="true">
  <path
    d="M3.5 1.5 7 5l-3.5 3.5"
    fill="none"
    stroke="currentColor"
    stroke-width="1.6"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
</svg>
