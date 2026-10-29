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

## 数据位置

- Windows：`%LOCALAPPDATA%\com.privatevoidyou.slackingdesk\`
- macOS：`~/Library/Application Support/com.privatevoidyou.slackingdesk/`

## 本地开发（可选）

需要 Rust（rustup）、Node.js 22+，以及各平台的 Tauri 依赖（Windows：MSVC Build Tools + WebView2；macOS：Xcode Command Line Tools）。

```bash
npm install
npx tauri icon src-tauri/icons/icon.svg -o src-tauri/icons
npx tauri dev
```

## 许可证

MIT
