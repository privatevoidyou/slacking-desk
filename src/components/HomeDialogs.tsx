import { useState } from "react";
import { Check, FolderOpen } from "lucide-react";
import * as api from "../api";
import type { HomeEdit, HomeIcon, HomeItem, Platform, PluginView } from "../api";
import { useLang, useT } from "../i18n";
import { PluginIcon } from "./PluginBits";

export const COLORS = ["#2F7BD6", "#D9588E", "#2B97A8", "#D9782F", "#3F9A66", "#9761BD", "#5A68D6", "#5B616B"];

/** 没选颜色时按名字固定挑一个，同一个名字每次颜色一样 */
export function defaultColor(name: string) {
  let h = 0;
  for (const c of name) h = (h * 31 + c.codePointAt(0)!) >>> 0;
  return COLORS[h % COLORS.length];
}

export function LetterIcon({ name, color }: { name: string; color?: string | null }) {
  return (
    <div className="ico l txt" style={{ background: color || defaultColor(name) }}>
      {[...name.trim()][0]?.toUpperCase() ?? "?"}
    </div>
  );
}

/** 首页图标：自定义图片 > 首字母（选了颜色）> 自动（插件图标 / 程序图标 / 首字母） */
export function TileIcon({ item, name, plugin, preview }: { item: HomeItem; name: string; plugin?: PluginView; preview?: { icon: HomeIcon; imageUrl: string | null } }) {
  const icon = preview?.icon ?? item.icon;
  const imageUrl = preview ? preview.imageUrl : item.imageUrl;
  if (icon.kind === "image" && imageUrl) {
    return (
      <div className="ico l img">
        <img src={imageUrl} alt="" draggable={false} />
      </div>
    );
  }
  if (icon.kind === "letter") return <LetterIcon name={name} color={icon.color} />;
  if (item.type === "plugin" && plugin) return <PluginIcon url={plugin.iconUrl} name={name} size="l" />;
  if (item.autoIconUrl) {
    return (
      <div className="ico l img plain">
        <img src={item.autoIconUrl} alt="" draggable={false} />
      </div>
    );
  }
  return <LetterIcon name={name} color={icon.color} />;
}

function Field(props: { label: string; children: React.ReactNode }) {
  return (
    <div className="field">
      <div className="fl">{props.label}</div>
      {props.children}
    </div>
  );
}

export function AddUrlDialog({ onClose }: { onClose: () => void }) {
  const t = useT();
  const [url, setUrl] = useState("");
  const [name, setName] = useState("");
  const [error, setError] = useState("");
  const save = () => {
    api.homeAddUrl(name, url).then(onClose, (e) => setError(String(e)));
  };
  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <form
        className="dlg small"
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <div className="dt">{t("home.addUrl")}</div>
        <Field label={t("home.url")}>
          <input className="inp" autoFocus value={url} placeholder="bilibili.com" onChange={(e) => setUrl(e.target.value)} />
        </Field>
        <Field label={t("home.nameOptional")}>
          <input className="inp" value={name} onChange={(e) => setName(e.target.value)} />
        </Field>
        {error && <div className="err top">{error}</div>}
        <div className="foot">
          <button type="button" className="btn" onClick={onClose}>
            {t("act.cancel")}
          </button>
          <button type="submit" className="btn pri" disabled={!url.trim()}>
            {t("act.add")}
          </button>
        </div>
      </form>
    </div>
  );
}

export function AddPluginDialog({ plugins, onClose }: { plugins: PluginView[]; onClose: () => void }) {
  const t = useT();
  const lang = useLang();
  const [error, setError] = useState("");
  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="dlg small">
        <div className="dt">{t("home.addPlugin")}</div>
        <div className="plist">
          {plugins.length === 0 && <div className="sh">{t("home.noPluginToAdd")}</div>}
          {plugins.map((p) => {
            const name = api.pluginName(p.manifest, p.id, lang);
            return (
              <div className="prow" key={p.id}>
                <PluginIcon url={p.iconUrl} name={name} size="s" />
                <div className="rmain">
                  <b>{name}</b>
                  <div className="rd">{api.pluginDesc(p.manifest, lang)}</div>
                </div>
                <button className="btn sm" onClick={() => api.homeAddPlugin(p.id).catch((e) => setError(String(e)))}>
                  {t("act.add")}
                </button>
              </div>
            );
          })}
        </div>
        {error && <div className="err top">{error}</div>}
        <div className="foot">
          <button className="btn" onClick={onClose}>
            {t("act.close")}
          </button>
        </div>
      </div>
    </div>
  );
}

