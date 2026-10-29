import { useEffect, useState } from "react";
import { appInfo, getSettings, saveSettings, type AppInfo, type Settings } from "./api";
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
  const theme = useEffectiveTheme(settings?.theme);

  useEffect(() => {
    appInfo().then(setInfo);
    getSettings().then(setSettings);
  }, []);

  useEffect(() => {
    document.documentElement.className = `${theme} ${info?.platform ?? ""}`;
  }, [theme, info]);

  if (!info || !settings) return null;

  const updateSettings = async (next: Settings) => {
    await saveSettings(next);
    setSettings(next);
  };

  return (
    <LangContext.Provider value={settings.language}>
      <div className="win">
        <Sidebar page={page} onChange={setPage} platform={info.platform} />
        <div className="main">
          <TitleBar platform={info.platform} />
          <section className="content">
            {page === "home" && <Home onGoPlugins={() => setPage("plugins")} />}
            {page === "plugins" && <Plugins />}
            {page === "settings" && <SettingsPage info={info} settings={settings} onChange={updateSettings} />}
          </section>
        </div>
      </div>
    </LangContext.Provider>
  );
}
