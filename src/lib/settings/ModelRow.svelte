<script lang="ts">
  import { api, errorText, modelKeyId, thisDevice, type ModelApiId, type RewriteModel } from "$lib/api";
  import PrivacyBadge from "$lib/PrivacyBadge.svelte";
  import Chevron from "./Chevron.svelte";
  import CredentialRow from "./CredentialRow.svelte";
  import { modelFacts } from "./profiles";

  let {
    model = $bindable(),
    apis,
    hasKey,
    open = $bindable(),
    onremove,
    onkeychange,
  }: {
    model: RewriteModel;
    apis: { id: ModelApiId; title: string }[];
    hasKey: boolean;
    open?: boolean;
    onremove: () => void;
    onkeychange: () => void;
  } = $props();

  let found = $state<string[]>([]);
  let finding = $state(false);
  let findError = $state<string | null>(null);

  let facts = $derived(modelFacts(model));
  let keyField = $derived({
    id: modelKeyId(model.id),
    label: "API key",
    placeholder: facts.isLocal ? "Usually not needed here" : `${model.name || "Model"} API key`,
    isSecret: true,
  });

  async function findModels() {
    finding = true;
    findError = null;
    try {
      found = await api.listModelNames($state.snapshot(model));
      if (found.length === 0) findError = `${model.name} listed no models.`;
    } catch (error) {
      findError = errorText(error);
    }
    finding = false;
  }
</script>

<div class="block">
  <button class="plain head" aria-expanded={open ?? false} onclick={() => (open = !open)}>
    <span class="name">{model.name || "Unnamed model"}</span>
    {#if model.model}
      <span class="model-id">{model.model}</span>
    {/if}
    <PrivacyBadge isLocal={facts.isLocal} text={facts.isLocal ? "Offline" : "Sends text"} />
    <span class="spacer"></span>
    <span class:warning-text={!facts.isConfigured} class="status">
      {facts.isConfigured ? "Ready" : "Needs a model"}
    </span>
    <Chevron open={open ?? false} />
  </button>
  {#if open}
    <div class="details">
      <label class="field-row">
        <span class="field-label">Name</span>
        <input type="text" placeholder="Shown in menus" bind:value={model.name} />
      </label>
      <div class="field-row">
        <span class="field-label">Address</span>
        <input type="text" placeholder="https://… /v1" spellcheck="false" bind:value={model.baseURL} />
        <select bind:value={model.api} aria-label="Format">
          {#each apis as option (option.id)}
            <option value={option.id}>{option.title}</option>
          {/each}
        </select>
      </div>
      <div class="field-row">
        <span class="field-label">Model</span>
        <input
          type="text"
          placeholder="Model name, or use Find models"
          spellcheck="false"
          bind:value={model.model}
        />
        {#if found.length > 0}
          <select
            aria-label="Pick a model"
            value=""
            onchange={(event) => {
              model.model = event.currentTarget.value;
              event.currentTarget.value = "";
            }}
          >
            <option value="" disabled>Pick…</option>
            {#each found as id (id)}
              <option value={id}>{id}</option>
            {/each}
          </select>
        {/if}
        <button onclick={findModels} disabled={finding || facts.host === null}>
          {finding ? "Finding…" : "Find models"}
        </button>
      </div>
      {#if findError}
        <span class="error-text">{findError}</span>
      {/if}
      <CredentialRow field={keyField} isSaved={hasKey} onchange={onkeychange} />
      <span class="note" class:local-text={facts.isLocal} class:cloud-text={!facts.isLocal}>
        {facts.isLocal
          ? `Runs on ${thisDevice}. The text never leaves it.`
          : `Each rewrite sends the transcript (text, not audio) to ${facts.destination}.`}
      </span>
      <div class="field-row note">
        {#if model.keyHelpURL}
          <button class="link" onclick={() => api.openUrl(model.keyHelpURL ?? "")}>
            {facts.isLocal ? `Get ${model.name}` : "How to get a key"}
          </button>
        {/if}
        <span class="spacer"></span>
        <button class="link danger" onclick={onremove}>Remove</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .head {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .name {
    font-weight: 600;
  }
  .model-id {
    color: var(--secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .status {
    color: var(--secondary);
    white-space: nowrap;
  }
  .details {
    padding-left: 12px;
  }
</style>
