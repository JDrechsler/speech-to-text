<script lang="ts">
  import {
    engineOfHistoryLabel,
    isMac,
    thisDevice,
    type EngineId,
    type EngineInfo,
    type EngineStatus,
    type RecordingEntry,
    type StyleInfo,
  } from "$lib/api";
  import { engineMenuTitle } from "$lib/settings/profiles";

  const dateFormat = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

  let {
    entry,
    engines,
    statuses,
    styles,
    currentEngine,
    isPlaying,
    isBusy,
    onplay,
    onretranscribe,
    oncopy,
    onreveal,
    ondelete,
  }: {
    entry: RecordingEntry;
    engines: EngineInfo[];
    statuses: EngineStatus[];
    styles: StyleInfo[];
    currentEngine: EngineId;
    isPlaying: boolean;
    isBusy: boolean;
    onplay: () => void;
    onretranscribe: (engine: EngineId) => void;
    oncopy: () => void;
    onreveal: () => void;
    ondelete: () => void;
  } = $props();

  let defaultEngine = $derived(engineOfHistoryLabel(entry.engine) ?? currentEngine);
  let defaultEngineInfo = $derived(engines.find((engine) => engine.id === defaultEngine));
  let styleTitle = $derived(styles.find((style) => style.id === entry.rewriteStyle)?.title ?? entry.rewriteStyle);
  let duration = $derived.by(() => {
    const seconds = Math.round(entry.duration);
    return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
  });

  function isReady(id: EngineId) {
    return statuses.find((status) => status.id === id)?.setupHint === null;
  }
</script>

<div class="block entry">
  <div class="head">
    <button class="plain play" title={isPlaying ? "Stop" : "Play recording"} onclick={onplay}>
      {isPlaying ? "■" : "▶"}
    </button>
    <span class="date">{dateFormat.format(new Date(entry.createdAt))}</span>
    {#if entry.duration > 0}
      <span class="note">{duration}</span>
    {/if}
    <span class="chip">{entry.engine}</span>
    <span class="spacer"></span>
    {#if isBusy}
      <span class="note">Transcribing…</span>
    {:else}
      <div class="split">
        <button
          title="Run again with {defaultEngineInfo ? engineMenuTitle(defaultEngineInfo) : defaultEngine}"
          disabled={!isReady(defaultEngine)}
          onclick={() => onretranscribe(defaultEngine)}
        >
          Re-transcribe
        </button>
        <span class="caret" aria-hidden="true">▾</span>
        <select
          value=""
          aria-label="Re-transcribe with another engine"
          title="Pick another engine"
          onchange={(event) => {
            const engine = event.currentTarget.value as EngineId;
            event.currentTarget.value = "";
            if (engine) onretranscribe(engine);
          }}
        >
          <option value="" disabled>Engine…</option>
          <optgroup label="On {thisDevice}">
            {#each engines.filter((engine) => engine.isLocal) as engine (engine.id)}
              <option value={engine.id} disabled={!isReady(engine.id)}>{engineMenuTitle(engine)}</option>
            {/each}
          </optgroup>
          <optgroup label="Cloud · uploads this recording">
            {#each engines.filter((engine) => !engine.isLocal) as engine (engine.id)}
              <option value={engine.id} disabled={!isReady(engine.id)}>{engineMenuTitle(engine)}</option>
            {/each}
          </optgroup>
        </select>
      </div>
      <button title="Copy transcript" onclick={oncopy}>Copy</button>
      <button title={isMac ? "Reveal audio file in Finder" : "Show audio file"} onclick={onreveal}>
        Show
      </button>
      <button class="danger" title="Delete this recording" onclick={ondelete}>Delete</button>
    {/if}
  </div>
  {#if entry.transcript === ""}
    <p class="missing">No transcript. Use Re-transcribe to recover the text.</p>
  {:else}
    <p class="text">{entry.rewritten ?? entry.transcript}</p>
    {#if entry.rewritten && entry.rewriteStyle}
      <p class="note heard">Rewritten as {styleTitle}. Heard: {entry.transcript}</p>
    {/if}
  {/if}
</div>

<style>
  .entry {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .play {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 1.5px solid var(--accent);
    color: var(--accent);
    font-size: 9px;
    text-align: center;
  }
  .date {
    font-weight: 500;
  }
  .chip {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 999px;
    color: var(--secondary);
    background: var(--segment-bg);
  }
  .split {
    position: relative;
    display: flex;
  }
  .caret {
    position: absolute;
    right: 0;
    width: 22px;
    top: 50%;
    transform: translateY(-50%);
    text-align: center;
    font-size: 10px;
    color: var(--secondary);
    pointer-events: none;
    z-index: 1;
  }
  .split button {
    border-top-right-radius: 0;
    border-bottom-right-radius: 0;
  }
  .split select {
    appearance: none;
    width: 22px;
    padding: 3px 0;
    border-left: none;
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
    color: transparent;
  }
  .split select option,
  .split select optgroup {
    color: var(--text);
  }
  p {
    margin: 0;
    user-select: text;
    cursor: text;
  }
  .text {
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .heard {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .missing {
    color: var(--secondary);
    font-style: italic;
  }
</style>
