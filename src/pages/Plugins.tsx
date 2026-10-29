import { useState } from "react";
import { FileText, FolderOpen, PackageOpen, RotateCw, Search, Wrench } from "lucide-react";
import * as api from "../api";
import type { Inspection, PluginView } from "../api";
import { useLang, useT } from "../i18n";
import InstallDialog from "../components/InstallDialog";
import { PermRows, PluginChips, PluginIcon } from "../components/PluginBits";

type Tab = "installed" | "market" | "updates";

export default function Plugins({ plugins }: { plugins: PluginView[] }) {
  const t = useT();
  const lang = useLang();
  const [tab, setTab] = useState<Tab>("installed");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string>();
  const [inspection, setInspection] = useState<Inspection | null>(null);
  const [removing, setRemoving] = useState<PluginView | null>(null);
  const [error, setError] = useState("");

  /** 执行一个操作，出错时在页面顶部显示 */
  const run = async (fn: () => Promise<unknown>) => {
    setError("");
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    }
  };

  const pick = () =>
    run(async () => {
      const r = await api.pluginPickFolder();
      if (r) setInspection(r);
    });

  const reload = (id: string) =>
    run(async () => {
      const r = await api.pluginReload(id);
      if (r) setInspection(r);
    });

  const q = query.trim().toLowerCase();
  const shown = plugins.filter((p) => !q || api.pluginName(p.manifest, p.id, lang).toLowerCase().includes(q) || p.id.includes(q));
  const current = plugins.find((p) => p.id === selected) ?? shown[0];
  const tabs: Tab[] = ["installed", "market", "updates"];

  return (
    <>
      <header className="hd">
        <h1>{t("plugins.title")}</h1>
        <div className="tools">
          <label className="search">
            <Search size={17} />
            <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("plugins.search")} />
          </label>
          <button className="btn" onClick={pick} title={t("plugins.loadFolderHint")}>
            <FolderOpen size={17} />
            {t("plugins.loadFolder")}
          </button>
        </div>
      </header>
      <div className="tabs">
        {tabs.map((id) => (
          <button key={id} className={tab === id ? "tab on" : "tab"} onClick={() => setTab(id)}>
            {t(`plugins.${id}`)}
            {id === "installed" && plugins.length > 0 && <span className="cnt">{plugins.length}</span>}
          </button>
        ))}
      </div>
      {error && <div className="err top">{error}</div>}

      {tab !== "installed" ? (
        <div className="empty">
          <div className="empty-ico">
            <PackageOpen size={28} />
          </div>
          <h2>{t("plugins.marketSoon")}</h2>
        </div>
      ) : plugins.length === 0 ? (
        <div className="empty">
          <div className="empty-ico">
            <PackageOpen size={28} />
          </div>
          <h2>{t("plugins.empty")}</h2>
          <p>{t("plugins.emptyDesc")}</p>
          <button className="btn" onClick={pick}>
            <FolderOpen size={17} />
            {t("plugins.loadFolder")}
          </button>
        </div>
      ) : (
        <div className="cols">
          <div className="list">
            {shown.length === 0 && <div className="sh pad">{t("plugins.noMatch")}</div>}
            {shown.map((p) => {
              const name = api.pluginName(p.manifest, p.id, lang);
              const stuck = p.problem?.kind === "crashed" || p.problem?.kind === "unresponsive";
              const blocked = p.problem?.kind === "invalid" || p.problem?.kind === "needsConfirm";
              return (
                <div key={p.id} className={current?.id === p.id ? "row sel" : "row"} onClick={() => setSelected(p.id)}>
                  <PluginIcon url={p.iconUrl} name={name} size="s" />
                  <div className="rmain">
                    <div className="rt">
                      <b>{name}</b>
                      <span className="ver">{p.manifest?.version}</span>
                      <PluginChips p={p} />
                    </div>
                    <div className="rd">{p.problem ? p.problem.message : api.pluginDesc(p.manifest, lang)}</div>
                  </div>
                  {stuck ? (
                    <button className="btn sm" onClick={(e) => (e.stopPropagation(), run(() => api.pluginRestart(p.id)))}>
                      <RotateCw size={15} />
                      {t("act.restart")}
                    </button>
                  ) : blocked ? (
                    <button className="btn sm" onClick={(e) => (e.stopPropagation(), reload(p.id))}>
                      <RotateCw size={15} />
                      {t("act.reload")}
                    </button>
                  ) : (
                    <button
                      className={p.enabled ? "tg on" : "tg"}
                      aria-label={p.enabled ? t("act.disable") : t("act.enable")}
                      onClick={(e) => (e.stopPropagation(), run(() => api.pluginSetEnabled(p.id, !p.enabled)))}
                    />
                  )}
                </div>
              );
            })}
          </div>
          {current && <Detail p={current} run={run} onReload={reload} onRemove={setRemoving} />}
        </div>
      )}

      {inspection && (
        <InstallDialog
          inspection={inspection}
          onCancel={() => setInspection(null)}
          onConfirm={async () => {
            await api.pluginAdd(inspection.path);
            setSelected(inspection.manifest?.id);
            setInspection(null);
          }}
        />
      )}
      {removing && <RemoveDialog p={removing} onClose={() => setRemoving(null)} />}
    </>
  );
}

