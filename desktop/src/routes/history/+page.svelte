<script lang="ts">
  import "$lib/settings/theme.css";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { message } from "@tauri-apps/plugin-dialog";
  import {
    api,
    errorText,
    isMac,
    type EngineId,
    type EngineInfo,
    type EngineStatus,
    type RecordingEntry,
    type RewriteCatalog,
    type Settings,
  } from "$lib/api";
  import HistoryRow from "$lib/history/HistoryRow.svelte";

  let entries = $state<RecordingEntry[]>([]);
  let loaded = $state(false);
  let engines = $state<EngineInfo[]>([]);
  let statuses = $state<EngineStatus[]>([]);
  let settings = $state<Settings | null>(null);
  let catalog = $state<RewriteCatalog | null>(null);
  let busyId = $state<string | null>(null);
  let playingId = $state<string | null>(null);

  let audio: HTMLAudioElement | null = null;
  let audioUrl: string | null = null;

  async function reload() {
    [entries, statuses, settings] = await Promise.all([api.historyEntries(), api.engineStatus(), api.settings()]);
    loaded = true;
  }

  function stopPlayback() {
    audio?.pause();
    audio = null;
    if (audioUrl) URL.revokeObjectURL(audioUrl);
    audioUrl = null;
    playingId = null;
  }

  async function togglePlayback(entry: RecordingEntry) {
    if (playingId === entry.id) return stopPlayback();
    stopPlayback();
    try {
      const bytes = await api.recordingAudio(entry.id);
      audioUrl = URL.createObjectURL(new Blob([bytes], { type: "audio/wav" }));
      audio = new Audio(audioUrl);
      audio.onended = stopPlayback;
      playingId = entry.id;
      await audio.play();
    } catch (error) {
      stopPlayback();
      await message(errorText(error), { title: "Could not play the recording", kind: "error" });
    }
  }

  async function retranscribe(entry: RecordingEntry, engine: EngineId) {
    busyId = entry.id;
    try {
      const updated = await api.retranscribe(entry.id, engine);
      entries = entries.map((item) => (item.id === updated.id ? updated : item));
    } catch (error) {
      await message(errorText(error), { title: "Re-transcription failed", kind: "error" });
    }
    busyId = null;
  }

  async function remove(entry: RecordingEntry) {
    if (playingId === entry.id) stopPlayback();
    await api.deleteHistoryEntry(entry.id);
    entries = entries.filter((item) => item.id !== entry.id);
  }

  onMount(() => {
    Promise.all([api.engines(), api.rewriteCatalog()]).then(([loadedEngines, loadedCatalog]) => {
      engines = loadedEngines;
      catalog = loadedCatalog;
    });
    reload();
    const unlistenChanges = listen("history-changed", reload);
    const unlistenFocus = getCurrentWindow().onFocusChanged(({ payload: focused }) => focused && reload());
    return () => {
      stopPlayback();
      unlistenChanges.then((stop) => stop());
      unlistenFocus.then((stop) => stop());
    };
  });
</script>

{#if loaded && settings}
  {#if entries.length === 0}
    <div class="empty">
      <svg viewBox="0 0 24 24" width="34" height="34" aria-hidden="true">
        <path d="M3 10h2v4H3zm4-4h2v12H7zm4-3h2v18h-2zm4 5h2v8h-2zm4 3h2v2h-2z" />
      </svg>
      <span>No recordings yet</span>
      <span class="note">Press {isMac ? "⌥Space" : "Alt+Space"} anywhere to start dictating.</span>
    </div>
  {:else}
    <main class="list">
      <div class="group">
        {#each entries as entry (entry.id)}
          <HistoryRow
            {entry}
            {engines}
            {statuses}
            styles={catalog?.styles ?? []}
            currentEngine={settings.engine}
            isPlaying={playingId === entry.id}
            isBusy={busyId === entry.id}
            onplay={() => togglePlayback(entry)}
            onretranscribe={(engine) => retranscribe(entry, engine)}
            oncopy={() => api.copyText(entry.rewritten ?? entry.transcript)}
            onreveal={() => api.revealRecording(entry.id)}
            ondelete={() => remove(entry)}
          />
        {/each}
      </div>
    </main>
  {/if}
{/if}

<style>
  .empty {
    height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    color: var(--secondary);
  }
  .empty svg {
    fill: currentColor;
    margin-bottom: 4px;
  }
  .list {
    max-width: none;
    padding: 14px;
  }
</style>
