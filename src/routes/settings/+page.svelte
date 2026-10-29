<script lang="ts">
  import "$lib/settings/theme.css";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    api,
    errorText,
    thisDevice,
    type DownloadProgress,
    type EngineId,
    type EngineInfo,
    type EngineStatus,
    type GeneralStatus,
    type Language,
    type Microphones,
    type ProfileSettings,
    type RewriteCatalog,
    type RewriteTargetInfo,
    type Settings,
    type WhisperModel,
  } from "$lib/api";
  import Icon from "$lib/Icon.svelte";
  import AppProfilesSection from "$lib/settings/AppProfilesSection.svelte";
  import CloudDetails from "$lib/settings/CloudDetails.svelte";
  import EngineRow from "$lib/settings/EngineRow.svelte";
  import GeneralSection from "$lib/settings/GeneralSection.svelte";
  import RewriteSection from "$lib/settings/RewriteSection.svelte";
  import WhisperDetails from "$lib/settings/WhisperDetails.svelte";

  const PROFILE_SAVE_DELAY = 350;

  let settings = $state<Settings | null>(null);
  let engines = $state<EngineInfo[]>([]);
  let statuses = $state<EngineStatus[]>([]);
  let models = $state<WhisperModel[]>([]);
  let languages = $state<Language[]>([]);
  let saved = $state<string[]>([]);
  let general = $state<GeneralStatus | null>(null);
  let microphones = $state<Microphones>({ devices: [], systemDefault: null });
  let catalog = $state<RewriteCatalog | null>(null);
  let profiles = $state<ProfileSettings | null>(null);
  let targets = $state<RewriteTargetInfo[]>([]);
  let progress = $state<Record<string, number>>({});
  let failures = $state<Record<string, string>>({});
  let expanded = $state<Record<string, boolean>>({});
  let loadError = $state<string | null>(null);

  let savedProfilesJson = "";
  let profileSaveTimer: ReturnType<typeof setTimeout> | undefined;

  let current = $derived(engines.find((engine) => engine.id === settings?.engine));
  let usesCloud = $derived(current ? !current.isLocal : false);
  let cloudEngines = $derived(engines.filter((engine) => !engine.isLocal));
  let whisper = $derived(engines.find((engine) => engine.isLocal));
  let currentNeedsSetup = $derived(
    statuses.find((status) => status.id === settings?.engine)?.setupHint !== null && statuses.length > 0,
  );
  let showsAppProfiles = $derived(general !== null && general.platform !== "linux");

  function isReady(id: EngineId) {
    return statuses.find((status) => status.id === id)?.setupHint === null;
  }

  function whisperStatus(): string {
    const downloading = Object.values(progress);
    if (downloading.length > 0) return `Downloading ${Math.round(Math.max(...downloading) * 100)}%`;
    const selected = models.find((model) => model.isSelected);
    return selected?.isInstalled ? selected.name : "No model yet";
  }

  async function reload() {
    [settings, statuses, models, saved, general, microphones, targets] = await Promise.all([
      api.settings(),
      api.engineStatus(),
      api.whisperModels(),
      api.savedCredentials(),
      api.generalStatus(),
      api.inputDevices(),
      api.rewriteTargets(),
    ]);
  }

  async function reloadProfiles() {
    replaceProfiles(await api.profileSettings());
  }

  function replaceProfiles(next: ProfileSettings) {
    savedProfilesJson = JSON.stringify(next);
    profiles = next;
    api.rewriteTargets().then((next) => (targets = next));
  }

  async function flushProfiles() {
    clearTimeout(profileSaveTimer);
    if (!profiles) return;
    const json = JSON.stringify($state.snapshot(profiles));
    if (json === savedProfilesJson) return;
    savedProfilesJson = json;
    await api.saveProfileSettings(JSON.parse(json));
    targets = await api.rewriteTargets();
  }

  $effect(() => {
    if (!profiles) return;
    const json = JSON.stringify($state.snapshot(profiles));
    if (json === savedProfilesJson) return;
    clearTimeout(profileSaveTimer);
    profileSaveTimer = setTimeout(flushProfiles, PROFILE_SAVE_DELAY);
  });

  async function chooseCloud(cloud: boolean) {
    settings = await api.useCloud(cloud);
    await reload();
    if (settings && !isReady(settings.engine)) expanded[settings.engine] = true;
  }

  async function select(id: EngineId) {
    settings = await api.setEngine(id);
    await reload();
    if (!isReady(id)) expanded[id] = true;
  }

  function showLoadError(error: unknown) {
    loadError = errorText(error);
  }

  onMount(() => {
    Promise.all([api.engines(), api.whisperLanguages(), api.rewriteCatalog()])
      .then(([loadedEngines, loadedLanguages, loadedCatalog]) => {
        engines = loadedEngines;
        languages = loadedLanguages;
        catalog = loadedCatalog;
      })
      .catch(showLoadError);
    reload()
      .then(() => {
        if (settings && !isReady(settings.engine)) expanded[settings.engine] = true;
      })
      .catch(showLoadError);
    reloadProfiles().catch(showLoadError);

    const unlistenDownload = listen<DownloadProgress>("model-download", ({ payload }) => {
      if (payload.done) {
        delete progress[payload.id];
        if (payload.error && payload.error !== "Download cancelled") failures[payload.id] = payload.error;
        reload();
      } else {
        progress[payload.id] = payload.fraction;
      }
    });
    const unlistenFocus = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) {
        reload();
      } else {
        flushProfiles();
      }
    });
    return () => {
      flushProfiles();
      unlistenDownload.then((stop) => stop());
      unlistenFocus.then((stop) => stop());
    };
  });
