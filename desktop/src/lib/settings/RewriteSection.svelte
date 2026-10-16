<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import {
    api,
    APPLE_REWRITER_ID,
    errorText,
    thisDevice,
    type ProfileSettings,
    type RewriteCatalog,
    type RewriteTargetInfo,
  } from "$lib/api";
  import Icon from "$lib/Icon.svelte";
  import ModelRow from "./ModelRow.svelte";
  import { isRewriteInUse } from "./profiles";

  let {
    profiles = $bindable(),
    targets,
    catalog,
    flush,
    onreplace,
    onkeychange,
  }: {
    profiles: ProfileSettings;
    targets: RewriteTargetInfo[];
    catalog: RewriteCatalog;
    flush: () => Promise<void>;
    onreplace: (settings: ProfileSettings) => void;
    onkeychange: () => void;
  } = $props();

  let expanded = $state<Record<string, boolean>>({});
  let failure = $state<string | null>(null);

  let inUse = $derived(isRewriteInUse(profiles));
  let summary = $derived(catalog.styles.find((style) => style.id === profiles.defaultStyle)?.summary ?? "");
  let selectableTargets = $derived(
    targets.filter((target) => target.id !== APPLE_REWRITER_ID || target.id === profiles.defaultRewriterID),
  );
  let chosen = $derived(targets.find((target) => target.id === profiles.defaultRewriterID));

  async function addModel(presetName: string) {
    try {
      await flush();
      const updated = await api.addRewriteModel(presetName);
      const added = updated.models[updated.models.length - 1];
      if (added) expanded[added.id] = true;
      onreplace(updated);
      failure = null;
    } catch (error) {
      failure = errorText(error);
    }
  }

  async function removeModel(id: string, name: string) {
    const confirmed = await ask("Its saved key is deleted too.", {
      title: `Remove ${name || "this model"}?`,
      kind: "warning",
      okLabel: "Remove",
      cancelLabel: "Cancel",
    });
    if (!confirmed) return;
    try {
      await flush();
      onreplace(await api.removeRewriteModel(id));
      failure = null;
    } catch (error) {
      failure = errorText(error);
    }
  }
</script>

<section class="section">
  <h2>Rewrite before pasting</h2>
  <div class="group">
    <label class="row">
      <span class="label">
        Style
        <span class="sub">{summary}</span>
      </span>
      <select bind:value={profiles.defaultStyle}>
        {#each catalog.styles as style (style.id)}
          <option value={style.id}>{style.title}</option>
        {/each}
      </select>
    </label>
    {#if profiles.defaultStyle === "custom"}
      <div class="block">
        <textarea
          rows="3"
          placeholder="Tell the model what to do, for example: Translate to English."
          bind:value={profiles.defaultCustomInstructions}
        ></textarea>
      </div>
    {/if}
    {#if inUse}
      <div class="block">
        <label class="field-row">
          <span>Rewrite with</span>
          <span class="spacer"></span>
          <select bind:value={profiles.defaultRewriterID}>
            {#if selectableTargets.length === 0}
              <option value={profiles.defaultRewriterID} disabled>Add a model below</option>
            {/if}
            {#each selectableTargets as target (target.id)}
              <option value={target.id}>
                {target.isReady ? target.menuTitle : `${target.menuTitle} (set up…)`}
              </option>
            {/each}
          </select>
        </label>
        {#if chosen && chosen.id !== APPLE_REWRITER_ID}
          <p class="note" class:local-text={chosen.isLocal} class:cloud-text={!chosen.isLocal}>
            <Icon name={chosen.isLocal ? "lock" : "cloud"} />
            {chosen.privacyStatement}
          </p>
        {/if}
        {#if chosen && !chosen.isReady}
          <p class="note warning-text">
            <Icon name="warning" />
            {chosen.id === APPLE_REWRITER_ID ? "Add a model below to rewrite your dictation." : chosen.setupHint}
          </p>
        {/if}
      </div>
      {#each profiles.models as model, index (model.id)}
        <ModelRow
          bind:model={profiles.models[index]}
          bind:open={expanded[model.id]}
          apis={catalog.apis}
          hasKey={targets.find((target) => target.id === model.id)?.hasKey ?? false}
          onremove={() => removeModel(model.id, model.name)}
          {onkeychange}
        />
      {/each}
      <div class="row">
        <select
          value=""
          aria-label="Add model"
          onchange={(event) => {
            const preset = event.currentTarget.value;
            event.currentTarget.value = "";
            if (preset) addModel(preset);
          }}
        >
          <option value="" disabled>Add model…</option>
          <optgroup label="Cloud · sends the text">
            {#each catalog.cloudPresets as preset (preset.name)}
              <option value={preset.name}>{preset.name}</option>
            {/each}
          </optgroup>
          <optgroup label="On {thisDevice} · private">
            {#each catalog.localPresets as preset (preset.name)}
              <option value={preset.name}>{preset.name}</option>
            {/each}
          </optgroup>
          <optgroup label="Other">
            <option value={catalog.otherPreset.name}>Any OpenAI-compatible server…</option>
          </optgroup>
        </select>
        <span class="note">Use your own key with Claude, OpenAI, Gemini and more, or a model running on {thisDevice}.</span>
      </div>
    {/if}
  </div>
  {#if failure}
    <p class="note error-text">{failure}</p>
  {:else if !inUse}
    <p class="note">A language model can clean up or reformat what you said. Apps can have their own style below.</p>
  {/if}
</section>

<style>
  .block p {
    margin: 6px 0 0;
    display: flex;
    gap: 5px;
    align-items: baseline;
  }
</style>
