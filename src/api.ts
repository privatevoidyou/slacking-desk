import { invoke } from "@tauri-apps/api/core";

export type Platform = "windows" | "macos" | "linux";
export type Theme = "light" | "dark" | "system";
export type Language = "zh-CN" | "en";

export interface AppInfo {
  version: string;
  platform: Platform;
  dataDir: string;
  glassSupported: boolean;
}

export interface Settings {
  language: Language;
  theme: Theme;
  uiStyle: "standard" | "glass";
  wallpaper: string | null;
  wallpaperFit: "cover" | "contain";
  wallpaperBlur: number;
  wallpaperTransparency: number;
  homeSort: "fixed" | "recent";
  autostart: boolean;
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

// ---------- 壁纸 / 数据目录里的文件 ----------

export const wallpaperPick = () => invoke<string | null>("wallpaper_pick");

/** 主窗口读取数据目录文件（图标、壁纸）的地址，和 Rust 的 home::appfile_url 一致 */
export function appfileUrl(platform: Platform, rel: string) {
  return platform === "windows" ? `http://appfile.localhost/${rel}` : `appfile://localhost/${rel}`;
}

// ---------- 首页 ----------

export interface HomeIcon {
  kind: "auto" | "letter" | "image";
  color?: string | null;
  file?: string | null;
}

export interface HomeItem {
  id: string;
  type: "plugin" | "url" | "file";
  name: string;
  target: string;
  args: string;
  icon: HomeIcon;
  autoIcon: string | null;
  hideConsole: boolean;
  runAsAdmin: boolean;
  lastUsed: number;
  order: number;
  autoIconUrl: string | null;
  imageUrl: string | null;
  missing: boolean;
}

export interface HomeEdit {
  id: string;
  name: string;
  target: string;
  args: string;
  icon: HomeIcon;
  hideConsole: boolean;
  runAsAdmin: boolean;
}

export const homeList = () => invoke<HomeItem[]>("home_list");
export const homeAddUrl = (name: string, url: string) => invoke<void>("home_add_url", { name, url });
export const homeAddPaths = (paths: string[]) => invoke<void>("home_add_paths", { paths });
export const homePickFiles = () => invoke<void>("home_pick_files");
export const homeAddPlugin = (id: string) => invoke<void>("home_add_plugin", { id });
export const homeRefetchIcon = (id: string) => invoke<void>("home_refetch_icon", { id });
export const homeUpdate = (edit: HomeEdit) => invoke<void>("home_update", { edit });
export const homeRemove = (id: string) => invoke<void>("home_remove", { id });
export const homeReorder = (ids: string[]) => invoke<void>("home_reorder", { ids });
export const homeLaunch = (id: string) => invoke<void>("home_launch", { id });
export const homePickTarget = () => invoke<string | null>("home_pick_target");
export const homePickImage = () => invoke<{ file: string; url: string } | null>("home_pick_image");

/** bat / cmd / ps1 才有"隐藏控制台" */
export const isConsoleScript = (path: string) => /\.(bat|cmd|ps1)$/i.test(path);
