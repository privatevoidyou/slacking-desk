# 接手说明（给 AI / 新开发者）

这份文件让接手的人不用翻聊天记录也能继续开发。**动手前先读完。** 用户文档见 `README.md`，平台差异见 `docs/platform.md`，插件开发见 market 仓库的 `docs/plugin-guide.md`。

## 1. 项目一句话

Slacking Desk / 摸鱼工作台：Tauri 2 桌面"壳"，所有功能都是插件。主要用户在中国大陆，主平台 Windows，其次 macOS，Linux 只保证壳本身和纯网页插件。用户是冷彻时光（GitHub `privatevoidyou`），定位"自己先用 + B 站个人开发者"，人设偏"摸鱼带师"。

- 仓库：`privatevoidyou/slacking-desk`（只放壳），`privatevoidyou/slacking-desk-market`（索引 + **所有插件** + 模板 + 插件文档）。都公开，MIT。
- bundle id：`com.privatevoidyou.slackingdesk`（定了不能改，决定数据目录）。

## 2. 和用户协作的硬规则（用户明确要求，必须遵守）

1. **不确定就停下来问，不要按自己的理解做一大堆**，这是最高优先级。用户讨厌浪费 token。
2. 先评审 / 给方案，用户确认后再写代码。每个里程碑完成后停下来汇报：改了哪些文件、怎么验证、Mac 测试清单（≤10 条，用户没有 Mac，由朋友帮测）。
3. 不往用户电脑装任何东西、不写任何文件，除非先说明并得到同意。沙箱里装东西可以。
4. 代码简单直接：不搞过度抽象、trait 工厂、层层接口；只在真正会失败的边界（下载、子进程、文件 IO、插件调用）处理错误，并把错误清楚显示给用户。
5. 插件协议宁小勿大：**每个字段、每个 API 都要有真实插件在用**才加。
6. 插件崩溃、卡死不能拖垮壳；进程插件能被强制结束。
7. 安全：安装时明示权限；API key 进系统钥匙串；下载做 hash 校验；不上报任何数据，不偷偷联网。
8. 平台差异集中在 `src-tauri/src/platform.rs`，并写进 `docs/platform.md`；做不到的直接写"做不到"，别硬凑。
9. 界面默认简体中文，同时支持 English；好看是硬要求（风格已确认，见第 5 节）。
10. 插件**一律放 market 仓库，不内嵌进壳**。只有设置类功能属于壳。
11. 用户的原则："能偷懒就偷懒，目标是能力结果，不是自建一大套套件"。

交流用中文，简洁。用户会在国内网络和代理两种环境下测试。

## 3. 交付流程

- 每个里程碑一个分支（`m1`、`m2`…），两个仓库各开一个 PR，用户验收后合并（M1 起用户授权由 AI 直接合并）。
- CI：`.github/workflows/build.yml`，推送到 `main` / `m*` 分支和 PR 时，在 windows-latest 和 macos-latest（universal）上构建，产物在 Actions 的 Artifacts（`slacking-desk-windows-latest` / `slacking-desk-macos-latest`）。可以用公开 API 轮询：`curl https://api.github.com/repos/privatevoidyou/slacking-desk/actions/runs?branch=m2`。
- 已知工具限制（用 GitHub MCP 推送时）：
  - **没有写 `.github/workflows/` 的权限（403）**。要改 CI 文件时，把完整文件发给用户，让他在网页上手动提交。
  - `push_files` 单次请求约 64KB 上限，大改动要分几次推。
  - 因此 `src-tauri/Cargo.lock`（100KB+）没有入库，见 `.gitignore`。图标只提交了 `icon.svg`，CI 里用 `npx tauri icon` 生成其他尺寸。
- 没签名：Windows 有 SmartScreen 提示；macOS 要 `xattr -cr` 或右键打开。不做自动更新。

## 4. 已定的架构（M1 已实现）

```
主窗口（首页 / 插件 / 设置）——只跑壳自己的代码
插件界面窗口  label "ui:<id 中的 . 换成 _>" ─┐  host.html → 沙箱 iframe（不给 allow-same-origin）
插件后台页    label "bg:<…>"（隐藏窗口）   ─┘  通过 postMessage → invoke("plugin_call")
插件后端      子进程，stdio JSON-RPC 2.0，一行一条 JSON（NDJSON）
```

