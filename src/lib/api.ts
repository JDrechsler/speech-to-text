import { invoke } from "@tauri-apps/api/core";

export type EngineId = "whisper" | "mai" | "scribe" | "assembly" | "stepfun" | "openai";
export type Phase = "idle" | "recording" | "finishing";

export interface Badge {
  isLocal: boolean;
  text: string;
}

export interface OverlayState {
  phase: Phase;
  status: string;
  contextLine: string | null;
  engineBadge: Badge;
  rewriteBadge: Badge | null;
}

export interface Settings {
  engine: EngineId;
  lastCloudEngine: EngineId | null;
  whisperModel: string | null;
  whisperLanguage: string;
  stepfunLanguage: string;
  azureEndpoint: string | null;
  microphone: string | null;
  pauseMedia: boolean;
}

export interface CredentialField {
  id: string;
  label: string;
  placeholder: string;
  isSecret: boolean;
}

export interface EngineInfo {
  id: EngineId;
  isLocal: boolean;
  displayName: string;
  provider: string;
  shortName: string;
  summary: string;
  credentialFields: CredentialField[];
  keyHelpUrl: string | null;
  privacyPolicyUrl: string | null;
}

export interface EngineStatus {
  id: EngineId;
  setupHint: string | null;
}

export interface WhisperModel {
  id: string;
  name: string;
  detail: string;
  bytes: number;
  isRecommended: boolean;
  isInstalled: boolean;
  isSelected: boolean;
  isDownloading: boolean;
}

export interface DownloadProgress {
  id: string;
  fraction: number;
  done: boolean;
  error: string | null;
}

export interface Language {
  code: string;
  name: string;
}

export interface InputDevice {
  name: string;
  isBuiltIn: boolean;
}

export interface Microphones {
  devices: InputDevice[];
  systemDefault: string | null;
}

export interface GeneralStatus {
  version: string;
  platform: string;
  dictionaryTerms: number;
  accessibilityAllowed: boolean;
  launchAtLogin: boolean;
}

export interface RecordingEntry {
  id: string;
  createdAt: string;
  duration: number;
  transcript: string;
  rewritten?: string;
  rewriteStyle?: string;
  engine: string;
}

export type RewriteStyleId = "off" | "cleanup" | "email" | "chat" | "prompt" | "custom";
export type ModelApiId = "openAI" | "anthropic";

export interface RewriteModel {
  id: string;
  name: string;
  api: ModelApiId;
  baseURL: string;
  model: string;
  keyHelpURL?: string;
}

export interface AppProfile {
  bundleID: string;
  appName: string;
  engine?: EngineId;
  style?: RewriteStyleId;
  rewriterID?: string;
  customInstructions: string;
}

export interface ProfileSettings {
  defaultStyle: RewriteStyleId;
  defaultRewriterID: string;
  defaultCustomInstructions: string;
  apps: AppProfile[];
  models: RewriteModel[];
}

export interface StyleInfo {
  id: RewriteStyleId;
  title: string;
  summary: string;
}

export interface ModelPreset {
  name: string;
  api: ModelApiId;
  baseURL: string;
  keyHelpURL: string | null;
}

export interface RewriteCatalog {
  styles: StyleInfo[];
  apis: { id: ModelApiId; title: string }[];
  cloudPresets: ModelPreset[];
  localPresets: ModelPreset[];
  otherPreset: ModelPreset;
}

export interface RewriteTargetInfo {
  id: string;
  displayName: string;
  isLocal: boolean;
  destination: string;
  badge: string;
  menuTitle: string;
  privacyStatement: string;
  isReady: boolean;
  setupHint: string | null;
  hasKey: boolean;
  keyPlaceholder: string;
}

export interface AppIdentity {
  id: string;
  name: string;
}

export const modelKeyId = (modelId: string) => `MODEL_KEY_${modelId}`;

export const STEPFUN_LANGUAGES: Language[] = [
  { code: "en", name: "English" },
  { code: "es", name: "Spanish" },
  { code: "fr", name: "French" },
  { code: "zh", name: "Chinese" },
  { code: "ja", name: "Japanese" },
  { code: "ko", name: "Korean" },
];

