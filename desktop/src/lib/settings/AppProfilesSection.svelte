<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import {
    api,
    APPLE_REWRITER_ID,
    errorText,
    isMac,
    thisDevice,
    type AppIdentity,
    type AppProfile,
    type EngineId,
    type EngineInfo,
    type ProfileSettings,
    type RewriteCatalog,
    type RewriteTargetInfo,
  } from "$lib/api";
  import Chevron from "./Chevron.svelte";
  import { engineMenuTitle, resolve } from "./profiles";

  const CHOOSE_FILE = "__choose__";
  const shortcut = isMac ? "⌥Space" : "Alt+Space";

  let {
    profiles = $bindable(),
    targets,
    catalog,
    engines,
    defaultEngine,
    flush,
    onreplace,
  }: {
    profiles: ProfileSettings;
    targets: RewriteTargetInfo[];
    catalog: RewriteCatalog;
    engines: EngineInfo[];
    defaultEngine: EngineId;
    flush: () => Promise<void>;
    onreplace: (settings: ProfileSettings) => void;
  } = $props();

  let expanded = $state<Record<string, boolean>>({});
  let running = $state<AppIdentity[]>([]);
  let failure = $state<string | null>(null);

  let candidates = $derived(running.filter((app) => !profiles.apps.some((known) => known.bundleID === app.id)));
  let selectableTargets = $derived(targets.filter((target) => target.id !== APPLE_REWRITER_ID));
  let defaultEngineInfo = $derived(engines.find((engine) => engine.id === defaultEngine));
  let defaultRewriter = $derived(targets.find((target) => target.id === profiles.defaultRewriterID));
  let defaultStyleTitle = $derived(catalog.styles.find((style) => style.id === profiles.defaultStyle)?.title);

  function styleTitle(id: string) {
    return catalog.styles.find((style) => style.id === id)?.title ?? id;
  }

  function engineOf(id: EngineId) {
    return engines.find((engine) => engine.id === id);
  }

  function summary(profile: AppProfile) {
    const resolved = resolve(profiles, profile, defaultEngine, targets);
    const style = resolved.rewrites ? styleTitle(resolved.style) : "No rewrite";
    return `${engineOf(resolved.engine)?.shortName ?? resolved.engine} · ${style}`;
  }

  function privacySummary(profile: AppProfile) {
    const resolved = resolve(profiles, profile, defaultEngine, targets);
    const engine = engineOf(resolved.engine);
    const audio = engine?.isLocal ? `Audio stays on ${thisDevice}` : `Audio goes to ${engine?.provider}`;
    if (!resolved.rewrites) return `${audio}.`;
    const text = resolved.rewriter?.isLocal
      ? `text is rewritten on ${thisDevice}`
      : `text goes to ${resolved.rewriter?.destination ?? "the rewrite model"}`;
    return `${audio}, ${text}.`;
  }

  async function refreshRunning() {
    running = await api.runningApps().catch(() => []);
  }

  async function chooseFile(): Promise<AppIdentity | null> {
    const picked = await openDialog({
      title: "Choose an app",
      defaultPath: isMac ? "/Applications" : undefined,
      filters: [{ name: "Apps", extensions: isMac ? ["app"] : ["exe"] }],
    });
    return typeof picked === "string" ? api.appIdentityOfFile(picked) : null;
  }

  async function add(choice: string) {
    try {
      const app = choice === CHOOSE_FILE ? await chooseFile() : candidates.find((app) => app.id === choice);
      if (!app) return;
      await flush();
      onreplace(await api.addAppProfile(app.id, app.name));
      expanded[app.id] = true;
      failure = null;
    } catch (error) {
      failure = errorText(error);
    }
  }

  async function remove(profile: AppProfile) {
    try {
      await flush();
      onreplace(await api.removeAppProfile(profile.bundleID));
      failure = null;
    } catch (error) {
      failure = errorText(error);
    }
  }
</script>

<section class="section">
  <h2>Per-app profiles</h2>
  <div class="group">
    {#each profiles.apps as profile, index (profile.bundleID)}
      {@const app = profiles.apps[index]}
      {@const effectiveStyle = app.style ?? profiles.defaultStyle}
      <div class="block">
        <button
          class="plain head"
          aria-expanded={expanded[profile.bundleID] ?? false}
          onclick={() => (expanded[profile.bundleID] = !expanded[profile.bundleID])}
        >
          <span class="name">{profile.appName}</span>
          <span class="spacer"></span>
          <span class="summary">{summary(app)}</span>
          <Chevron open={expanded[profile.bundleID] ?? false} />
        </button>
        {#if expanded[profile.bundleID]}
          <div class="details">
            <label class="field-row">
              <span class="field-label wide">Transcribe with</span>
              <select bind:value={app.engine}>
                <option value={undefined}>
                  Default ({defaultEngineInfo ? engineMenuTitle(defaultEngineInfo) : defaultEngine})
                </option>
                {#each engines as engine (engine.id)}
                  <option value={engine.id}>{engineMenuTitle(engine)}</option>
                {/each}
              </select>
            </label>
            <label class="field-row">
              <span class="field-label wide">Style</span>
              <select bind:value={app.style}>
                <option value={undefined}>Default ({defaultStyleTitle})</option>
                {#each catalog.styles as style (style.id)}
                  <option value={style.id}>{style.title}</option>
                {/each}
              </select>
            </label>
            {#if app.style === "custom"}
              <textarea
                rows="3"
                placeholder="Tell the model what to do, for example: Translate to English."
                bind:value={app.customInstructions}
              ></textarea>
            {/if}
            {#if effectiveStyle !== "off"}
              <label class="field-row">
                <span class="field-label wide">Rewrite with</span>
                <select bind:value={app.rewriterID}>
                  <option value={undefined}>Default ({defaultRewriter?.displayName ?? "none yet"})</option>
                  {#each selectableTargets as target (target.id)}
                    <option value={target.id}>{target.menuTitle}</option>
                  {/each}
                </select>
              </label>
            {/if}
            <div class="field-row note">
              <span>{privacySummary(app)}</span>
              <span class="spacer"></span>
              <button class="link danger" onclick={() => remove(profile)}>Remove</button>
            </div>
          </div>
        {/if}
      </div>
    {/each}
    <div class="row">
      <select
        value=""
        aria-label="Add app"
        onfocus={refreshRunning}
        onmousedown={refreshRunning}
        onchange={(event) => {
          const choice = event.currentTarget.value;
          event.currentTarget.value = "";
          if (choice) add(choice);
        }}
      >
        <option value="" disabled>Add app…</option>
        {#if candidates.length > 0}
          <optgroup label="Open right now">
            {#each candidates as app (app.id)}
              <option value={app.id}>{app.name}</option>
            {/each}
          </optgroup>
        {/if}
        <option value={CHOOSE_FILE}>{isMac ? "Choose from Applications…" : "Choose an app…"}</option>
      </select>
    </div>
  </div>
  {#if failure}
    <p class="note error-text">{failure}</p>
  {:else}
    <p class="note">
      The app in front when you press {shortcut} picks its profile. Every other app uses the settings above.
    </p>
  {/if}
</section>

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
  .summary {
    color: var(--secondary);
    white-space: nowrap;
  }
  .details {
    padding-left: 12px;
  }
  .wide {
    width: 110px !important;
  }
</style>
