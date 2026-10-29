# 接手说明（给 AI / 新开发者）

这份文件让接手的人不用翻聊天记录也能继续开发。**动手前先读完。** 用户文档见 `README.md`，平台差异见 `docs/platform.md`，插件开发见 market 仓库的 `docs/plugin-guide.md`。

## 1. 项目一句话

Slacking Desk / 摸鱼工作台：Tauri 2 桌面"壳"，所有功能都是插件。主要用户在中国大陆，主平台 Windows，其次 macOS，Linux 只保证壳本身和纯网页插件。用户是冷彻时光（GitHub `privatevoidyou`），定位"自己先用 + B 站个人开发者"，人设偏"摸鱼带师"。

- 仓库：`privatevoidyou/slacking-desk`（只放壳），`privatevoidyou/slacking-desk-market`（索引 + **所有插件** + 模板 + 插件文档）。都公开，MIT。
- bundle id：`com.privatevoidyou.slackingdesk`（定了不能改，决定 macOS / Linux 的数据目录）。
- **数据目录**（用户定）：Windows 放安装目录下的 `data\`（`main.rs` 的 `windows_data_dir`，写不进去才退回 `%LOCALAPPDATA%\<id>`；0.2.0 及以前在 `%LOCALAPPDATA%` 的数据第一次启动时自动搬过来）；WebView2 缓存用 `WEBVIEW2_USER_DATA_FOLDER` 也放进去；卸载时勾"删除应用数据"才删（`src-tauri/windows/hooks.nsh`）。macOS / Linux 用系统默认位置。代码里一律用 `data_dir()`，不要用 `app_local_data_dir()`。开发时（`tauri dev`）Windows 数据在 `target\debug\data\`。

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
- 网页快捷方式：添加时去这个网站取它自己的图标（`siteicon.rs`：apple-touch-icon > 最大的 icon > /favicon.ico，只请求这一个网站），取不到用首字母；编辑框可"重新获取网站图标"。（M2 验收时用户改的，原来是不抓。）以后取不到的多了，再考虑自己整理热门网站图标库。
- 内置插件可以禁用也可以卸载。
- 插件 SDK 命名：JS 全局对象 `slacking`；Python 是 `slacking_plugin.py`（零依赖）。

## 6. 路线图

| 里程碑 | 状态 | 内容 |
|---|---|---|
| M1 壳骨架 + 插件运行时 | ✅ 已合并 | 见第 4 节 |
| M2 首页与设置 | 已实现，待用户验收 | 见下面"M2 已定细节"和"M2 实现要点" |
| M3 安装器与市场 | | 本地 zip / 文件夹、GitHub Release、npm（只下载 tarball，用 integrity 校验，不执行 npm install）；sha256 校验；解压防路径穿越 / 符号链接 / 超大文件；升级、卸载、失败回滚；uv + 每插件独立 venv（Python 下载到壳数据目录，`UV_CACHE_DIR` 等指向数据目录）；pip 不当下载渠道；市场页；索引 **ed25519 签名**（私钥在 market 仓库 Secrets，公钥写进壳，到时教用户生成）；market 仓库加校验索引的 Action（要用户手动提交 workflow 文件）；国内镜像；**代理设置**三档：跟随系统（默认）/ 不使用 / 自定义 http 或 socks5（下载和 uv 都走它） |
| 之后 | | 插件**一个一个和用户讨论后再做**，不要一次做一批 |

### M2 已定细节（用户已确认）

范围：首页 + 部分设置，一次验收。

- **首页网格**：仿 iTab，图标居中、间距宽松、名字一行省略；只有 1×1 普通图标（数据里预留 `group`、`size`，以后做分组和 2×2 / 4×2 小组件）。数据存 `home.json`：`type / name / target / args / icon / lastUsed / order` 等。首次运行 M2 时把已启用插件迁移到首页。
- 末尾"+"格：添加网址 / 添加本地程序或文件 / 添加插件（已安装但不在首页的）；也可以把文件（可多个）拖到首页添加。插件安装时自动上首页、卸载时移除；纯后台插件不上首页；插件崩溃 / 无响应时图标显示红点。从首页移除 ≠ 卸载。
- **快捷方式**：能运行的（exe、lnk、bat、cmd、ps1；Mac 的 .app / .command / .sh）、网址（没写协议自动补 https://）、其他文件 / 文件夹用系统默认方式打开；可填参数；工作目录 = 文件所在文件夹。Windows 用 ShellExecute，Mac 用 `open`。bat / cmd 默认显示控制台，每个快捷方式可选"隐藏"；Windows 可选"以管理员身份运行"（插件图标没有）。目标不存在：图标变灰 + "找不到文件" + 可重新选择。
- **图标**：exe / lnk / .app 自动提取成 png，缓存在数据目录 `icons/`；其他用首字母 + 可选颜色；也可自定义图片。
- **右键菜单**：去掉所有 WebView 默认菜单（主窗口和插件窗口），空白处右键什么都不弹；输入框保留复制粘贴。图标右键：打开 / 编辑（名字、图标、参数、隐藏控制台、管理员）/ 从首页移除。正式版屏蔽 Ctrl+R、F5、Ctrl+P、Ctrl+F、F12（开发版保留）。图片不可拖拽。
- **排序**：固定顺序（可拖动）/ 最近使用（只统计从首页启动的次数，此模式下不能拖）。
- **壁纸**（壳的设置功能，不是插件，暂不开放插件 API）：无（默认）或本地图片（jpg / png / webp，≤30MB，不支持 gif），复制到数据目录；铺满 / 完整显示；模糊、透明度滑块（透明度是壁纸本身变淡，透出纯色底；原来的"变暗"用户要求去掉）；作用于整个主窗口。不做"使用桌面壁纸"。
- **界面风格：标准 / 毛玻璃**（和壁纸分开）：有壁纸时 → 侧栏、标题栏、主区域都不盖底，直接露出壁纸，只有卡片（设置分组、插件列表 / 详情、对话框、菜单）有底色：标准 = 卡片接近不透明，毛玻璃 = 卡片半透明磨砂；直接写在壁纸上的文字加阴影（M2 验收时用户定）；没壁纸时 → Win11 Mica / Acrylic、Mac vibrancy（Tauri window effects），Win10 / Linux → 纯色（用户定："默认的话改成纯色"，没壁纸时一律不用渐变）。风险：自定义标题栏 + 阴影和窗口效果的兼容、两种主题下的文字对比度。
- **设置分组**：外观（主题、界面风格、壁纸）/ 通用（语言、开机自启）/ 关于（版本、数据目录）。
- **开机自启**：默认关；开机启动时只待在托盘（`--autostart` 参数，tauri-plugin-autostart：Windows HKCU Run、Mac 登录项）；卸载程序要删掉 Run 键。
- **测试**：界面在 Linux 沙箱测；图标提取、bat、管理员、自启、Win11 毛玻璃要用户实测（写进清单）。

### M2 实现要点

- `home.rs`：`home.json` 读写、迁移（没有 home.json 时把已启用、有界面的插件加进来）、文件损坏时备份成 `home.json.bak` 再重建、保存时清理 `icons/` 里没人用的图片；`home_*` 命令；插件安装 / 移除时由 `plugins.rs` 调 `on_plugin_installed / on_plugin_removed`。
- `platform.rs`：`launch`（Win ShellExecuteExW，`runas` 管理员、`SW_HIDE` 隐藏控制台，ps1 用 `powershell -ExecutionPolicy Bypass -File`；Mac `open`，.sh 用终端打开；Linux 直接执行或 xdg-open）、`extract_icon`（Win IShellItemImageFactory 128px；Mac plutil + sips 读 .app 图标；Linux 不提取）、`glass_supported`（Win build ≥ 22000、Mac 是、Linux 否）。
- 壁纸和自定义图标通过 `appfile` 协议给主窗口读（只允许 `icons/`、`wallpaper/`），插件窗口用不了。
- 右键 / 快捷键屏蔽：主窗口和 host.html 用 `src/guard.ts`；插件自己的 HTML 由 `plugins.rs` 的 `inject_guard` 注入同样的脚本（插件作者不用管）。
- 窗口默认 `visible: false`，启动时没带 `--autostart` 才显示；毛玻璃且没壁纸且系统支持时才开 Mica / vibrancy（`apply_window_style`）。
- 拖动排序用 pointer 事件自己实现（没用 HTML5 拖放，Tauri 的文件拖入会和它冲突）。

### 记下但暂时不做

- 仿 Windows 搜索的弹出框（默认 Alt+Space，不用 Ctrl+Space，会和输入法冲突；在任务栏上方弹出，Mac 用菜单栏图标；搜首页条目、已安装程序、默认浏览器网页搜索；**不**真正嵌进 Win11 任务栏）。
- LLM 接口配置：以后做，形式是 提供商 ID、名称、API 地址（BaseURL）、协议、API Key（存钥匙串）、模型列表 + 连通性测试。
- 小组件（天气 / 日历 / 待办）：两种可能路线——数据模板渲染 / 叠加 WebView，到时再定；网格已预留 2×2、4×2。
- 图标分组、大时钟、图标大小选项（"不急"）。
- 插件接口原则：**等第一个真实插件需要时再实现**，文档里不写规划表。

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
7. Xvfb 里 WebKit 渲染会滞后，截图前先动一下鼠标再等 1～2 秒；启动时加 `WEBKIT_DISABLE_COMPOSITING_MODE=1` 更稳。文件选择对话框没法自动化，测壁纸时直接把图片复制到数据目录 `wallpaper/`，再改 `settings.json` 的 `wallpaper` 字段。
8. Windows 交叉检查需要 `llvm-rc`（装 llvm），否则 `cargo check --target x86_64-pc-windows-msvc` 会在 build script 报错。
9. `tests/bad-*` 的预期结果见各插件界面上的文字，以及 market 仓库 `docs/plugin-guide.md` 第 8 节。

## 8. 已知限制 / 待办

- binary 和 node 两种后端没有示例插件，没实测。
- "插件界面死循环不影响主窗口"在 Linux 上实测过，M1 由用户验收通过；macOS 上的表现以朋友的测试反馈为准。
- 未签名的 macOS 版通知能不能弹出不确定。
- 首页"+"菜单：用户反馈刚装完时弹出位置不对、之后又正常了，没复现，用户在观察。
- 网址图标暂时只读环境变量里的代理（`HTTPS_PROXY` 等），不读 Windows 系统代理；M3 做代理设置时一起接上。
- `Cargo.lock` 未入库；用户本地 clone 后可以补上。
