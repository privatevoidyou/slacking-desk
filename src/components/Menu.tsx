import { useEffect, useRef, type ReactNode } from "react";

export interface MenuItem {
  label: string;
  icon?: ReactNode;
  hint?: string;
  danger?: boolean;
  disabled?: boolean;
  onClick: () => void;
}

/** 自绘的弹出菜单（右键菜单、"+"菜单）。点外面、按 Esc、滚动时关闭。 */
export default function Menu(props: { x: number; y: number; items: (MenuItem | "sep")[]; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const close = (e: Event) => {
      if (e instanceof MouseEvent && ref.current?.contains(e.target as Node)) return;
      props.onClose();
    };
    const key = (e: KeyboardEvent) => e.key === "Escape" && props.onClose();
    addEventListener("mousedown", close, true);
    addEventListener("contextmenu", close, true);
    addEventListener("blur", close);
    addEventListener("keydown", key);
    addEventListener("wheel", close, true);
    return () => {
      removeEventListener("mousedown", close, true);
      removeEventListener("contextmenu", close, true);
      removeEventListener("blur", close);
      removeEventListener("keydown", key);
      removeEventListener("wheel", close, true);
    };
  }, [props.onClose]);
  // 不超出窗口
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    if (r.right > innerWidth - 8) el.style.left = `${Math.max(8, innerWidth - r.width - 8)}px`;
    if (r.bottom > innerHeight - 8) el.style.top = `${Math.max(8, props.y - r.height)}px`;
  }, [props.x, props.y]);
  return (
    <div className="menu" ref={ref} style={{ left: props.x, top: props.y }} role="menu">
      {props.items.map((it, i) =>
        it === "sep" ? (
          <div key={i} className="sep" />
        ) : (
          <button
            key={i}
            className={it.danger ? "mi red" : "mi"}
            disabled={it.disabled}
            role="menuitem"
            onClick={() => {
              props.onClose();
              it.onClick();
            }}
          >
            {it.icon}
            {it.label}
            {it.hint && <span className="kbd">{it.hint}</span>}
          </button>
        ),
      )}
    </div>
  );
}
