# 平台差异

壳在 Windows、macOS、Linux 上的行为差异都记在这里。代码里的平台差异集中在 `src-tauri/src/platform.rs`（少量和窗口外观有关的在 `tauri.windows.conf.json` / `tauri.macos.conf.json`），改动时同步更新本文。

Linux 在 v1 只保证壳本身和纯网页插件可用。

## 数据目录

| 系统 | 位置 |
|---|---|
| Windows | 安装目录下的 `data\`；写不进去时用 `%LOCALAPPDATA%\com.privatevoidyou.slackingdesk\` |
| macOS | `~/Library/Application Support/com.privatevoidyou.slackingdesk/` |
| Linux | `~/.local/share/com.privatevoidyou.slackingdesk/` |

目录内容：`settings.json`、`installed.json`、`plugins/`、`plugin-data/<id>/`、`logs/plugins/<id>.log`、`cache/`。

网页引擎也会在同一目录放自己的数据（Windows：`EBWebView/`；Linux：`CacheStorage/`、`WebKitCache/`、`storage/` 等；macOS 放在 `~/Library/WebKit/` 下），不要手动修改。

## 窗口外观

| | Windows | macOS | Linux |
|---|---|---|---|
| 主窗口标题栏 | 自绘（无系统边框，保留阴影） | 系统红绿灯 + 透明标题栏 | 系统标题栏 |
| 插件窗口 | 系统标题栏 | 系统标题栏 | 系统标题栏 |
| 关闭主窗口 | 收进托盘 | 收进菜单栏；点 Dock 图标重新打开 | 收进托盘 |

## 网页引擎与隔离

| | Windows | macOS | Linux |
|---|---|---|---|
| 引擎 | WebView2（Chromium） | WKWebView（Safari） | WebKitGTK |
| 插件文件地址 | `http://plugin.localhost/<id>/...` | `plugin://localhost/<id>/...` | `plugin://localhost/<id>/...` |

- 每个插件窗口（界面 / 后台页）都是独立的 webview，插件页面放在宿主页的沙箱 iframe 里。主窗口不运行任何插件代码。
- 插件页面卡死时，壳可以直接销毁它的窗口。主窗口是否完全不受影响取决于引擎是否把不同窗口放进不同进程，用 `tests/bad-web` 和 `tests/bad-background` 实测。Linux（WebKitGTK）已实测：插件界面死循环时主窗口照常可用，卡死的窗口能正常关闭；Windows / macOS 在 M1 验收时确认。
- 插件页面的 CSP 里不写 `'self'`，只写插件协议地址：WebKit 在沙箱 iframe（origin 为 null）里会把 `'self'` 当成放行任意 https 地址（Linux 实测外部图片和 fetch 都能加载）。
- 隐藏窗口里的定时器可能被引擎节流。看门狗的心跳由 Rust 主动发起，不受影响；插件自己的定时器（如闹钟）在 M4 实测并处理。

## 本地程序（插件后端）

| | Windows | macOS / Linux |
|---|---|---|
| 结束整棵进程树 | Job Object（`KILL_ON_JOB_CLOSE`） | 进程组（`killpg`） |
| 壳崩溃时 | 系统关闭 Job 句柄，插件进程自动结束 | 插件的 stdin 被关闭；用 SDK 的插件会自动退出，不读 stdin 的插件可能残留 |
| 黑色控制台窗口 | 用 `CREATE_NO_WINDOW` 隐藏 | 无 |

查找 Python 3.10+（按顺序，只缓存成功的结果）：

| 系统 | 候选 |
|---|---|
| Windows | `py -3`、`python`、`python3`（应用商店的 `python.exe` 占位程序会被跳过） |
| macOS | `/opt/homebrew/bin/python3`、`/usr/local/bin/python3`、`/Library/Frameworks/Python.framework/Versions/Current/bin/python3`、`python3` |
| Linux | `python3`、`python` |

macOS 从 Dock 启动的程序拿不到终端里的 PATH，所以要写全路径。系统自带的 `/usr/bin/python3` 通常是 3.9，版本不够会被跳过。

查找 Node.js：macOS 依次尝试 `/opt/homebrew/bin/node`、`/usr/local/bin/node`、`node`；其他系统用 `node`。

`binary` 插件按 `windows-x64`、`windows-arm64`、`macos-arm64`、`macos-x64`、`linux-x64` 选择可执行文件。

## 系统通知

- Windows：安装版显示为「Slacking Desk」。用 `tauri dev` 运行时可能显示为 PowerShell。
- macOS：首次发送时系统会询问是否允许通知。未签名的应用可能不显示通知，需要实测。
- Linux：需要桌面环境提供通知服务。

## 托盘

- Windows / Linux：左键单击打开主窗口，右键菜单「打开 / 退出」。
- macOS：菜单栏图标，行为相同。
