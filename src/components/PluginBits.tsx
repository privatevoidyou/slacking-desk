import { Activity, AppWindow, Bell, Bot, FileText, Globe, Keyboard, Layers, MonitorUp, MousePointerClick, type LucideIcon } from "lucide-react";
import type { PluginView } from "../api";
import { useT, type TKey } from "../i18n";

/** 插件图标：清单里的 icon，没有就用名字首字 */
export function PluginIcon({ url, name, size }: { url: string | null; name: string; size: "s" | "m" | "l" }) {
  if (url) {
    return (
      <div className={`ico img ${size}`}>
        <img src={url} alt="" draggable={false} />
      </div>
    );
  }
  return <div className={`ico letter ${size}`}>{[...name][0]?.toUpperCase() ?? "?"}</div>;
}

const PERMS: Record<string, { icon: LucideIcon; risk: "low" | "mid" | "high" }> = {
  notify: { icon: Bell, risk: "low" },
  hotkey: { icon: Keyboard, risk: "low" },
  "windows.read": { icon: Layers, risk: "mid" },
  "windows.control": { icon: AppWindow, risk: "mid" },
  wallpaper: { icon: MonitorUp, risk: "mid" },
  network: { icon: Globe, risk: "mid" },
  llm: { icon: Bot, risk: "mid" },
  files: { icon: FileText, risk: "mid" },
  input: { icon: MousePointerClick, risk: "high" },
};

/** 权限清单（安装确认框和详情页共用）。background 单独一行，不算风险等级。 */
export function PermRows({ permissions, background }: { permissions: string[]; background: boolean }) {
  const t = useT();
  if (permissions.length === 0 && !background) return <div className="sh">{t("detail.noPermissions")}</div>;
  return (
    <>
      {permissions.map((p) => {
        const info = PERMS[p];
        const Icon = info?.icon ?? Globe;
        return (
          <div className="perm" key={p}>
            <span className="pi">
              <Icon size={17} />
            </span>
            <span className="pt">{t(`perm.${p}` as TKey) ?? p}</span>
            {info && <span className={`chip ${info.risk === "mid" ? "orange" : info.risk === "high" ? "red" : ""}`}>{t(`risk.${info.risk}`)}</span>}
          </div>
        );
      })}
      {background && (
        <div className="perm">
          <span className="pi">
            <Activity size={17} />
          </span>
          <span className="pt">{t("perm.background")}</span>
        </div>
      )}
    </>
  );
}

/** 列表行和详情页标题旁的小标签 */
export function PluginChips({ p }: { p: PluginView }) {
  const t = useT();
  const m = p.manifest;
  return (
    <>
      <span className="chip">{t("chip.dev")}</span>
      {m?.background && <span className="chip blue">{t("chip.background")}</span>}
      {m?.backend && <span className="chip orange">{t("chip.backend")}</span>}
      {p.problem && <span className="chip red">{t(`chip.${p.problem.kind}`)}</span>}
      {!p.enabled && <span className="chip">{t("chip.disabled")}</span>}
    </>
  );
}
