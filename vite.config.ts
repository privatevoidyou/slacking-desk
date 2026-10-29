import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: {
    // 两个页面：主窗口 index.html，插件窗口的宿主页 host.html
    rollupOptions: { input: { main: "index.html", host: "host.html" } },
  },
});