- **Rust 靠窗口 label 认插件**，不信参数里的 id。插件窗口的 capability（`capabilities/plugin-host.json`）只允许 `plugin_call`、`plugin_heartbeat`。
- 插件文件走自定义协议：Windows `http://plugin.localhost/<id>/…`，其他 `plugin://localhost/<id>/…`。插件窗口只能读自己的文件（403）。`/__sdk__/slacking.js|css` 是 SDK。
- **CSP**：没有 `network` 权限时只允许插件协议地址。**不能写 `'self'`**：WebKit 在沙箱 iframe 里会把它当成放行任意 https（实测过）。
- **看门狗**：只管后台页。Rust 每 5 秒 eval 一次 ping，10 秒没 pong 就销毁窗口、标记"无响应"、发通知（启动有 5 秒宽限）。界面窗口卡死由用户直接关掉。
- **后端**：首次 `backend.call` 时启动；所有窗口关掉就停（`shutdown` → 关 stdin → 3 秒 → 强杀）。Windows 用 Job Object、Unix 用进程组杀整棵进程树。调用 30 秒超时。崩溃后标记"已崩溃"，显示 stderr 最后 20 行，**不自动重启**，等用户点"重启"。stderr 写 `logs/plugins/<id>.log`，超过 1MB 轮换成 `.log.1`。环境变量 `PYTHONUTF8=1`、`PYTHONUNBUFFERED=1`。M1 用系统 Python ≥3.10（Windows 先试 `py -3`）。
- **API 分发**：`api.rs` 里一个 match，先查权限再执行。错误码只有 6 个：`permission_denied`、`unsupported_platform`、`invalid_params`、`not_found`、`timeout`、`internal`。已实现：`app.info`、`storage.get/set/remove`（每插件 1MB，存在 `plugin-data/<id>/storage.json`）、`notify.show`（要 `notify` 权限）、`backend.call`（只限网页端调用）。
- **权限**：`manifest.rs` 的 `PERMISSIONS` 里有 9 个名字（notify、hotkey、windows.read、windows.control、wallpaper、network、llm、files、input），目前只有 notify 和 network 真正生效，其余是预留名字，对应 API 要等第一个用它的插件出现时再加。安装时整体同意，不能单项撤销；有 `background` 显示"常驻后台"，有 `backend` 醒目警告"包含本地程序，不受权限限制"。
- **安装记录** `installed.json`：`{"plugins":[{"id","source":{"type":"dev","path"},"enabled","grants":[],"allowBackground","allowBackend"}]}`。M1 只有开发者模式（直接用本地文件夹），重新加载时权限变多会再弹确认。
- 插件数据和本体分开：`plugin-data/<id>/`，移除时可选删不删数据。
- i18n：前端 `src/i18n.tsx` 里的 `useT(key, vars)`；Rust 用 `tr(中文, 英文)`；`plugin.json` 有可选 `i18n.en.{name,description}`。

**加一个新的插件 API 时**：`api.rs` 加分支（和需要的权限）→ `sdk/slacking.js` 加封装 → market 仓库的 `docs/plugin-guide.md` 写文档 → 必须有一个真实插件在用。平台相关的代码放 `platform.rs`。

## 5. 已确认的产品和视觉决定

- 视觉：样稿风格已确认（简洁、大圆角卡片、蓝色主色、浅色 / 深色 / 跟随系统）。纯 CSS，没用 Tailwind / shadcn。首页问候语是"摸鱼带师"口吻。
- 首页：插件和网页 / 软件快捷方式**混在一个网格里**；图标下方保留一行插件自己显示的状态小字（以后的 `home.setStatus`）。首次启动首页为空，不预装插件，不主动联网。
- 网页快捷方式不自动抓 favicon（会联网），用首字母图标，可自定义。
- 内置插件可以禁用也可以卸载。
- 插件 SDK 命名：JS 全局对象 `slacking`；Python 是 `slacking_plugin.py`（零依赖）。

## 6. 路线图

| 里程碑 | 状态 | 内容 |
|---|---|---|
| M1 壳骨架 + 插件运行时 | ✅ 已合并 | 见第 4 节 |
| M2 首页与设置 | 下一个 | 图标网格、拖拽、自定义图标；网页 / 软件快捷方式（读 exe、lnk、.app 图标）；两种排序（最近使用 / 固定顺序）；主题、开机自启、语言；多个 OpenAI 兼容 LLM 接口（key 存钥匙串）+ 连通性测试；**工作台换壁纸**；**代理设置**三档：跟随系统（默认）/ 不使用 / 自定义 http 或 socks5（M3 的下载和 uv 也走它）；**仿 Windows 搜索的弹出框**：默认快捷键 Alt+Space（可改，不用 Ctrl+Space，会和输入法冲突）或点托盘，在任务栏上方弹出，Mac 用菜单栏图标；搜首页条目、已安装程序（开始菜单 / 商店应用、Mac 的"应用程序"），以及用默认浏览器做网页搜索。**不**做真正嵌进 Win11 任务栏 |
| M3 安装器与市场 | | 本地 zip / 文件夹、GitHub Release、npm（只下载 tarball，用 integrity 校验，不执行 npm install）；sha256 校验；解压防路径穿越 / 符号链接 / 超大文件；升级、卸载、失败回滚；uv + 每插件独立 venv（Python 下载到壳数据目录，`UV_CACHE_DIR` 等指向数据目录）；pip 不当下载渠道；市场页；索引 **ed25519 签名**（私钥在 market 仓库 Secrets，公钥写进壳，到时教用户生成）；market 仓库加校验索引的 Action（要用户手动提交 workflow 文件）；国内镜像 |
| 之后 | | 插件**一个一个和用户讨论后再做**，不要一次做一批 |

