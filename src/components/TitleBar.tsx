import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import type { Platform } from "../api";
import { useT } from "../i18n";

/** Windows 上自绘标题栏按钮；macOS 用系统红绿灯，这里只留拖动区域。 */
export default function TitleBar({ platform }: { platform: Platform }) {
  const t = useT();
  const win = getCurrentWindow();
  return (
    <div className="tbar" data-tauri-drag-region>
      {platform === "windows" && (
        <>
          <button className="cap" aria-label={t("win.minimize")} onClick={() => win.minimize()}>
            <Minus size={16} />
          </button>
          <button className="cap" aria-label={t("win.maximize")} onClick={() => win.toggleMaximize()}>
            <Square size={13} />
          </button>
          <button className="cap close" aria-label={t("win.close")} onClick={() => win.hide()}>
            <X size={17} />
          </button>
        </>
      )}
    </div>
  );
}