export function EditDialog(props: { item: HomeItem; name: string; plugin?: PluginView; platform: Platform; onClose: () => void }) {
  const t = useT();
  const { item } = props;
  const [edit, setEdit] = useState<HomeEdit>({
    id: item.id,
    name: item.type === "plugin" && !item.name ? props.name : item.name,
    target: item.target,
    args: item.args,
    icon: item.icon,
    hideConsole: item.hideConsole,
    runAsAdmin: item.runAsAdmin,
  });
  const [imageUrl, setImageUrl] = useState(item.imageUrl);
  const [error, setError] = useState("");
  const set = (patch: Partial<HomeEdit>) => setEdit((e) => ({ ...e, ...patch }));
  const setIcon = (patch: Partial<HomeIcon>) => setEdit((e) => ({ ...e, icon: { ...e.icon, ...patch } }));
  const fail = (e: unknown) => setError(String(e));

  const pickImage = () =>
    api.homePickImage().then((r) => {
      if (!r) return;
      setImageUrl(r.url);
      setIcon({ kind: "image", file: r.file });
    }, fail);
  const pickTarget = () => api.homePickTarget().then((p) => p && set({ target: p }), fail);
  const save = () => {
    // 插件名字没改就存空，这样会跟着界面语言变
    const name = item.type === "plugin" && edit.name.trim() === props.name ? "" : edit.name;
    const icon = edit.icon.kind === "image" && !edit.icon.file ? { ...edit.icon, kind: "auto" as const } : edit.icon;
    api.homeUpdate({ ...edit, name, icon }).then(props.onClose, fail);
  };
  const previewName = edit.name.trim() || props.name;
  const kinds = ["auto", "letter", "image"] as const;

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && props.onClose()}>
      <form
        className="dlg"
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <div className="dt">{t(item.type === "plugin" ? "home.editPlugin" : "home.edit")}</div>
        <Field label={t("home.icon")}>
          <div className="icoPick">
            <TileIcon item={item} name={previewName} plugin={props.plugin} preview={{ icon: edit.icon, imageUrl }} />
            <div>
              <div className="seg sm">
                {kinds.map((k) => (
                  <button
                    type="button"
                    key={k}
                    className={edit.icon.kind === k ? "on" : ""}
                    onClick={() => (k === "image" && !edit.icon.file ? pickImage() : setIcon({ kind: k }))}
                  >
                    {t(`home.icon.${k}`)}
                  </button>
                ))}
              </div>
              {edit.icon.kind === "letter" && (
                <div className="sw">
                  {COLORS.map((c) => (
                    <button
                      type="button"
                      key={c}
                      aria-label={c}
                      className={(edit.icon.color || defaultColor(previewName)) === c ? "on" : ""}
                      style={{ background: c }}
                      onClick={() => setIcon({ color: c })}
                    />
                  ))}
                </div>
              )}
              {edit.icon.kind === "image" && (
                <button type="button" className="link" onClick={pickImage}>
                  {t("home.changeImage")}
                </button>
              )}
            </div>
          </div>
        </Field>
        <Field label={t("home.name")}>
          <input className="inp" value={edit.name} onChange={(e) => set({ name: e.target.value })} />
        </Field>
        {item.type === "url" && (
          <Field label={t("home.url")}>
            <input className="inp" value={edit.target} onChange={(e) => set({ target: e.target.value })} />
          </Field>
        )}
        {item.type === "file" && (
          <>
            <Field label={t("home.location")}>
              <div className="inprow">
                <input className="inp mono left" value={edit.target} onChange={(e) => set({ target: e.target.value })} />
                <button type="button" className="btn sm" onClick={pickTarget}>
                  <FolderOpen size={15} />
                  {t("home.browse")}
                </button>
              </div>
              {item.missing && edit.target === item.target && <div className="miss-t left">{t("home.missing")}</div>}
            </Field>
            <Field label={t("home.args")}>
              <input className="inp mono left" value={edit.args} onChange={(e) => set({ args: e.target.value })} />
            </Field>
            {api.isConsoleScript(edit.target) && (
              <Check2 label={t("home.hideConsole")} on={edit.hideConsole} onChange={(v) => set({ hideConsole: v })} />
            )}
            {props.platform === "windows" && (
              <Check2 label={t("home.runAsAdmin")} on={edit.runAsAdmin} onChange={(v) => set({ runAsAdmin: v })} />
            )}
          </>
        )}
        {error && <div className="err top">{error}</div>}
        <div className="foot">
          <button type="button" className="btn" onClick={props.onClose}>
            {t("act.cancel")}
          </button>
          <button type="submit" className="btn pri">
            {t("act.save")}
          </button>
        </div>
      </form>
    </div>
  );
}

function Check2(props: { label: string; on: boolean; onChange: (v: boolean) => void }) {
  return (
    <button type="button" className="ck" role="checkbox" aria-checked={props.on} onClick={() => props.onChange(!props.on)}>
      <span className={props.on ? "box on" : "box"}>{props.on && <Check size={13} strokeWidth={3} />}</span>
      {props.label}
    </button>
  );
}
