import { invoke } from "@tauri-apps/api/core";

export type Platform = "windows" | "macos" | "linux";
export type Theme = "light" | "dark" | "system";
export type Language = "zh-CN" | "en";

export interface AppInfo {
  version: string;
  platform: Platform;
  dataDir: string;
}

export interface Settings {
  language: Language;
  theme: Theme;
}

export const appInfo = () => invoke<AppInfo>("app_info");
export const getSettings = () => invoke<Settings>("get_settings");
export const saveSettings = (settings: Settings) => invoke<void>("save_settings", { settings });

// ---------- 插件 ----------

export interface Manifest {
  manifestVersion: number;
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  icon?: string | null;
  minShellVersion: string;
  ui?: { page: string; width?: number | null; height?: number | null } | null;
  background?: string | null;
  backend?: { runtime: "python" | "node" | "binary"; entry: unknown } | null;
  permissions: string[];
  i18n?: Record<string, { name?: string | null; description?: string | null }>;
}

export interface Problem {
  kind: "invalid" | "needsConfirm" | "crashed" | "unresponsive";
  message: string;
  detail: string[];
}

export interface PluginView {
  id: string;
  path: string;
  enabled: boolean;
  grants: string[];
  allowBackground: boolean;
  allowBackend: boolean;
  manifest: Manifest | null;
  warnings: string[];
  iconUrl: string | null;
  problem: Problem | null;
  uiOpen: boolean;
  bgRunning: boolean;
  backendRunning: boolean;
}

/** 安装确认框的内容 */
export interface Inspection {
  path: string;
  manifest: Manifest | null;
  errors: string[];
  warnings: string[];
  replaces: boolean;
}

export const openDataDir = () => invoke<void>("open_data_dir");
export const pluginsList = () => invoke<PluginView[]>("plugins_list");
export const pluginPickFolder = () => invoke<Inspection | null>("plugin_pick_folder");
export const pluginAdd = (path: string) => invoke<void>("plugin_add", { path });
export const pluginSetEnabled = (id: string, enabled: boolean) => invoke<void>("plugin_set_enabled", { id, enabled });
export const pluginReload = (id: string) => invoke<Inspection | null>("plugin_reload", { id });
export const pluginRestart = (id: string) => invoke<void>("plugin_restart", { id });
export const pluginRemove = (id: string, deleteData: boolean) => invoke<void>("plugin_remove", { id, deleteData });
export const pluginOpen = (id: string) => invoke<void>("plugin_open", { id });
export const pluginOpenLog = (id: string) => invoke<void>("plugin_open_log", { id });
export const pluginDevtools = (id: string) => invoke<void>("plugin_devtools", { id });

/** 插件显示名：英文界面优先用清单里的 i18n.en */
export function pluginName(m: Manifest | null, id: string, lang: Language) {
  if (!m) return id;
  return (lang === "en" && m.i18n?.en?.name) || m.name;
}

export function pluginDesc(m: Manifest | null, lang: Language) {
  if (!m) return "";
  return (lang === "en" && m.i18n?.en?.description) || m.description;
}
