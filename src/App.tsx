import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { appfileUrl, appInfo, getSettings, pluginsList, saveSettings, type AppInfo, type PluginView, type Settings } from "./api";
import { LangContext } from "./i18n";
import TitleBar from "./components/TitleBar";
import Sidebar, { type Page } from "./components/Sidebar";
import Home from "./pages/Home";
import Plugins from "./pages/Plugins";
import SettingsPage from "./pages/Settings";

function useEffectiveTheme(theme: Settings["theme"] | undefined) {
  const [systemDark, setSystemDark] = useState(() => matchMedia("(prefers-color-scheme: dark)").matches);
  useEffect(() => {
    const mq = matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setSystemDark(mq.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);
  if (theme === "light" || theme === "dark") return theme;
  return systemDark ? "dark" : "light";
}

export default function App() {
  const [info, setInfo] = useState<AppInfo>();
  const [settings, setSettings] = useState<Settings>();
  const [page, setPage] = useState<Page>("home");
  const [plugins, setPlugins] = useState<PluginView[]>([]);
  const theme = useEffectiveTheme(settings?.theme);

  useEffect(() => {
    appInfo().then(setInfo);
    getSettings().then(setSettings);
    // 插件状态（启停、崩溃、窗口开关）变化时由 Rust 通知
    const refresh = () => pluginsList().then(setPlugins);
    refresh();
    const off = listen("plugins-changed", refresh);
    return () => void off.then((f) => f());
  }, []);

  // 界面风格：wp = 有壁纸；fx = 毛玻璃且没壁纸，用系统透明效果（不支持时就是普通纯色）
  const glass = settings?.uiStyle === "glass";
  const wp = !!settings?.wallpaper;
  const fx = glass && !wp && !!info?.glassSupported;
  useEffect(() => {
    document.documentElement.className = [theme, info?.platform ?? "", glass ? "glass" : "", wp ? "wp" : "", fx ? "fx" : ""].join(" ");
  }, [theme, info, glass, wp, fx]);

  if (!info || !settings) return null;

  const updateSettings = async (next: Settings) => {
    await saveSettings(next);
    setSettings(next);
  };

  const blur = settings.wallpaperBlur;

  return (
    <LangContext.Provider value={settings.language}>
      {settings.wallpaper && (
        <>
          <div
            className="wallpaper"
            style={{
              backgroundImage: `url("${appfileUrl(info.platform, `wallpaper/${settings.wallpaper}`)}")`,
              backgroundSize: settings.wallpaperFit,
              filter: blur ? `blur(${blur}px)` : undefined,
              // 透明度：透出下面的纯色底
              opacity: 1 - settings.wallpaperTransparency / 100,
              // 模糊后边缘会透出底色，稍微放大盖住
              transform: blur ? `scale(${1 + blur / 200})` : undefined,
            }}
          />
        </>
      )}
      <div className="win">
        <Sidebar page={page} onChange={setPage} platform={info.platform} version={info.version} />
        <div className="main">
          <TitleBar platform={info.platform} />
          <section className="content">
            {page === "home" && <Home plugins={plugins} platform={info.platform} settings={settings} onSettings={updateSettings} />}
            {page === "plugins" && <Plugins plugins={plugins} />}
            {page === "settings" && <SettingsPage info={info} settings={settings} onChange={updateSettings} onPreview={setSettings} />}
          </section>
        </div>
      </div>
    </LangContext.Provider>
  );
}
