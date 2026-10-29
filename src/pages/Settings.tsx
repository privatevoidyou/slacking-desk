import { useState } from "react";
import { ChevronDown, Image, Monitor, Moon, Sparkles, Square, Sun } from "lucide-react";
import { appfileUrl, openDataDir, wallpaperPick, type AppInfo, type Language, type Settings, type Theme } from "../api";
import { useT } from "../i18n";

const themes: { id: Theme; icon: typeof Sun }[] = [
  { id: "light", icon: Sun },
  { id: "dark", icon: Moon },
  { id: "system", icon: Monitor },
];

const styles: { id: Settings["uiStyle"]; icon: typeof Sun }[] = [
  { id: "standard", icon: Square },
  { id: "glass", icon: Sparkles },
];

export default function SettingsPage(props: {
  info: AppInfo;
  settings: Settings;
  onChange: (s: Settings) => Promise<void>;
  /** 拖滑块时只预览，不保存 */
  onPreview: (s: Settings) => void;
}) {
  const t = useT();
  const s = props.settings;
  const [error, setError] = useState("");
  const update = (patch: Partial<Settings>) => {
    setError("");
    props.onChange({ ...s, ...patch }).catch((e) => setError(`${t("error.save")}：${e}`));
  };
  const pick = () =>
    wallpaperPick().then(
      (name) => name && update({ wallpaper: name }),
      (e) => setError(String(e)),
    );
  const slider = (key: "wallpaperBlur" | "wallpaperTransparency", max: number, unit: string) => (
    <div className="slider">
      <input
        type="range"
        min={0}
        max={max}
        value={s[key]}
        onChange={(e) => props.onPreview({ ...s, [key]: Number(e.target.value) })}
        onPointerUp={() => update({})}
        onKeyUp={() => update({})}
      />
      <span className="sv">
        {s[key]}
        {unit}
      </span>
    </div>
  );

  return (
    <>
      <header className="hd">
        <h1>{t("settings.title")}</h1>
      </header>
      <div className="sbody">
        {error && <div className="err">{error}</div>}
        <div className="grp">
          <div className="gT">{t("settings.appearance")}</div>
          <div className="srow top">
            <div className="sl">{t("settings.theme")}</div>
            <div className="themes">
              {themes.map(({ id, icon: Icon }) => (
                <button key={id} className={s.theme === id ? "th on" : "th"} onClick={() => update({ theme: id })}>
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
          <div className="srow top">
            <div>
              <div className="sl">{t("settings.uiStyle")}</div>
              <div className="sh">{t(props.info.glassSupported ? "settings.glassHint" : "settings.glassHintSolid")}</div>
            </div>
            <div className="themes">
              {styles.map(({ id, icon: Icon }) => (
                <button key={id} className={s.uiStyle === id ? "th on" : "th"} onClick={() => update({ uiStyle: id })}>
                  <div className={`pv pv-${id === "glass" ? "glass" : "light"}`}>
                    <i />
                    <i />
                  </div>
                  <div className="thl">
                    <Icon size={15} />
                    {t(`style.${id}`)}
                  </div>
                </button>
              ))}
            </div>
          </div>
          <div className="srow">
            <div className="wprow">
              <div className="wpthumb" style={s.wallpaper ? { backgroundImage: `url("${appfileUrl(props.info.platform, `wallpaper/${s.wallpaper}`)}")` } : undefined}>
                {!s.wallpaper && <Image size={22} />}
              </div>
              <div>
                <div className="sl">{t("settings.wallpaper")}</div>
                <div className="sh">{s.wallpaper ? t("settings.wallpaperFormats") : t("settings.wallpaperNone")}</div>
                <div className="btnrow">
                  <button className="btn sm" onClick={pick}>
                    <Image size={15} />
                    {t(s.wallpaper ? "settings.wallpaperChange" : "settings.wallpaperPick")}
                  </button>
                  {s.wallpaper && (
                    <button className="btn sm" onClick={() => update({ wallpaper: null })}>
                      {t("act.remove")}
                    </button>
                  )}
                </div>
              </div>
            </div>
          </div>
          {s.wallpaper && (
            <>
              <div className="srow">
                <div className="sl">{t("settings.wallpaperFit")}</div>
                <div className="seg sm">
                  {(["cover", "contain"] as const).map((f) => (
                    <button key={f} className={s.wallpaperFit === f ? "on" : ""} onClick={() => update({ wallpaperFit: f })}>
                      {t(`fit.${f}`)}
                    </button>
                  ))}
                </div>
              </div>
              <div className="srow">
                <div className="sl">{t("settings.blur")}</div>
                {slider("wallpaperBlur", 40, "")}
              </div>
              <div className="srow">
                <div className="sl">{t("settings.transparency")}</div>
                {slider("wallpaperTransparency", 90, "%")}
              </div>
            </>
          )}
        </div>
        <div className="grp">
          <div className="gT">{t("settings.general")}</div>
          <div className="srow">
            <div className="sl">{t("settings.language")}</div>
            <label className="sel2">
              <select value={s.language} onChange={(e) => update({ language: e.target.value as Language })}>
                <option value="zh-CN">简体中文</option>
                <option value="en">English</option>
              </select>
              <ChevronDown size={16} />
            </label>
          </div>
          <div className="srow">
            <div>
              <div className="sl">{t("settings.autostart")}</div>
              <div className="sh">{t("settings.autostartHint")}</div>
            </div>
            <button
              className={s.autostart ? "tg on" : "tg"}
              role="switch"
              aria-checked={s.autostart}
              aria-label={t("settings.autostart")}
              onClick={() => update({ autostart: !s.autostart })}
            />
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
            <div className="dirrow">
              <div className="sh mono">{props.info.dataDir}</div>
              <button className="btn sm" onClick={() => openDataDir().catch((e) => setError(String(e)))}>
                {t("settings.openDataDir")}
              </button>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