用户首发最想要的插件：写作 / 阅读、无边框小窗、窗口托盘、闹钟、看电视壁纸、窗口透明度，以及以后的各种点子。老板键不是主角。原始需求里还有：快捷键大全、老板键、窗口录制脚本。已知决定：

- 纯纯写作以 https://github.com/privatevoidyou/book_desk 为基础（不是 NovelDesk）。
- 小说爬虫：自定义的精简书源格式，不兼容"阅读（Legado）"；只支持静态网页，限速，不绕过反爬。
- 看电视：**不自己做播放器 WebView**，用用户自己的浏览器全屏，插件只负责隐藏边框、联动老板键等，插件可以很薄。
- 透明度、无边框、托盘这类窗口句柄能力各自做成独立插件；多个插件同时操作同一个窗口可能互相影响，必须写明注意事项，由用户实测评估。
- macOS 做不到的（置顶别的应用的窗口、隐藏单个窗口、嵌入桌面层等）按 review 的结论降级或标"不支持"。

## 7. 本地测试方法（Linux 沙箱）

沙箱里不能跑 Windows / macOS，只能用 Linux + Xvfb 做端到端测试，Windows 用 `cargo check --target x86_64-pc-windows-msvc` 查编译错误（需要先 `rustup target add x86_64-pc-windows-msvc`），macOS 只能靠 CI。

1. 依赖（Amazon Linux / Fedora 包名）：`webkit2gtk4.1-devel gtk3-devel libsoup3-devel librsvg2-devel libappindicator-gtk3-devel openssl-devel xorg-x11-server-Xvfb xdotool ImageMagick`。
2. 构建：`npm ci && npx tauri icon src-tauri/icons/icon.svg -o src-tauri/icons && npx tauri build --debug --no-bundle`。**不要用裸 `cargo build`**：它会去连开发服务器，加载不了 `host.html`。
3. 预先写好 `~/.local/share/com.privatevoidyou.slackingdesk/installed.json`（格式见第 4 节，`grants` 填已同意的权限），把 market 仓库的 `templates/*`、`tests/*` 都加进去。
4. `Xvfb :99 -screen 0 1600x1000x24 &`，然后 `DISPLAY=:99 src-tauri/target/debug/slacking-desk`。
5. `tests/selftest` 启动约 10 秒后会把结果写进 `plugin-data/test.selftest/storage.json` 的 `results`。预期：`info`、`storageSetGet`、`backendEcho` 成功（`unknownApi` 为 `not_found`）；`backendFail` 为 `internal`；`backendMissing`、`unknownApi` 为 `not_found`；`tauri` 为 `undefined/undefined`；`parentDoc`、`localStorage`、`fetchIpc*`、`fetchNoCors`、`img` 都报错；`fetchOther` 为 403；`fetchOwn` 为 `test.selftest`。
6. 截图用 `import -window root x.png`，点击用 `xdotool mousemove X Y click 1`。没有窗口管理器时 `xdotool windowquit` 关不掉窗口，要用 python-xlib 发 `WM_DELETE_WINDOW`。关进程用 `pkill -x slacking-desk`（`pkill -f` 会把自己的 shell 也杀掉）。
7. `tests/bad-*` 的预期结果见各插件界面上的文字，以及 market 仓库 `docs/plugin-guide.md` 第 8 节。

## 8. 已知限制 / 待办

- binary 和 node 两种后端没有示例插件，没实测。
- "插件界面死循环不影响主窗口"在 Linux 上实测过，M1 由用户验收通过；macOS 上的表现以朋友的测试反馈为准。
- 未签名的 macOS 版通知能不能弹出不确定。
- `Cargo.lock` 未入库；用户本地 clone 后可以补上。
