/*
 * 插件宿主页：每个插件窗口（界面 / 后台页）都先加载这里，再把插件页面放进沙箱 iframe。
 * iframe 没有 allow-same-origin，插件拿不到 Tauri IPC，只能通过 postMessage 找宿主页转发。
 * 宿主页不判断权限：Rust 按窗口 label 认出插件，再查权限。
 */
import { invoke } from "@tauri-apps/api/core";

interface HostConfig {
  id: string;
  kind: "ui" | "bg";
  src: string;
  theme: "light" | "dark" | "system";
}

const cfg = (window as unknown as { __SLACKING_HOST__: HostConfig }).__SLACKING_HOST__;
const dark = matchMedia("(prefers-color-scheme: dark)");

const frame = document.createElement("iframe");
frame.setAttribute("sandbox", "allow-scripts allow-forms allow-modals");
frame.src = cfg.src;
document.body.appendChild(frame);

function post(msg: unknown) {
  frame.contentWindow?.postMessage(msg, "*");
}

function resolvedTheme() {
  return cfg.theme === "system" ? (dark.matches ? "dark" : "light") : cfg.theme;
}

function applyTheme(notify: boolean) {
  const theme = resolvedTheme();
  document.documentElement.dataset.theme = theme;
  if (notify) post({ __slacking: "event", name: "theme.changed", data: { theme } });
}

applyTheme(false);
dark.addEventListener("change", () => cfg.theme === "system" && applyTheme(true));

window.addEventListener("message", async (e) => {
  if (e.source !== frame.contentWindow) return;
  const m = e.data;
  if (!m || typeof m !== "object") return;
  if (m.__slacking === "call" && typeof m.method === "string") {
    try {
      const result = await invoke("plugin_call", { method: m.method, params: m.params ?? null });
      post({ __slacking: "res", id: m.id, result });
    } catch (err) {
      const error = typeof err === "object" && err && "code" in err ? err : { code: "internal", message: String(err) };
      post({ __slacking: "res", id: m.id, error });
    }
  } else if (m.__slacking === "pong") {
    invoke("plugin_heartbeat").catch(() => {});
  }
});

// 下面两个函数由 Rust 通过 eval 调用
Object.assign(window, {
  /** 看门狗 ping：转给插件页面，插件 SDK 回 pong 后报告心跳 */
  __slackingPing: () => post({ __slacking: "ping" }),
  /** 设置里改了主题 */
  __slackingTheme: (theme: HostConfig["theme"]) => {
    cfg.theme = theme;
    applyTheme(true);
  },
});
