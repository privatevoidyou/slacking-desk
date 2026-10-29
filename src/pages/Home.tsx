import { useState } from "react";
import { Sparkles } from "lucide-react";
import * as api from "../api";
import type { PluginView } from "../api";
import { useLang, useT } from "../i18n";
import { PluginIcon } from "../components/PluginBits";

function greetingKey() {
  const h = new Date().getHours();
  if (h < 5) return "greet.night" as const;
  if (h < 11) return "greet.morning" as const;
  if (h < 13) return "greet.noon" as const;
  if (h < 18) return "greet.afternoon" as const;
  return "greet.evening" as const;
}

/** M1：只显示已启用且有界面的插件。搜索、排序、快捷方式在 M2。 */
export default function Home({ plugins, onGoPlugins }: { plugins: PluginView[]; onGoPlugins: () => void }) {
  const t = useT();
  const lang = useLang();
  const [error, setError] = useState("");
  const tiles = plugins.filter((p) => p.enabled && p.manifest?.ui && !["invalid", "needsConfirm"].includes(p.problem?.kind ?? ""));

  const open = (id: string) => {
    setError("");
    api.pluginOpen(id).catch((e) => setError(String(e)));
  };

  return (
    <>
      <header className="hd">
        <div>
          <h1>{t(greetingKey())}</h1>
          <p className="sub">{t("home.sub")}</p>
        </div>
      </header>
      {error && <div className="err top">{error}</div>}
      {tiles.length === 0 ? (
        <div className="empty">
          <div className="empty-ico">
            <Sparkles size={28} />
          </div>
          <h2>{t("home.emptyTitle")}</h2>
          <p>{t("home.emptyDesc")}</p>
          <button className="btn" onClick={onGoPlugins}>
            {t("home.emptyCta")}
          </button>
        </div>
      ) : (
        <div className="grid">
          {tiles.map((p) => {
            const name = api.pluginName(p.manifest, p.id, lang);
            return (
              <button key={p.id} className="tile" onClick={() => open(p.id)} title={api.pluginDesc(p.manifest, lang)}>
                <PluginIcon url={p.iconUrl} name={name} size="l" />
                <span className="lbl">{name}</span>
                <span className={p.problem ? "st bad" : "st"}>{p.problem ? t(`chip.${p.problem.kind}`) : p.bgRunning ? t("chip.background") : "\u00a0"}</span>
              </button>
            );
          })}
        </div>
      )}
    </>
  );
}
