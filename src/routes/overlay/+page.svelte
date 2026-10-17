<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { api, isMac, type OverlayState } from "$lib/api";
  import PrivacyBadge from "$lib/PrivacyBadge.svelte";

  const MAX_BARS = 96;
  const BAR_WIDTH = 4;
  const BAR_GAP = 3;
  const WAVE_HEIGHT = 48;
  const hint = isMac
    ? "⌥Space finish & paste   ·   ⌥Esc cancel"
    : "Alt+Space finish & paste   ·   Alt+Esc cancel";

  let overlay = $state<OverlayState | null>(null);
  let levels = $state<number[]>([]);
  let waveWidth = $state(400);

  let barCount = $derived(Math.max(1, Math.floor((waveWidth + BAR_GAP) / (BAR_WIDTH + BAR_GAP))));
  let bars = $derived.by(() => {
    const visible = levels.slice(-barCount);
    const padding = new Array(barCount - visible.length).fill(0);
    return [...padding, ...visible].map((level) => Math.max(4, Math.sqrt(level) * WAVE_HEIGHT));
  });

  onMount(() => {
    api.overlayState().then((current) => (overlay = current));
    const unlistenState = listen<OverlayState>("overlay-state", (event) => {
      if (event.payload.phase === "recording" && overlay?.phase !== "recording") levels = [];
      overlay = event.payload;
    });
    const unlistenLevel = listen<number>("level", (event) => {
      const next = [...levels, event.payload];
      levels = next.length > MAX_BARS ? next.slice(next.length - MAX_BARS) : next;
    });
    const dragWindow = (event: MouseEvent) => {
      if (event.button === 0) getCurrentWindow().startDragging();
    };
    if (!isMac) window.addEventListener("mousedown", dragWindow);
    return () => {
      unlistenState.then((stop) => stop());
      unlistenLevel.then((stop) => stop());
      window.removeEventListener("mousedown", dragWindow);
    };
  });
</script>

<div class="panel">
  <div class="top">
    <span class="phase">{overlay?.phase === "finishing" ? "Finishing…" : "Listening…"}</span>
    <div class="wave" bind:clientWidth={waveWidth}>
      {#each bars as height, index (index)}
        <span class="bar" style:height="{height}px"></span>
      {/each}
    </div>
  </div>
  <div class="bottom">
    {#if overlay?.status}
      <span class="status">{overlay.status}</span>
    {:else if overlay?.contextLine}
      <span class="context">{overlay.contextLine}</span>
    {:else}
      <span class="hint">{hint}</span>
    {/if}
    <span class="spacer"></span>
    {#if overlay}
      <PrivacyBadge isLocal={overlay.engineBadge.isLocal} text={overlay.engineBadge.text} />
      {#if overlay.rewriteBadge}
        <PrivacyBadge isLocal={overlay.rewriteBadge.isLocal} text={overlay.rewriteBadge.text} />
      {/if}
    {/if}
    {#if overlay?.phase === "recording"}
      <span class="rec"><span class="dot"></span>REC</span>
    {/if}
  </div>
</div>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    background: transparent;
    overflow: hidden;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
    --local: #30d158;
    --cloud: #ff9f0a;
    user-select: none;
    cursor: default;
  }
  .panel {
    box-sizing: border-box;
    width: 560px;
    height: 106px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    border-radius: 18px;
    background: rgba(0, 0, 0, 0.82);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #fff;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .phase {
    font-size: 14px;
    color: #8e8e93;
    white-space: nowrap;
  }
  .wave {
    flex: 1;
    height: 48px;
    display: flex;
    align-items: center;
    gap: 3px;
    overflow: hidden;
  }
  .bar {
    flex: none;
    width: 4px;
    border-radius: 2px;
    background: rgba(10, 132, 255, 0.9);
    transition: height 50ms linear;
  }
  .bottom {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
  }
  .status {
    color: #ffd60a;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  .context {
    color: #aeaeb2;
    font-weight: 500;
  }
  .hint {
    color: #8e8e93;
    white-space: pre;
  }
  .spacer {
    flex: 1;
  }
  .rec {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: #ff453a;
    font-weight: 600;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #ff453a;
  }
</style>
