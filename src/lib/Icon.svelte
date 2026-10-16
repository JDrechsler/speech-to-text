<script lang="ts">
  let { name, size = 11 }: { name: "lock" | "cloud" | "warning"; size?: number } = $props();
</script>

<svg viewBox="0 0 16 16" width={size} height={size} aria-hidden="true">
  {#if name === "lock"}
    <path
      d="M4.5 7V5a3.5 3.5 0 0 1 7 0v2h.5a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1zm1.5 0h4V5a2 2 0 0 0-4 0z"
    />
  {:else if name === "cloud"}
    <path
      d="M4.4 13A3.4 3.4 0 0 1 4 6.2a4.6 4.6 0 0 1 8.9.9A3 3 0 0 1 12 13h-3V9.4l1.3 1.3 1-1L8 6.4 4.7 9.7l1 1L7 9.4V13z"
    />
  {:else}
    <path d="M8 1.5 15 14H1zM7.2 6v4h1.6V6zm0 5.2v1.6h1.6v-1.6z" />
  {/if}
</svg>

<style>
  svg {
    flex: none;
    fill: currentColor;
    vertical-align: -1px;
  }
</style>
