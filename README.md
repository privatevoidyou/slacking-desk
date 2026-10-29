# Slacking Desk / 摸鱼工作台

插件优先的桌面工作台：壳本身只负责插件的发现、安装、启停和展示，所有功能都是插件。插件和插件开发文档在 [slacking-desk-market](https://github.com/privatevoidyou/slacking-desk-market)。

> 当前处于开发预览阶段（M1）。

## 下载试用

在本仓库的 Actions 页面打开最新一次 `build`，在 Artifacts 里下载：

- Windows：`slacking-desk-windows-latest`，解压后运行 `.exe`（NSIS）或 `.msi`。未签名，SmartScreen 提示时点「更多信息 → 仍要运行」。
- macOS：`slacking-desk-macos-latest`，打开 `.dmg` 拖进「应用程序」。未签名，首次打开前在终端执行：

  ```bash
  xattr -cr "/Applications/Slacking Desk.app"
  ```

## 试用插件

壳本身不带插件。M1 阶段可以用「插件 → 加载本地文件夹」加载 [slacking-desk-market](https://github.com/privatevoidyou/slacking-desk-market) 里的示例插件（下载 ZIP 解压，选择 `templates/js-plugin` 等文件夹）。

## 数据位置

- Windows：`%LOCALAPPDATA%\com.privatevoidyou.slackingdesk\`
- macOS：`~/Library/Application Support/com.privatevoidyou.slackingdesk/`

插件日志在数据目录的 `logs/plugins/` 里。各平台的行为差异见 [docs/platform.md](docs/platform.md)。

## 代码结构

| 路径 | 内容 |
|---|---|
| `src/` | 主窗口界面（React） |
| `src/host.ts`、`host.html` | 插件窗口的宿主页：把插件页面放进沙箱 iframe，转发 API 调用 |
| `src-tauri/src/plugins.rs` | 插件管理、插件窗口、看门狗、`plugin://` 协议 |
| `src-tauri/src/api.rs` | 插件 API 分发和权限检查 |
| `src-tauri/src/backend.rs` | 插件本地程序（JSON-RPC over stdio） |
| `src-tauri/src/manifest.rs` | `plugin.json` 校验 |
| `src-tauri/src/platform.rs` | 平台适配层 |
| `src-tauri/sdk/` | 插件 SDK（`/__sdk__/slacking.js`、`slacking.css`） |
| `src-tauri/capabilities/` | 主窗口和插件窗口各自能调用的命令 |

## 本地开发（可选）

需要 Rust（rustup）、Node.js 22+，以及各平台的 Tauri 依赖（Windows：MSVC Build Tools + WebView2；macOS：Xcode Command Line Tools）。

```bash
npm install
npx tauri icon src-tauri/icons/icon.svg -o src-tauri/icons
npx tauri dev
```

## 许可证

MIT