export const api = {
  overlayState: () => invoke<OverlayState>("overlay_state"),
  settings: () => invoke<Settings>("get_settings"),
  engines: () => invoke<EngineInfo[]>("engine_catalog"),
  engineStatus: () => invoke<EngineStatus[]>("engine_status"),
  setEngine: (engine: EngineId) => invoke<Settings>("set_engine", { engine }),
  setWhisperLanguage: (language: string) => invoke<Settings>("set_whisper_language", { language }),
  setStepfunLanguage: (language: string) => invoke<Settings>("set_stepfun_language", { language }),
  setMicrophone: (microphone: string | null) => invoke<Settings>("set_microphone", { microphone }),
  setPauseMedia: (enabled: boolean) => invoke<Settings>("set_pause_media", { enabled }),
  whisperModels: () => invoke<WhisperModel[]>("whisper_models"),
  whisperLanguages: () => invoke<Language[]>("whisper_languages"),
  selectWhisperModel: (id: string) => invoke<Settings>("select_whisper_model", { id }),
  downloadWhisperModel: (id: string) => invoke<void>("download_whisper_model", { id }),
  cancelWhisperDownload: (id: string) => invoke<void>("cancel_whisper_download", { id }),
  deleteWhisperModel: (id: string) => invoke<void>("delete_whisper_model", { id }),
  savedCredentials: () => invoke<string[]>("saved_credentials"),
  saveCredential: (id: string, value: string) => invoke<void>("save_credential", { id, value }),
  removeCredential: (id: string) => invoke<void>("remove_credential", { id }),
  inputDevices: () => invoke<Microphones>("input_devices"),
  generalStatus: () => invoke<GeneralStatus>("general_status"),
  setLaunchAtLogin: (enabled: boolean) => invoke<void>("set_launch_at_login", { enabled }),
  requestAccessibility: () => invoke<boolean>("request_accessibility"),
  openDictionary: () => invoke<void>("open_dictionary"),
  openRecordings: () => invoke<void>("open_recordings"),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  historyEntries: () => invoke<RecordingEntry[]>("history_entries"),
  deleteHistoryEntry: (id: string) => invoke<void>("delete_history_entry", { id }),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  openHistoryWindow: () => invoke<void>("open_history_window"),
  useCloud: (cloud: boolean) => invoke<Settings>("use_cloud", { cloud }),
  rewriteCatalog: () => invoke<RewriteCatalog>("rewrite_catalog"),
  profileSettings: () => invoke<ProfileSettings>("profile_settings"),
  saveProfileSettings: (settings: ProfileSettings) =>
    invoke<ProfileSettings>("save_profile_settings", { settings }),
  rewriteTargets: () => invoke<RewriteTargetInfo[]>("rewrite_targets"),
  addRewriteModel: (preset: string) => invoke<ProfileSettings>("add_rewrite_model", { preset }),
  removeRewriteModel: (id: string) => invoke<ProfileSettings>("remove_rewrite_model", { id }),
  listModelNames: (model: RewriteModel) => invoke<string[]>("list_model_names", { model }),
  runningApps: () => invoke<AppIdentity[]>("running_apps"),
  appIdentityOfFile: (path: string) => invoke<AppIdentity>("app_identity_of_file", { path }),
  addAppProfile: (bundleId: string, appName: string) =>
    invoke<ProfileSettings>("add_app_profile", { bundleId, appName }),
  removeAppProfile: (bundleId: string) => invoke<ProfileSettings>("remove_app_profile", { bundleId }),
  retranscribe: (id: string, engine: EngineId) =>
    invoke<RecordingEntry>("retranscribe", { id, engine }),
  recordingAudio: (id: string) => invoke<ArrayBuffer>("recording_audio", { id }),
  revealRecording: (id: string) => invoke<void>("reveal_recording", { id }),
};

const HISTORY_LABEL_PREFIXES: [string, EngineId][] = [
  ["whisper", "whisper"],
  ["mai", "mai"],
  ["scribe", "scribe"],
  ["assemblyai", "assembly"],
  ["stepfun", "stepfun"],
  ["openai", "openai"],
];

export function engineOfHistoryLabel(label: string): EngineId | undefined {
  return HISTORY_LABEL_PREFIXES.find(([prefix]) => label.startsWith(prefix))?.[1];
}

export const thisDevice = isMacPlatform() ? "this Mac" : "this computer";

function isMacPlatform() {
  return typeof navigator !== "undefined" && navigator.userAgent.includes("Mac");
}

export function errorText(error: unknown): string {
  return typeof error === "string" ? error : error instanceof Error ? error.message : String(error);
}

export const isMac = isMacPlatform();

export function formatBytes(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  return `${Math.round(bytes / 1e6)} MB`;
}
