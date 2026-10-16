import {
  thisDevice,
  type AppProfile,
  type EngineId,
  type EngineInfo,
  type ProfileSettings,
  type RewriteModel,
  type RewriteStyleId,
  type RewriteTargetInfo,
} from "$lib/api";

const LOOPBACK_HOSTS = ["localhost", "127.0.0.1", "[::1]", "::1"];

export interface ModelFacts {
  host: string | null;
  isLocal: boolean;
  isConfigured: boolean;
  destination: string;
}

export function modelFacts(model: RewriteModel): ModelFacts {
  let host: string | null = null;
  try {
    host = new URL(model.baseURL.trim()).hostname || null;
  } catch {
    host = null;
  }
  const isLocal = host !== null && LOOPBACK_HOSTS.includes(host);
  return {
    host,
    isLocal,
    isConfigured: host !== null && model.model.trim() !== "",
    destination: isLocal ? thisDevice : (host ?? "the internet"),
  };
}

export function engineMenuTitle(engine: EngineInfo): string {
  return engine.isLocal ? `${engine.displayName} · on ${thisDevice}` : `${engine.displayName} · ${engine.provider}`;
}

export function rewrites(style: RewriteStyleId, instructions: string): boolean {
  if (style === "off") return false;
  if (style === "custom") return instructions.trim() !== "";
  return true;
}

export function isRewriteInUse(settings: ProfileSettings): boolean {
  return (
    settings.defaultStyle !== "off" ||
    settings.apps.some((app) => (app.style ?? "off") !== "off") ||
    settings.models.length > 0
  );
}

export interface ResolvedProfile {
  engine: EngineId;
  style: RewriteStyleId;
  rewrites: boolean;
  rewriter: RewriteTargetInfo | undefined;
}

export function resolve(
  settings: ProfileSettings,
  profile: AppProfile,
  defaultEngine: EngineId,
  targets: RewriteTargetInfo[],
): ResolvedProfile {
  const style = profile.style ?? settings.defaultStyle;
  const instructions =
    profile.style === "custom" ? profile.customInstructions : settings.defaultCustomInstructions;
  const rewriter =
    targets.find((target) => target.id === profile.rewriterID) ??
    targets.find((target) => target.id === settings.defaultRewriterID);
  return {
    engine: profile.engine ?? defaultEngine,
    style,
    rewrites: rewrites(style, instructions),
    rewriter,
  };
}
