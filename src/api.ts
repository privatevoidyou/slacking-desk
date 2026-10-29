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
