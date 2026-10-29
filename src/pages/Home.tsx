import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Clock, FolderOpen, Globe, Pencil, Pin, Plus, Puzzle, SquareArrowOutUpRight, X } from "lucide-react";
import * as api from "../api";
import type { HomeItem, Platform, PluginView, Settings } from "../api";
import { useLang, useT } from "../i18n";
import Menu, { type MenuItem } from "../components/Menu";
import { AddPluginDialog, AddUrlDialog, EditDialog, TileIcon } from "../components/HomeDialogs";

function greetingKey() {
  const h = new Date().getHours();
  if (h < 5) return "greet.night" as const;
  if (h < 11) return "greet.morning" as const;
  if (h < 13) return "greet.noon" as const;
  if (h < 18) return "greet.afternoon" as const;
  return "greet.evening" as const;
}

type Dialog = { kind: "url" } | { kind: "plugin" } | { kind: "edit"; item: HomeItem } | null;
type MenuState = { x: number; y: number; items: (MenuItem | "sep")[] } | null;
/** 正在拖动的图标：鼠标相对图标左上角的偏移，和当前位置 */
type Drag = { id: string; dx: number; dy: number; x: number; y: number } | null;

export default function Home(props: {
  plugins: PluginView[];
  platform: Platform;
  settings: Settings;
  onSettings: (s: Settings) => Promise<void>;
}) {
  const t = useT();
  const lang = useLang();
  const [items, setItems] = useState<HomeItem[]>([]);
  const [error, setError] = useState("");
  const [menu, setMenu] = useState<MenuState>(null);
  const [dialog, setDialog] = useState<Dialog>(null);
  const [drag, setDrag] = useState<Drag>(null);
  const [dropping, setDropping] = useState(false);
  const gridRef = useRef<HTMLDivElement>(null);
  const press = useRef<{ id: string; x: number; y: number; dx: number; dy: number } | null>(null);
  const justDragged = useRef(false);
  const itemsRef = useRef(items);
  itemsRef.current = items;
  const sort = props.settings.homeSort;

  useEffect(() => {
    const refresh = () => api.homeList().then(setItems, (e) => setError(String(e)));
    refresh();
    const off = listen("home-changed", refresh);
    return () => void off.then((f) => f());
  }, []);

  // 把文件（可以多个）拖进窗口就添加到首页
  useEffect(() => {
    const off = getCurrentWebview().onDragDropEvent((e) => {
      if (e.payload.type === "enter" || e.payload.type === "over") setDropping(true);
      else if (e.payload.type === "leave") setDropping(false);
      else if (e.payload.type === "drop") {
        setDropping(false);
        if (e.payload.paths.length) api.homeAddPaths(e.payload.paths).catch((err) => setError(String(err)));
      }
    });
    return () => void off.then((f) => f());
  }, []);

  const pluginOf = (it: HomeItem) => (it.type === "plugin" ? props.plugins.find((p) => p.id === it.target) : undefined);
  const nameOf = (it: HomeItem) => {
    if (it.name) return it.name;
    const p = pluginOf(it);
    return p ? api.pluginName(p.manifest, p.id, lang) : it.target;
  };

  const shown = useMemo(() => {
    if (sort !== "recent") return items;
    return [...items].sort((a, b) => b.lastUsed - a.lastUsed || a.order - b.order);
  }, [items, sort]);

  const addable = props.plugins.filter(
    (p) => p.manifest?.ui && !items.some((i) => i.type === "plugin" && i.target === p.id),
  );

  const launch = (it: HomeItem) => {
    setError("");
    if (it.missing) return setDialog({ kind: "edit", item: it });
    api.homeLaunch(it.id).catch((e) => setError(String(e)));
  };

  const tileMenu = (e: React.MouseEvent, it: HomeItem) => {
    e.preventDefault();
    setMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: t("act.open"), icon: <SquareArrowOutUpRight size={16} />, onClick: () => launch(it) },
        { label: t("home.editMenu"), icon: <Pencil size={16} />, onClick: () => setDialog({ kind: "edit", item: it }) },
        "sep",
        { label: t("home.remove"), icon: <X size={16} />, danger: true, onClick: () => api.homeRemove(it.id).catch((err) => setError(String(err))) },
      ],
    });
  };

  const addMenu = (e: React.MouseEvent) => {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setMenu({
      x: r.left + r.width / 2,
      y: r.top + 48,
      items: [
        { label: t("home.addUrl"), icon: <Globe size={16} />, onClick: () => setDialog({ kind: "url" }) },
        { label: t("home.addFile"), icon: <FolderOpen size={16} />, onClick: () => api.homePickFiles().catch((err) => setError(String(err))) },
        {
          label: t("home.addPlugin"),
          icon: <Puzzle size={16} />,
          hint: addable.length ? t("home.addableCount", { n: addable.length }) : undefined,
          onClick: () => setDialog({ kind: "plugin" }),
        },
      ],
    });
  };

  // ---------- 固定顺序下拖动排序（用鼠标事件，系统拖放留给"拖文件进来"） ----------
  const onPointerDown = (e: React.PointerEvent, it: HomeItem) => {
    if (e.button !== 0 || sort !== "fixed") return;
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    press.current = { id: it.id, x: e.clientX, y: e.clientY, dx: e.clientX - r.left, dy: e.clientY - r.top };
  };

  useEffect(() => {
    const move = (e: PointerEvent) => {
      const p = press.current;
      if (!p) return;
      if (!drag && Math.hypot(e.clientX - p.x, e.clientY - p.y) < 6) return;
      setDrag({ id: p.id, dx: p.dx, dy: p.dy, x: e.clientX, y: e.clientY });
      // 鼠标停在哪个图标上，就把拖动的图标挪到那个位置
      const tiles = gridRef.current?.querySelectorAll<HTMLElement>("[data-tile]");
      tiles?.forEach((el) => {
        const r = el.getBoundingClientRect();
        const over = e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom;
        const overId = el.dataset.tile;
        if (over && overId && overId !== p.id) {
          setItems((list) => {
            const from = list.findIndex((x) => x.id === p.id);
            const to = list.findIndex((x) => x.id === overId);
            if (from < 0 || to < 0) return list;
            const next = [...list];
            next.splice(to, 0, next.splice(from, 1)[0]);
            return next;
          });
        }
      });
    };
    const up = () => {
      if (press.current && drag) {
        justDragged.current = true;
        setTimeout(() => (justDragged.current = false), 0);
        api.homeReorder(itemsRef.current.map((x) => x.id)).catch((err) => setError(String(err)));
      }
      press.current = null;
      setDrag(null);
    };
    addEventListener("pointermove", move);
    addEventListener("pointerup", up);
    return () => {
      removeEventListener("pointermove", move);
      removeEventListener("pointerup", up);
    };
  }, [drag]);

  const dragged = drag && items.find((i) => i.id === drag.id);

  const tile = (it: HomeItem, ghost = false) => {
    const p = pluginOf(it);
    const name = nameOf(it);
    const crashed = p?.problem && ["crashed", "unresponsive"].includes(p.problem.kind);
    const unusable = it.type === "plugin" && (!p || !p.enabled || ["invalid", "needsConfirm"].includes(p.problem?.kind ?? ""));
    const cls = ["tile", it.missing || unusable ? "miss" : "", drag?.id === it.id && !ghost ? "placeholder" : "", ghost ? "ghost" : ""].join(" ");
    const tip = it.missing ? t("home.missing") : crashed ? t(`chip.${p!.problem!.kind}`) : unusable ? t("home.pluginUnavailable") : it.type === "plugin" ? api.pluginDesc(p?.manifest ?? null, lang) : it.target;
    return (
      <button
        key={it.id}
        data-tile={ghost ? undefined : it.id}
        className={cls}
        title={tip}
        onClick={() => !justDragged.current && launch(it)}
        onContextMenu={(e) => tileMenu(e, it)}
        onPointerDown={(e) => onPointerDown(e, it)}
        style={ghost && drag ? { left: drag.x - drag.dx, top: drag.y - drag.dy } : undefined}
      >
        <div className="ico-wrap">
          <TileIcon item={it} name={name} plugin={p} />
          {crashed && <span className="reddot" />}
        </div>
        <span className="lbl">{name}</span>
        {it.missing && <span className="miss-t">{t("home.missing")}</span>}
      </button>
    );
  };

  const editItem = dialog?.kind === "edit" ? items.find((i) => i.id === dialog.item.id) ?? dialog.item : null;

  return (
    <div className={dropping ? "home dropping" : "home"}>
      <header className="hd">
        <div>
          <h1>{t(greetingKey())}</h1>
          <p className="sub">{t("home.sub")}</p>
        </div>
        <div className="tools">
          <div className="seg">
            <button className={sort === "fixed" ? "on" : ""} onClick={() => props.onSettings({ ...props.settings, homeSort: "fixed" })}>
              <Pin size={15} />
              {t("home.fixed")}
            </button>
            <button className={sort === "recent" ? "on" : ""} onClick={() => props.onSettings({ ...props.settings, homeSort: "recent" })}>
              <Clock size={15} />
              {t("home.recent")}
            </button>
          </div>
        </div>
      </header>
      {error && <div className="err top">{error}</div>}
      <div className={drag ? "grid dragging" : "grid"} ref={gridRef}>
        {shown.map((it) => tile(it))}
        <button className="tile add" onClick={addMenu}>
          <div className="ico l">
            <Plus size={26} />
          </div>
          <span className="lbl">{t("act.add")}</span>
        </button>
      </div>
      {items.length === 0 && <p className="home-hint">{t("home.addHint")}</p>}
      {dragged && tile(dragged, true)}
      {dropping && <div className="dropzone">{t("home.dropHere")}</div>}
      {menu && <Menu {...menu} onClose={() => setMenu(null)} />}
      {dialog?.kind === "url" && <AddUrlDialog onClose={() => setDialog(null)} />}
      {dialog?.kind === "plugin" && <AddPluginDialog plugins={addable} onClose={() => setDialog(null)} />}
      {editItem && (
        <EditDialog item={editItem} name={nameOf(editItem)} plugin={pluginOf(editItem)} platform={props.platform} onClose={() => setDialog(null)} />
      )}
    </div>
  );
}