</script>

{#if settings && general && catalog && profiles && whisper}
  <main>
    <section class="section">
      <h2>Where your speech is transcribed</h2>
      <div class="group">
        <div class="block">
          <div class="segmented" role="radiogroup" aria-label="Transcribe">
            <button role="radio" aria-checked={!usesCloud} class:on={!usesCloud} onclick={() => chooseCloud(false)}>
              <Icon name="lock" /> On {thisDevice}
            </button>
            <button role="radio" aria-checked={usesCloud} class:on={usesCloud} onclick={() => chooseCloud(true)}>
              <Icon name="cloud" /> Cloud
            </button>
          </div>
          <p class="note privacy" class:local-text={!usesCloud} class:cloud-text={usesCloud}>
            <Icon name={usesCloud ? "cloud" : "lock"} />
            {usesCloud
              ? "Each recording is uploaded to the provider you pick. Needs your own API key."
              : `Private and offline. Your voice never leaves ${thisDevice}.`}
          </p>
          {#if currentNeedsSetup}
            <p class="setup">
              <span class="warning-text"><Icon name="warning" size={12} /></span>
              {usesCloud
                ? `Add your ${current?.provider} API key below, or pick another provider.`
                : "Download a Whisper model to start dictating."}
            </p>
          {/if}
        </div>
      </div>
    </section>

    <section class="section">
      <div class="group">
        {#if usesCloud}
          {#each cloudEngines as engine (engine.id)}
            <EngineRow
              {engine}
              selected={settings.engine === engine.id}
              ready={isReady(engine.id)}
              statusText={isReady(engine.id) ? "Ready" : "Needs API key"}
              bind:open={expanded[engine.id]}
              onselect={() => select(engine.id)}
            >
              <CloudDetails {engine} {settings} {saved} onchange={reload} />
            </EngineRow>
          {/each}
        {:else}
          <EngineRow
            engine={whisper}
            selected
            showsSelector={false}
            ready={isReady(whisper.id) && Object.keys(progress).length === 0}
            statusText={whisperStatus()}
            bind:open={expanded[whisper.id]}
          >
            <WhisperDetails
              {models}
              bind:progress
              bind:failures
              isEngineSelected={settings.engine === whisper.id}
              language={settings.whisperLanguage}
              {languages}
              onchange={reload}
            />
          </EngineRow>
        {/if}
      </div>
    </section>

    <RewriteSection
      bind:profiles
      {targets}
      {catalog}
      flush={flushProfiles}
      onreplace={replaceProfiles}
      onkeychange={reload}
    />

    {#if showsAppProfiles}
      <AppProfilesSection
        bind:profiles
        {targets}
        {catalog}
        {engines}
        defaultEngine={settings.engine}
        flush={flushProfiles}
        onreplace={replaceProfiles}
      />
    {/if}

    <GeneralSection {settings} status={general} {microphones} onchange={reload} />
  </main>
{:else if loadError}
  <main>
    <p class="load-error">Settings could not load: {loadError}</p>
  </main>
{/if}

<style>
  .segmented {
    display: flex;
    padding: 2px;
    border-radius: 7px;
    background: var(--segment-bg);
  }
  .segmented button {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border: none;
    border-radius: 5px;
    background: transparent;
    padding: 4px 10px;
  }
  .segmented button.on {
    background: var(--segment-on);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.15);
    font-weight: 500;
  }
  .privacy,
  .setup {
    margin: 10px 0 0;
    display: flex;
    gap: 6px;
    align-items: baseline;
  }
  .setup {
    font-weight: 500;
  }
  .load-error {
    color: var(--danger);
    user-select: text;
  }
</style>
