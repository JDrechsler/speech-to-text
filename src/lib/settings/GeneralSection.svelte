<script lang="ts">
  import {
    api,
    errorText,
    isMac,
    thisDevice,
    type GeneralStatus,
    type Microphones,
    type Settings,
  } from "$lib/api";

  const RELEASES_URL = "https://github.com/JDrechsler/speech-to-text/releases";
  const SYSTEM_DEFAULT = "default";
  const BUILT_IN = "builtin";
  const shortcuts = isMac ? "⌥ Space start and stop · ⌥ Esc cancel" : "Alt+Space start and stop · Alt+Esc cancel";

  let {
    settings,
    status,
    microphones,
    onchange,
  }: {
    settings: Settings;
    status: GeneralStatus;
    microphones: Microphones;
    onchange: () => void;
  } = $props();

  let failure = $state<string | null>(null);

  let microphoneChoice = $derived(settings.microphone ?? (isMac ? BUILT_IN : SYSTEM_DEFAULT));
  let words = $derived(status.dictionaryTerms === 1 ? "1 word" : `${status.dictionaryTerms} words`);
  let builtIn = $derived(microphones.devices.find((device) => device.isBuiltIn));
  let missingChoice = $derived(
    microphoneChoice !== SYSTEM_DEFAULT &&
      microphoneChoice !== BUILT_IN &&
      !microphones.devices.some((device) => device.name === microphoneChoice),
  );

  async function run(action: () => Promise<unknown>) {
    try {
      await action();
      failure = null;
    } catch (error) {
      failure = errorText(error);
    }
    onchange();
  }
</script>

<section class="section">
  <h2>General</h2>
  <div class="group">
    <div class="row">
      <span class="label">
        Personal dictionary
        <span class="sub">
          {words} the engines should spell your way. Stays on {thisDevice}, sent along to cloud engines.
        </span>
      </span>
      <button onclick={() => run(api.openDictionary)}>Edit…</button>
    </div>
    <div class="row">
      <span class="label">Shortcuts</span>
      <span class="note">{shortcuts}</span>
    </div>
    <div class="row">
      <span class="label">Auto-paste into the active app</span>
      {#if status.accessibilityAllowed}
        <span class="local-text">✓ Allowed</span>
      {:else}
        <button onclick={() => run(api.requestAccessibility)}>Allow…</button>
      {/if}
    </div>
    <label class="row">
      <span class="label">
        Microphone
        {#if missingChoice}
          <span class="sub warning-text">Not connected, using the system default</span>
        {/if}
      </span>
      <select
        value={microphoneChoice}
        onchange={(event) => run(() => api.setMicrophone(event.currentTarget.value))}
      >
        <option value={SYSTEM_DEFAULT}>
          System default{microphones.systemDefault ? ` (${microphones.systemDefault})` : ""}
        </option>
        {#if isMac}
          <option value={BUILT_IN}>{builtIn?.name ?? "Built-in microphone"} (preferred)</option>
        {/if}
        {#each microphones.devices.filter((device) => !(isMac && device.isBuiltIn)) as device (device.name)}
          <option value={device.name}>{device.name}</option>
        {/each}
        {#if missingChoice}
          <option value={microphoneChoice}>{microphoneChoice} (not connected)</option>
        {/if}
      </select>
    </label>
    <label class="row">
      <span class="label">
        Pause media while recording
        <span class="sub">Pauses music and videos, and resumes them when you finish.</span>
      </span>
      <input
        type="checkbox"
        checked={settings.pauseMedia}
        disabled={!isMac}
        onchange={(event) => run(() => api.setPauseMedia(event.currentTarget.checked))}
      />
    </label>
    <label class="row">
      <span class="label">Launch at login</span>
      <input
        type="checkbox"
        checked={status.launchAtLogin}
        onchange={(event) => run(() => api.setLaunchAtLogin(event.currentTarget.checked))}
      />
    </label>
    <div class="row">
      <span class="label">
        Recordings
        <span class="sub">Kept on {thisDevice} for 24 hours, then deleted.</span>
      </span>
      <button onclick={() => run(api.openHistoryWindow)}>History…</button>
      <button onclick={() => run(api.openRecordings)}>Open Folder</button>
    </div>
    <div class="row">
      <span class="label">Version</span>
      <span class="note">{status.version}</span>
      <button class="link" onclick={() => run(() => api.openUrl(RELEASES_URL))}>Release notes</button>
    </div>
  </div>
  {#if failure}
    <p class="note error-text">{failure}</p>
  {/if}
</section>
