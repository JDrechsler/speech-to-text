<script lang="ts">
  import { api, errorText, thisDevice, type CredentialField } from "$lib/api";

  let {
    field,
    isSaved,
    publicValue = null,
    onchange,
  }: {
    field: CredentialField;
    isSaved: boolean;
    publicValue?: string | null;
    onchange: () => void;
  } = $props();

  let draft = $state("");
  let editing = $state(false);
  let failure = $state<string | null>(null);

  function startEditing() {
    draft = publicValue ?? "";
    editing = true;
  }

  function stopEditing() {
    draft = "";
    editing = false;
  }

  async function save() {
    if (!draft.trim()) return;
    try {
      await api.saveCredential(field.id, draft);
      failure = null;
      stopEditing();
      onchange();
    } catch (error) {
      failure = errorText(error);
    }
  }

  async function remove() {
    try {
      await api.removeCredential(field.id);
      onchange();
    } catch (error) {
      failure = errorText(error);
    }
  }
</script>

<div class="field-row">
  <span class="field-label">{field.label}</span>
  {#if isSaved && !editing}
    {#if field.isSecret}
      <span class="saved">Saved, encrypted on {thisDevice}</span>
    {:else}
      <span class="saved value" title={publicValue ?? ""}>{publicValue}</span>
    {/if}
    <span class="spacer"></span>
    <button onclick={startEditing}>Change</button>
    <button class="danger" onclick={remove}>Remove</button>
  {:else}
    <input
      type={field.isSecret ? "password" : "text"}
      placeholder={field.placeholder}
      autocomplete="off"
      spellcheck="false"
      bind:value={draft}
      onkeydown={(event) => event.key === "Enter" && save()}
    />
    <button onclick={save} disabled={!draft.trim()}>Save</button>
    {#if editing}
      <button onclick={stopEditing}>Cancel</button>
    {/if}
  {/if}
</div>
{#if failure}
  <span class="error-text">{failure}</span>
{/if}

<style>
  .saved {
    color: var(--secondary);
  }
  .value {
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
  }
</style>
