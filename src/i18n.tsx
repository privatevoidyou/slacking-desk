import { createContext, useContext } from "react";
import type { Language } from "./api";

const zh = {
  "brand.sub": "摸鱼工作台",
  "nav.home": "首页",
  "nav.plugins": "插件",
  "nav.settings": "设置",
  "side.preview": "开发预览",
  "win.minimize": "最小化",
  "win.maximize": "最大化",
  "win.close": "关闭",
  "greet.morning": "早上好",
  "greet.noon": "中午好",
  "greet.afternoon": "下午好",
  "greet.evening": "晚上好",
  "greet.night": "夜深了",
  "home.sub": "今天也要优雅地摸鱼。",
  "home.search": "搜索插件和快捷方式",
  "home.recent": "最近使用",
  "home.fixed": "固定顺序",
  "home.add": "添加快捷方式",
  "home.emptyTitle": "首页还空着",
  "home.emptyDesc": "插件都在市场里。装上几个摸鱼利器，它们就会出现在这里。",
  "home.emptyCta": "去插件页",
  "plugins.title": "插件",
  "plugins.installed": "已安装",
  "plugins.market": "市场",
  "plugins.updates": "更新",
  "plugins.search": "搜索已安装的插件",
  "plugins.fromFile": "从文件安装",
  "plugins.empty": "还没有安装任何插件",
  "plugins.marketSoon": "插件市场即将开放",
  "settings.title": "设置",
  "settings.appearance": "外观",
  "settings.general": "通用",
  "settings.language": "界面语言",
  "settings.about": "关于",
  "settings.version": "版本",
  "settings.dataDir": "数据目录",
  "theme.light": "浅色",
  "theme.dark": "深色",
  "theme.system": "跟随系统",
  "error.save": "保存失败",
};

type Key = keyof typeof zh;

const en: Record<Key, string> = {
  "brand.sub": "Slack in style",
  "nav.home": "Home",
  "nav.plugins": "Plugins",
  "nav.settings": "Settings",
  "side.preview": "Dev preview",
  "win.minimize": "Minimize",
  "win.maximize": "Maximize",
  "win.close": "Close",
  "greet.morning": "Good morning",
  "greet.noon": "Good noon",
  "greet.afternoon": "Good afternoon",
  "greet.evening": "Good evening",
  "greet.night": "Up late",
  "home.sub": "Slack off, elegantly.",
  "home.search": "Search plugins and shortcuts",
  "home.recent": "Recent",
  "home.fixed": "Custom order",
  "home.add": "Add shortcut",
  "home.emptyTitle": "Nothing here yet",
  "home.emptyDesc": "Plugins live in the market. Install a few and they will show up here.",
  "home.emptyCta": "Go to Plugins",
  "plugins.title": "Plugins",
  "plugins.installed": "Installed",
  "plugins.market": "Market",
  "plugins.updates": "Updates",
  "plugins.search": "Search installed plugins",
  "plugins.fromFile": "Install from file",
  "plugins.empty": "No plugins installed yet",
  "plugins.marketSoon": "The market is coming soon",
  "settings.title": "Settings",
  "settings.appearance": "Appearance",
  "settings.general": "General",
  "settings.language": "Language",
  "settings.about": "About",
  "settings.version": "Version",
  "settings.dataDir": "Data folder",
  "theme.light": "Light",
  "theme.dark": "Dark",
  "theme.system": "System",
  "error.save": "Save failed",
};

const dicts: Record<Language, Record<Key, string>> = { "zh-CN": zh, en };

export const LangContext = createContext<Language>("zh-CN");

export function useT() {
  const lang = useContext(LangContext);
  return (key: Key) => dicts[lang][key];
}
