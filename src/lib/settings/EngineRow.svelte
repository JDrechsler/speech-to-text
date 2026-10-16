<script lang="ts">
  import type { Snippet } from "svelte";
  import type { EngineInfo } from "$lib/api";
  import PrivacyBadge from "$lib/PrivacyBadge.svelte";
  import Chevron from "./Chevron.svelte";

  let {
    engine,
    selected,
    statusText,
    ready,
    showsSelector = true,
    open = $bindable(),
    onselect,
    children,
  }: {
    engine: EngineInfo;
    selected: boolean;
    statusText: string;
    ready: boolean;
    showsSelector?: boolean;
    open?: boolean;
    onselect?: () => void;
    children: Snippet;
  } = $props();
</script>

<div class="block">
  <div class="head">
    {#if showsSelector}
      <button
        class="plain radio"
        class:on={selected}
        aria-pressed={selected}
        aria-label="Use {engine.displayName}"
        title={selected ? "In use" : `Use ${engine.displayName}`}
        onclick={onselect}
      ></button>
    {/if}
    <button class="plain toggle" aria-expanded={open ?? false} onclick={() => (open = !open)}>
      <span class="name">{engine.displayName}</span>
      {#if !engine.isLocal}
        <span class="provider">{engine.provider}</span>
      {/if}
      <PrivacyBadge isLocal={engine.isLocal} text={engine.isLocal ? "Offline" : "Uploads audio"} />
      <span class="spacer"></span>
      <span class="status" class:attention={!ready && selected}>
        {#if ready}<span class="check">✓</span>{/if}
        {statusText}
      </span>
      <Chevron open={open ?? false} />
    </button>
  </div>
  {#if open}
    <div class="details" class:indented={showsSelector}>
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .radio {
    flex: none;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 1.5px solid var(--tertiary);
    box-sizing: border-box;
  }
  .radio.on {
    border: 5px solid var(--accent);
  }
  .toggle {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .name {
    font-weight: 600;
  }
  .provider {
    color: var(--secondary);
  }
  .status {
    color: var(--secondary);
    white-space: nowrap;
  }
  .status.attention {
    color: var(--warning);
  }
  .check {
    color: var(--local);
  }
  .indented {
    padding-left: 26px;
  }
</style>
