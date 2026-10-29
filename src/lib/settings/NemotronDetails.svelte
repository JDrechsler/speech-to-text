<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { api, errorText, formatBytes, type WhisperModel } from "$lib/api";

  let {
    models,
    progress = $bindable(),
    failures = $bindable(),
    isEngineSelected,
    onchange,
  }: {
    models: WhisperModel[];
    progress: Record<string, number>;
    failures: Record<string, string>;
    isEngineSelected: boolean;
    onchange: () => void;
  } = $props();

  async function use(model: WhisperModel) {
    await api.selectNemotronModel(model.id);
    onchange();
  }

  function download(model: WhisperModel) {
    progress[model.id] = 0;
    delete failures[model.id];
    api.downloadNemotron(model.id).catch((error) => {
      delete progress[model.id];
      if (errorText(error) !== "Download cancelled") failures[model.id] = errorText(error);
    });
  }

  async function remove(model: WhisperModel) {
    const confirmed = await ask("You can download it again at any time.", {
      title: `Delete Nemotron ${model.name}?`,
      kind: "warning",
      okLabel: "Delete",
      cancelLabel: "Cancel",
    });
    if (!confirmed) return;
    try {
      await api.deleteNemotron(model.id);
    } catch (error) {
      failures[model.id] = errorText(error);
    }
    onchange();
  }
</script>

{#each models as model (model.id)}
  <div class="model" title={model.detail}>
    <div class="field-row">
      <span>{model.name}</span>
      <span class="size">{formatBytes(model.bytes)}</span>
      {#if model.isRecommended}
        <span class="recommended">Recommended</span>
      {/if}
      <span class="spacer"></span>
      {#if progress[model.id] !== undefined}
        <progress max="1" value={progress[model.id]}></progress>
        <span class="percent">{Math.round(progress[model.id] * 100)}%</span>
        <button onclick={() => api.cancelNemotronDownload(model.id)}>Cancel</button>
      {:else if model.isInstalled}
        {#if isEngineSelected && model.isSelected}
          <span class="local-text">✓ In use</span>
        {:else}
          <button onclick={() => use(model)}>Use</button>
        {/if}
        <button class="danger" title="Delete this model" onclick={() => remove(model)}>Delete</button>
      {:else}
        <button onclick={() => download(model)}>Download</button>
      {/if}
    </div>
    {#if failures[model.id]}
      <span class="error-text">{failures[model.id]}</span>
    {/if}
  </div>
{/each}
<span class="note">
  Hover a model for how much memory it needs. It loads when you start recording and frees the memory after 5 idle
  minutes. Runs with NVIDIA's NeMo-Speech.cpp; the model files are a community conversion by AtomicChat. Your
  dictionary is not used with this engine, and it detects the spoken language by itself.
</span>

<style>
  .model {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .size {
    color: var(--secondary);
  }
  .recommended {
    font-size: 10px;
    font-weight: 600;
    padding: 1px 6px;
    border-radius: 999px;
    color: var(--accent);
    background: var(--accent-soft);
  }
  progress {
    width: 90px;
  }
  .percent {
    width: 34px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
</style>
