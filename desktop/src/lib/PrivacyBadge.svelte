<script lang="ts">
  import Icon from "./Icon.svelte";

  let { isLocal, text }: { isLocal: boolean; text: string } = $props();
</script>

<span class="badge" class:local={isLocal}>
  <Icon name={isLocal ? "lock" : "cloud"} />
  {text}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 7px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 600;
    white-space: nowrap;
    color: var(--cloud);
    background: color-mix(in srgb, var(--cloud) 18%, transparent);
  }
  .badge.local {
    color: var(--local);
    background: color-mix(in srgb, var(--local) 18%, transparent);
  }
</style>
