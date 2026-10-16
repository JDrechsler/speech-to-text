<script lang="ts">
  import { api, STEPFUN_LANGUAGES, type EngineInfo, type Settings } from "$lib/api";
  import CredentialRow from "./CredentialRow.svelte";

  const AZURE_ENDPOINT = "AZURE_SPEECH_ENDPOINT";

  let {
    engine,
    settings,
    saved,
    onchange,
  }: {
    engine: EngineInfo;
    settings: Settings;
    saved: string[];
    onchange: () => void;
  } = $props();

  async function setStepfunLanguage(code: string) {
    await api.setStepfunLanguage(code);
    onchange();
  }
</script>

<span class="note">{engine.summary}</span>
{#each engine.credentialFields as field (field.id)}
  <CredentialRow
    {field}
    isSaved={saved.includes(field.id)}
    publicValue={field.id === AZURE_ENDPOINT ? settings.azureEndpoint : null}
    {onchange}
  />
{/each}
{#if engine.id === "stepfun"}
  <label class="field-row">
    <span>Spoken language</span>
    <select value={settings.stepfunLanguage} onchange={(event) => setStepfunLanguage(event.currentTarget.value)}>
      {#each STEPFUN_LANGUAGES as option (option.code)}
        <option value={option.code}>{option.name}</option>
      {/each}
    </select>
  </label>
{/if}
<div class="field-row note">
  {#if engine.keyHelpUrl}
    <button class="link" onclick={() => api.openUrl(engine.keyHelpUrl ?? "")}>How to get a key</button>
  {/if}
  {#if engine.privacyPolicyUrl}
    <button class="link" onclick={() => api.openUrl(engine.privacyPolicyUrl ?? "")}>
      {engine.provider} privacy policy
    </button>
  {/if}
</div>
