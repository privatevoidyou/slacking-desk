import { Fish, House, Puzzle, Settings } from "lucide-react";
import type { Platform } from "../api";
import { useT } from "../i18n";

export type Page = "home" | "plugins" | "settings";

export default function Sidebar(props: { page: Page; onChange: (p: Page) => void; platform: Platform; version: string }) {
  const t = useT();
  const items = [
    { id: "home" as const, icon: House, label: t("nav.home") },
    { id: "plugins" as const, icon: Puzzle, label: t("nav.plugins") },
    { id: "settings" as const, icon: Settings, label: t("nav.settings") },
  ];
  return (
    <aside className="side">
      <div className={props.platform === "macos" ? "drag mac" : "drag"} data-tauri-drag-region />
      <div className="brand" data-tauri-drag-region>
        <div className="logo" data-tauri-drag-region>
          <Fish size={22} />
        </div>
        <div data-tauri-drag-region>
          <div className="b1" data-tauri-drag-region>Slacking Desk</div>
          <div className="b2" data-tauri-drag-region>{t("brand.sub")}</div>
        </div>
      </div>
      {items.map(({ id, icon: Icon, label }) => (
        <button key={id} className={props.page === id ? "nav on" : "nav"} onClick={() => props.onChange(id)}>
          <Icon size={19} />
          {label}
        </button>
      ))}
      <div className="side-foot">v{props.version} · {t("side.preview")}</div>
    </aside>
  );
}
