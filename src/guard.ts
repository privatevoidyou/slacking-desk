/*
 * 去掉浏览器默认的右键菜单（输入框里保留复制粘贴）、禁止拖动图片；
 * 正式版再屏蔽刷新、打印、查找、开发者工具等快捷键（开发版保留，方便调试）。
 * 插件页面由 Rust 注入同样的脚本（plugins.rs 的 guard_js）。
 */
addEventListener(
  "contextmenu",
  (e) => {
    const t = e.target as HTMLElement | null;
    if (!t?.closest?.("input, textarea, [contenteditable]")) e.preventDefault();
  },
  true,
);
addEventListener("dragstart", (e) => (e.target as HTMLElement | null)?.tagName === "IMG" && e.preventDefault(), true);

if (import.meta.env.PROD) {
  addEventListener(
    "keydown",
    (e) => {
      const c = e.ctrlKey || e.metaKey;
      const k = e.key.toLowerCase();
      if (["F3", "F5", "F12"].includes(e.key) || (c && ["r", "p", "f", "g", "u"].includes(k)) || (c && e.shiftKey && ["i", "j", "c"].includes(k))) {
        e.preventDefault();
      }
    },
    true,
  );
}
