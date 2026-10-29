import { useState } from "react";
import { TriangleAlert } from "lucide-react";
import { pluginDesc, pluginName, type Inspection } from "../api";
import { useLang, useT } from "../i18n";
import { PermRows, PluginIcon } from "./PluginBits";

const RUNTIME_NAME = { python: "Python", node: "Node.js", binary: "exe / app" } as const;

/** 安装（加载）确认框：显示本地程序警告、权限清单和来源。清单有错误时只显示错误。 */
export default function InstallDialog(props: { inspection: Inspection; onCancel: () => void; onConfirm: () => Promise<void> }) {
  const t = useT();
  const lang = useLang();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const { manifest: m, errors, warnings, path } = props.inspection;

  if (!m) {
    return (
      <div className="overlay">
        <div className="dlg">
          <div className="dt">{t("install.invalidTitle")}</div>
          <div className="mono left sub">{path}</div>
          <ul className="errlist">
            {errors.map((e) => (
              <li key={e}>{e}</li>
            ))}
          </ul>
          <div className="foot">
            <button className="btn" onClick={props.onCancel}>
              {t("act.close")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  const name = pluginName(m, m.id, lang);
  const confirm = async () => {
    setBusy(true);
    setError("");
    try {
      await props.onConfirm();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  return (
    <div className="overlay">
      <div className="dlg">
        <div className="dh">
          <PluginIcon url={null} name={name} size="m" />
          <div>
            <div className="dt">{t("install.title", { name })}</div>
            <div className="dm">{t("install.meta", { version: m.version, author: m.author || "—" })}</div>
          </div>
        </div>
        {pluginDesc(m, lang) && <p className="dd">{pluginDesc(m, lang)}</p>}
        {props.inspection.replaces && <div className="note">{t("install.replaces")}</div>}
        {m.backend && (
          <div className="callout">
            <TriangleAlert size={20} />
            <div>
              <b>{t("install.backendTitle", { runtime: RUNTIME_NAME[m.backend.runtime] ?? m.backend.runtime })}</b>
              <p>{t("install.backendDesc")}</p>
            </div>
          </div>
        )}
        <div className="sec">
          <div className="secT">{t("install.permissions")}</div>
          <PermRows permissions={m.permissions} background={!!m.background} />
        </div>
        <div className="sec tight">
          <div className="kv">
            <span className="k">{t("install.source")}</span>
            <span>{t("install.sourceDev")}</span>
          </div>
          {m.backend && (
            <div className="kv">
              <span className="k">{t("install.runtime")}</span>
              <span>
                {m.backend.runtime === "python"
                  ? t("install.runtimePython")
                  : m.backend.runtime === "node"
                    ? t("install.runtimeNode")
                    : t("install.runtimeBinary")}
              </span>
            </div>
          )}
        </div>
        {warnings.length > 0 && (
          <ul className="warnlist">
            {warnings.map((w) => (
              <li key={w}>{w}</li>
            ))}
          </ul>
        )}
        {error && <div className="err">{error}</div>}
        <div className="foot">
          <button className="btn" onClick={props.onCancel} disabled={busy}>
            {t("act.cancel")}
          </button>
          <button className="btn pri" onClick={confirm} disabled={busy}>
            {t("install.confirm")}
          </button>
        </div>
      </div>
    </div>
  );
}
