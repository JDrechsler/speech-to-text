<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { api, errorText, formatBytes, type Language, type WhisperModel } from "$lib/api";

  let {
    models,
    progress = $bindable(),
    failures = $bindable(),
    isEngineSelected,
    language,
    languages,
    onchange,
  }: {
    models: WhisperModel[];
    progress: Record<string, number>;
    failures: Record<string, string>;
    isEngineSelected: boolean;
    language: string;
    languages: Language[];
    onchange: () => void;
  } = $props();

  async function use(model: WhisperModel) {
    await api.selectWhisperModel(model.id);
    onchange();
  }

  function download(model: WhisperModel) {
    progress[model.id] = 0;
    delete failures[model.id];
    api.downloadWhisperModel(model.id).catch((error) => {
      if (errorText(error) !== "Download cancelled") failures[model.id] = errorText(error);
    });
  }

  async function remove(model: WhisperModel) {
    const confirmed = await ask("You can download it again at any time.", {
      title: `Delete ${model.name}?`,
      kind: "warning",
      okLabel: `Delete ${formatBytes(model.bytes)}`,
      cancelLabel: "Cancel",
    });
    if (!confirmed) return;
    try {
      await api.deleteWhisperModel(model.id);
    } catch (error) {
      failures[model.id] = errorText(error);
    }
    onchange();
  }

  async function setLanguage(code: string) {
    await api.setWhisperLanguage(code);
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
        <button onclick={() => api.cancelWhisperDownload(model.id)}>Cancel</button>
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
<span class="note">Bigger models are more accurate and use more memory. Hover a model for details.</span>
<label class="field-row">
  <span>Spoken language</span>
  <select value={language} onchange={(event) => setLanguage(event.currentTarget.value)}>
    {#each languages as option (option.code)}
      <option value={option.code}>{option.name}</option>
    {/each}
  </select>
</label>

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
