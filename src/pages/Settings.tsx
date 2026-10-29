import { useState } from "react";
import { ChevronDown, Monitor, Moon, Sun } from "lucide-react";
import type { AppInfo, Language, Settings, Theme } from "../api";
import { useT } from "../i18n";

const themes: { id: Theme; icon: typeof Sun }[] = [
  { id: "light", icon: Sun },
  { id: "dark", icon: Moon },
  { id: "system", icon: Monitor },
];

export default function SettingsPage(props: { info: AppInfo; settings: Settings; onChange: (s: Settings) => Promise<void> }) {
  const t = useT();
  const [error, setError] = useState("");
  const update = (patch: Partial<Settings>) => {
    setError("");
    props.onChange({ ...props.settings, ...patch }).catch((e) => setError(`${t("error.save")}：${e}`));
  };
  return (
    <>
      <header className="hd">
        <h1>{t("settings.title")}</h1>
      </header>
      <div className="sbody">
        {error && <div className="err">{error}</div>}
        <div className="grp">
          <div className="gT">{t("settings.appearance")}</div>
          <div className="themes">
            {themes.map(({ id, icon: Icon }) => (
              <button key={id} className={props.settings.theme === id ? "th on" : "th"} onClick={() => update({ theme: id })}>
                <div className={`pv pv-${id}`}>
                  <i />
                  <i />
                </div>
                <div className="thl">
                  <Icon size={15} />
                  {t(`theme.${id}`)}
                </div>
              </button>
            ))}
          </div>
        </div>
        <div className="grp">
          <div className="gT">{t("settings.general")}</div>
          <div className="srow">
            <div className="sl">{t("settings.language")}</div>
            <label className="sel2">
              <select value={props.settings.language} onChange={(e) => update({ language: e.target.value as Language })}>
                <option value="zh-CN">简体中文</option>
                <option value="en">English</option>
              </select>
              <ChevronDown size={16} />
            </label>
          </div>
        </div>
        <div className="grp">
          <div className="gT">{t("settings.about")}</div>
          <div className="srow">
            <div className="sl">{t("settings.version")}</div>
            <div className="sh">{props.info.version}</div>
          </div>
          <div className="srow">
            <div className="sl">{t("settings.dataDir")}</div>
            <div className="sh mono">{props.info.dataDir}</div>
          </div>
        </div>
      </div>
    </>
  );
}