function Detail(props: {
  p: PluginView;
  run: (fn: () => Promise<unknown>) => void;
  onReload: (id: string) => void;
  onRemove: (p: PluginView) => void;
}) {
  const t = useT();
  const lang = useLang();
  const { p, run } = props;
  const m = p.manifest;
  const name = api.pluginName(m, p.id, lang);
  const status = [p.uiOpen && t("status.uiOpen"), p.bgRunning && t("status.bgRunning"), p.backendRunning && t("status.backendRunning")].filter(Boolean);
  const usable = p.enabled && !(p.problem?.kind === "invalid" || p.problem?.kind === "needsConfirm");

  return (
    <div className="detail">
      <div className="dh">
        <PluginIcon url={p.iconUrl} name={name} size="m" />
        <div>
          <div className="dn">{name}</div>
          <div className="dm">
            <span>{m ? `v${m.version}` : p.id}</span>
            {m?.author && <span>{m.author}</span>}
            {status.map((s) => (
              <span key={String(s)}>
                <i className="dot" />
                {s}
              </span>
            ))}
          </div>
        </div>
      </div>
      {m && <p className="dd">{api.pluginDesc(m, lang)}</p>}
      <div className="acts">
        {m?.ui && (
          <button className="btn pri sm" disabled={!usable} onClick={() => run(() => api.pluginOpen(p.id))}>
            {t("act.open")}
          </button>
        )}
        <button className="btn sm" onClick={() => props.onReload(p.id)}>
          {t("act.reload")}
        </button>
        <button className="btn sm" onClick={() => run(() => api.pluginSetEnabled(p.id, !p.enabled))}>
          {p.enabled ? t("act.disable") : t("act.enable")}
        </button>
        <button className="btn sm danger" onClick={() => props.onRemove(p)}>
          {t("act.remove")}
        </button>
      </div>
      <div className="acts minor">
        <button className="link" onClick={() => run(() => api.pluginOpenLog(p.id))}>
          <FileText size={15} />
          {t("act.log")}
        </button>
        {(p.uiOpen || p.bgRunning) && (
          <button className="link" onClick={() => run(() => api.pluginDevtools(p.id))}>
            <Wrench size={15} />
            {t("act.devtools")}
          </button>
        )}
      </div>

      {p.problem && (
        <div className="sec">
          <div className="secT">{t("detail.problem")}</div>
          <div className="problem">
            <b>{p.problem.message}</b>
            {p.problem.detail.length > 0 && <pre>{p.problem.detail.join("\n")}</pre>}
            {(p.problem.kind === "crashed" || p.problem.kind === "unresponsive") && (
              <button className="btn sm" onClick={() => run(() => api.pluginRestart(p.id))}>
                <RotateCw size={15} />
                {t("act.restart")}
              </button>
            )}
          </div>
        </div>
      )}
      {m && (
        <div className="sec">
          <div className="secT">{t("detail.permissions")}</div>
          {m.backend && (
            <div className="callout slim">
              <span>{t("install.backendDesc")}</span>
            </div>
          )}
          <PermRows permissions={p.grants} background={p.allowBackground && !!m.background} />
        </div>
      )}
      <div className="sec">
        <div className="secT">{t("detail.location")}</div>
        <div className="mono left">{p.path}</div>
      </div>
      {p.warnings.length > 0 && (
        <div className="sec">
          <div className="secT">{t("detail.warnings")}</div>
          <ul className="warnlist">
            {p.warnings.map((w) => (
              <li key={w}>{w}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

function RemoveDialog({ p, onClose }: { p: PluginView; onClose: () => void }) {
  const t = useT();
  const lang = useLang();
  const [deleteData, setDeleteData] = useState(false);
  const [error, setError] = useState("");
  const remove = async () => {
    try {
      await api.pluginRemove(p.id, deleteData);
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <div className="overlay">
      <div className="dlg small">
        <div className="dt">{t("remove.title", { name: api.pluginName(p.manifest, p.id, lang) })}</div>
        <p className="dd sub">{t("remove.desc")}</p>
        <label className="check">
          <input type="checkbox" checked={deleteData} onChange={(e) => setDeleteData(e.target.checked)} />
          {t("remove.deleteData")}
        </label>
        {error && <div className="err">{error}</div>}
        <div className="foot">
          <button className="btn" onClick={onClose}>
            {t("act.cancel")}
          </button>
          <button className="btn pri danger-fill" onClick={remove}>
            {t("act.remove")}
          </button>
        </div>
      </div>
    </div>
  );
}
