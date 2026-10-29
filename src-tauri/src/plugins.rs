//! 插件管理：安装记录、启停、窗口（界面 / 后台页）、看门狗、plugin:// 协议，以及主窗口用的命令。

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::http::{Request, Response};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_notification::NotificationExt;

use crate::api::{self, ApiError, Caller};
use crate::backend::{Backend, SpawnArgs};
use crate::manifest::{self, Manifest};
use crate::tr;

const SDK_JS: &str = include_str!("../sdk/slacking.js");
const SDK_CSS: &str = include_str!("../sdk/slacking.css");
/// 后台页刚启动时多给的时间，以及之后允许的最长无响应时间
const BG_GRACE: Duration = Duration::from_secs(5);
const BG_TIMEOUT: Duration = Duration::from_secs(10);

/// installed.json 里的一条记录
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub id: String,
    pub source: Source,
    pub enabled: bool,
    /// 用户同意过的权限
    pub grants: Vec<String>,
    #[serde(default)]
    pub allow_background: bool,
    #[serde(default)]
    pub allow_backend: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Source {
    /// 开发者模式：直接使用本地文件夹，不复制
    Dev { path: String },
}

#[derive(Default, Serialize, Deserialize)]
struct InstalledFile {
    plugins: Vec<Record>,
}

/// 给用户看的问题。kind：invalid / needsConfirm / crashed / unresponsive
#[derive(Clone, Serialize)]
pub struct Problem {
    pub kind: &'static str,
    pub message: String,
    pub detail: Vec<String>,
}

struct Plugin {
    record: Record,
    manifest: Option<Manifest>,
    warnings: Vec<String>,
    problem: Option<Problem>,
    backend: Option<Arc<Backend>>,
    /// 后台页在运行时是 Some：最近一次心跳（启动时预先加上宽限时间）
    last_pong: Option<Instant>,
}

impl Plugin {
    fn dir(&self) -> PathBuf {
        match &self.record.source {
            Source::Dev { path } => PathBuf::from(path),
        }
    }
    fn blocked(&self) -> bool {
        matches!(self.problem.as_ref().map(|p| p.kind), Some("invalid" | "needsConfirm"))
    }
}

pub struct Plugins {
    root: PathBuf,
    shell_version: String,
    list: Mutex<Vec<Plugin>>,
}

pub struct RunningInfo {
    pub grants: Vec<String>,
    pub data_dir: PathBuf,
}

// ---------- 启动与保存 ----------

pub fn init(root: PathBuf, shell_version: String) -> Plugins {
    let file: InstalledFile = std::fs::read_to_string(root.join("installed.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let list = file
        .plugins
        .into_iter()
        .map(|record| {
            let mut p = Plugin { record, manifest: None, warnings: vec![], problem: None, backend: None, last_pong: None };
            load_into(&mut p, &shell_version);
            p
        })
        .collect();
    Plugins { root, shell_version, list: Mutex::new(list) }
}

/// 重新读取清单，并检查是否需要再次确认权限
fn load_into(p: &mut Plugin, shell_version: &str) {
    p.manifest = None;
    p.problem = None;
    match manifest::load(&p.dir(), shell_version) {
        Err(errors) => {
            p.problem = Some(Problem { kind: "invalid", message: tr("插件清单有错误", "Invalid plugin manifest"), detail: errors });
        }
        Ok((m, warnings)) => {
            if m.id != p.record.id {
                p.problem = Some(Problem {
                    kind: "invalid",
                    message: tr("插件 id 变了，请移除后重新加载", "Plugin id changed; remove it and load again"),
                    detail: vec![format!("{} → {}", p.record.id, m.id)],
                });
            } else if !grants_cover(&p.record, &m) {
                p.problem = Some(Problem {
                    kind: "needsConfirm",
                    message: tr("插件申请的权限有变化，点「重新加载」确认", "Permissions changed; click Reload to review"),
                    detail: vec![],
                });
            }
            p.warnings = warnings;
            p.manifest = Some(m);
        }
    }
}

fn grants_cover(r: &Record, m: &Manifest) -> bool {
    m.permissions.iter().all(|p| r.grants.contains(p))
        && (m.background.is_none() || r.allow_background)
        && (m.backend.is_none() || r.allow_backend)
}

impl Plugins {
    /// 给首页初始化用：(已启用且有界面的插件, 所有已安装插件)
    pub fn home_seed(&self) -> (Vec<String>, Vec<String>) {
        let list = self.list.lock().unwrap();
        let ui = list
            .iter()
            .filter(|p| p.record.enabled && !p.blocked() && p.manifest.as_ref().is_some_and(|m| m.ui.is_some()))
            .map(|p| p.record.id.clone())
            .collect();
        (ui, list.iter().map(|p| p.record.id.clone()).collect())
    }

    fn save(&self, list: &[Plugin]) -> Result<(), String> {
        let file = InstalledFile { plugins: list.iter().map(|p| p.record.clone()).collect() };
        let text = serde_json::to_string_pretty(&file).expect("序列化不会失败");
        std::fs::write(self.root.join("installed.json"), text)
            .map_err(|e| tr(&format!("保存 installed.json 失败：{e}"), &format!("Failed to save installed.json: {e}")))
    }
    fn data_dir(&self, id: &str) -> PathBuf {
        self.root.join("plugin-data").join(id)
    }
    fn log_path(&self, id: &str) -> PathBuf {
        self.root.join("logs").join("plugins").join(format!("{id}.log"))
    }
}

fn state(app: &AppHandle) -> &Plugins {
    app.state::<Plugins>().inner()
}

fn changed(app: &AppHandle) {
    let _ = app.emit_to("main", "plugins-changed", ());
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

/// 壳启动后：给已启用、有后台页的插件启动后台页，并开始看门狗
pub fn start_all(app: &AppHandle) {
    let ids: Vec<String> = state(app).list.lock().unwrap().iter().map(|p| p.record.id.clone()).collect();
    for id in ids {
        let _ = start_background(app, &id);
    }
    spawn_watchdog(app.clone());
}

/// 壳退出时：直接结束所有插件进程
pub fn kill_all(app: &AppHandle) {
    let Some(s) = app.try_state::<Plugins>() else { return };
    for p in s.list.lock().unwrap().iter_mut() {
        if let Some(b) = p.backend.take() {
            b.kill_now();
        }
    }
}

// ---------- 窗口 ----------

fn label(kind: &str, id: &str) -> String {
    // 窗口 label 不允许 "."，插件 id 不允许 "_"，所以可以无损互换
    format!("{kind}:{}", id.replace('.', "_"))
}

/// "ui:example_hello" → ("ui", "example.hello")
pub fn parse_label(label: &str) -> Option<(&str, String)> {
    let (kind, rest) = label.split_once(':')?;
    matches!(kind, "ui" | "bg").then(|| (kind, rest.replace('_', ".")))
}

/// 插件文件的地址。Windows 的 WebView2 把自定义协议映射成 http://<scheme>.localhost
pub fn plugin_url(id: &str, rel: &str) -> String {
    let rel = rel.trim_start_matches("./").replace('\\', "/");
    if cfg!(windows) {
        format!("http://plugin.localhost/{id}/{rel}")
    } else {
        format!("plugin://localhost/{id}/{rel}")
    }
}

fn display_name(app: &AppHandle, m: &Manifest) -> String {
    let en = app.state::<crate::AppState>().settings.lock().unwrap().language == "en";
    en.then(|| m.i18n.get("en").and_then(|t| t.name.clone())).flatten().unwrap_or_else(|| m.name.clone())
}

/// 插件窗口 = 壳的宿主页（host.html）+ 沙箱 iframe 里的插件页面
fn create_host(app: &AppHandle, kind: &str, id: &str, m: &Manifest, page: &str) -> Result<(), String> {
    let theme = app.state::<crate::AppState>().settings.lock().unwrap().theme.clone();
    let cfg = json!({ "id": id, "kind": kind, "src": plugin_url(id, page), "theme": theme });
    let mut b = WebviewWindowBuilder::new(app, label(kind, id), WebviewUrl::App("host.html".into()))
        .title(display_name(app, m))
        .initialization_script(format!("window.__SLACKING_HOST__ = {cfg};"))
        .theme(match theme.as_str() {
            "dark" => Some(tauri::Theme::Dark),
            "light" => Some(tauri::Theme::Light),
            _ => None,
        });
    b = match (kind, &m.ui) {
        ("ui", Some(ui)) => b.inner_size(ui.width.unwrap_or(480) as f64, ui.height.unwrap_or(360) as f64).center(),
        _ => b.visible(false).skip_taskbar(true).inner_size(240.0, 160.0),
    };
    b.build().map(|_| ()).map_err(|e| tr(&format!("打开插件窗口失败：{e}"), &format!("Failed to open plugin window: {e}")))
}

pub fn open_ui(app: &AppHandle, id: &str) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(&label("ui", id)) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    let m = {
        let list = state(app).list.lock().unwrap();
        let p = list.iter().find(|p| p.record.id == id).ok_or_else(|| tr("找不到插件", "Plugin not found"))?;
        if !p.record.enabled || p.blocked() {
            return Err(tr("插件未启用或有错误", "Plugin is disabled or has errors"));
        }
        p.manifest.clone().expect("未被阻止的插件一定有清单")
    };
    let page = m.ui.as_ref().ok_or_else(|| tr("这个插件没有界面", "This plugin has no UI"))?.page.clone();
    create_host(app, "ui", id, &m, &page)?;
    changed(app);
    Ok(())
}

fn start_background(app: &AppHandle, id: &str) -> Result<(), String> {
    if app.get_webview_window(&label("bg", id)).is_some() {
        return Ok(());
    }
    let m = {
        let mut list = state(app).list.lock().unwrap();
        let Some(p) = list.iter_mut().find(|p| p.record.id == id) else { return Ok(()) };
        let runnable = p.record.enabled && !p.blocked() && p.record.allow_background && p.problem.is_none();
        match p.manifest.clone() {
            Some(m) if runnable && m.background.is_some() => {
                p.last_pong = Some(Instant::now() + BG_GRACE);
                m
            }
            _ => return Ok(()),
        }
    };
    let page = m.background.clone().expect("上面检查过");
    if let Err(e) = create_host(app, "bg", id, &m, &page) {
        let mut list = state(app).list.lock().unwrap();
        if let Some(p) = list.iter_mut().find(|p| p.record.id == id) {
            p.last_pong = None;
        }
        return Err(e);
    }
    changed(app);
    Ok(())
}

/// 关闭插件的所有窗口并停掉后端。返回界面窗口之前是否开着。
fn stop_plugin(app: &AppHandle, id: &str) -> bool {
    let ui_open = app.get_webview_window(&label("ui", id)).is_some();
    for kind in ["ui", "bg"] {
        if let Some(w) = app.get_webview_window(&label(kind, id)) {
            let _ = w.destroy();
        }
    }
    let mut list = state(app).list.lock().unwrap();
    if let Some(p) = list.iter_mut().find(|p| p.record.id == id) {
        p.last_pong = None;
        if let Some(b) = p.backend.take() {
            b.stop();
        }
    }
    ui_open
}

/// 插件窗口被销毁：后台页标记为停止；插件没有任何窗口时停掉后端
pub fn on_window_destroyed(app: &AppHandle, label_: &str) {
    let Some((kind, id)) = parse_label(label_) else { return };
    let other = if kind == "ui" { "bg" } else { "ui" };
    let other_open = app.get_webview_window(&label(other, &id)).is_some();
    let mut list = state(app).list.lock().unwrap();
    if let Some(p) = list.iter_mut().find(|p| p.record.id == id) {
        if kind == "bg" {
            p.last_pong = None;
        }
        if !other_open {
            if let Some(b) = p.backend.take() {
                b.stop();
            }
        }
    }
    drop(list);
    changed(app);
}

/// 看门狗：每 5 秒给后台页发一次 ping，10 秒没回应就停掉它。
/// ping 由 Rust 主动发起（eval），不依赖页面里的定时器，隐藏窗口的定时器节流不会造成误判。
fn spawn_watchdog(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let now = Instant::now();
            let mut dead = Vec::new();
            let mut alive = Vec::new();
            {
                let mut list = state(&app).list.lock().unwrap();
                for p in list.iter_mut() {
                    let Some(t) = p.last_pong else { continue };
                    if now > t + BG_TIMEOUT {
                        p.last_pong = None;
                        p.problem = Some(Problem {
                            kind: "unresponsive",
                            message: tr("后台页无响应，已停止", "Background page stopped responding and was stopped"),
                            detail: vec![],
                        });
                        dead.push((p.record.id.clone(), p.manifest.clone()));
                    } else {
                        alive.push(p.record.id.clone());
                    }
                }
            }
            for (id, m) in dead {
                if let Some(w) = app.get_webview_window(&label("bg", &id)) {
                    let _ = w.destroy();
                }
                let name = m.map(|m| display_name(&app, &m)).unwrap_or(id);
                notify(&app, &tr(&format!("插件「{name}」无响应"), &format!("\"{name}\" is not responding")), &tr("后台页已停止，可以在插件页重启", "Its background page was stopped. You can restart it on the Plugins page."));
                changed(&app);
            }
            for id in alive {
                if let Some(w) = app.get_webview_window(&label("bg", &id)) {
                    let _ = w.eval("window.__slackingPing && window.__slackingPing()");
                }
            }
        }
    });
}

fn on_backend_exit(app: &AppHandle, id: &str, backend: &Arc<Backend>, unexpected: bool, code: String) {
    let mut list = state(app).list.lock().unwrap();
    let Some(p) = list.iter_mut().find(|p| p.record.id == id) else { return };
    if p.backend.as_ref().is_some_and(|b| Arc::ptr_eq(b, backend)) {
        p.backend = None;
    }
    if !unexpected {
        drop(list);
        changed(app);
        return;
    }
    let tail: Vec<String> = backend.tail.lock().unwrap().iter().cloned().collect();
    p.problem = Some(Problem {
        kind: "crashed",
        message: tr(&format!("后端意外退出（退出码 {code}）"), &format!("Backend exited unexpectedly (code {code})")),
        detail: tail,
    });
    let name = p.manifest.as_ref().map(|m| display_name(app, m)).unwrap_or_else(|| id.to_string());
    drop(list);
    notify(app, &tr(&format!("插件「{name}」的后端崩溃了"), &format!("\"{name}\" backend crashed")), &tr("可以在插件页查看日志并重启", "See the log and restart it on the Plugins page."));
    changed(app);
}

// ---------- 给 API 用 ----------

pub fn running_info(app: &AppHandle, id: &str) -> Option<RunningInfo> {
    let s = state(app);
    let list = s.list.lock().unwrap();
    let p = list.iter().find(|p| p.record.id == id)?;
    (p.record.enabled && !p.blocked()).then(|| RunningInfo { grants: p.record.grants.clone(), data_dir: s.data_dir(id) })
}

/// 取得插件的后端进程，没有就启动。崩溃后不自动重启，等用户点「重启」。
pub fn ensure_backend(app: &AppHandle, id: &str) -> Result<Arc<Backend>, String> {
    let s = state(app);
    let mut list = s.list.lock().unwrap();
    let p = list.iter_mut().find(|p| p.record.id == id).ok_or_else(|| tr("找不到插件", "Plugin not found"))?;
    if let Some(b) = &p.backend {
        return Ok(b.clone());
    }
    if p.problem.as_ref().is_some_and(|x| x.kind == "crashed") {
        return Err(tr("后端已崩溃，请在插件页点「重启」后再试", "Backend crashed. Click Restart on the Plugins page first."));
    }
    let m = p.manifest.clone().ok_or_else(|| tr("插件清单有错误", "Invalid plugin manifest"))?;
    let spec = m.backend.as_ref().ok_or_else(|| tr("这个插件没有后端", "This plugin has no backend"))?;
    if !p.record.allow_backend {
        return Err(tr("没有同意运行这个插件的本地程序", "Running this plugin's local program was not approved"));
    }
    let data_dir = s.data_dir(id);
    std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
    let language = app.state::<crate::AppState>().settings.lock().unwrap().language.clone();
    let handle = app.clone();
    let plugin_id = id.to_string();
    let b = Backend::spawn(SpawnArgs {
        app: app.clone(),
        id,
        dir: &p.dir(),
        spec,
        log_path: s.log_path(id),
        init: json!({
            "shellVersion": s.shell_version,
            "pluginId": id,
            "dataDir": data_dir.display().to_string(),
            "language": language,
            "platform": crate::platform_name(),
        }),
        on_exit: Box::new(move |b, unexpected, code| on_backend_exit(&handle, &plugin_id, b, unexpected, code)),
    })?;
    p.backend = Some(b.clone());
    drop(list);
    changed(app);
    Ok(b)
}

/// 设置改了主题后通知所有插件窗口
pub fn broadcast_theme(app: &AppHandle, theme: &str) {
    for (l, w) in app.webview_windows() {
        if parse_label(&l).is_some() {
            let _ = w.eval(format!("window.__slackingTheme && window.__slackingTheme({})", json!(theme)));
            let _ = w.set_theme(match theme {
                "dark" => Some(tauri::Theme::Dark),
                "light" => Some(tauri::Theme::Light),
                _ => None,
            });
        }
    }
}

// ---------- plugin:// 协议 ----------

/// 插件文件只能被它自己的窗口读取（主窗口可以读所有插件的图标）。
/// 响应头里的 CSP 决定插件能不能联网：没有 network 权限就只能访问自己的文件。
pub fn serve(app: &AppHandle, caller: &str, req: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let path = percent_decode(req.uri().path());
    let path = path.trim_start_matches('/');
    let (first, rest) = path.split_once('/').unwrap_or((path, ""));
    if first == "__sdk__" {
        return match rest {
            "slacking.js" => respond(200, "text/javascript; charset=utf-8", SDK_JS.as_bytes().to_vec(), None),
            "slacking.css" => respond(200, "text/css; charset=utf-8", SDK_CSS.as_bytes().to_vec(), None),
            _ => not_found(rest),
        };
    }
    let id = first;
    let allowed = caller == "main" || parse_label(caller).is_some_and(|(_, owner)| owner == id);
    if !allowed {
        return respond(403, "text/plain; charset=utf-8", b"forbidden".to_vec(), None);
    }
    let found = {
        let list = state(app).list.lock().unwrap();
        list.iter().find(|p| p.record.id == id).map(|p| (p.dir(), p.record.grants.iter().any(|g| g == "network")))
    };
    let Some((dir, network)) = found else { return not_found(path) };
    match manifest::resolve(&dir, rest).and_then(|f| std::fs::read(&f).map_err(|e| e.to_string())) {
        Ok(bytes) if mime(rest).starts_with("text/html") => respond(200, mime(rest), inject_guard(bytes), Some(csp(network))),
        Ok(bytes) => respond(200, mime(rest), bytes, Some(csp(network))),
        Err(_) => not_found(rest),
    }
}

/// 插件页面也去掉浏览器默认右键菜单、屏蔽刷新等快捷键（和主窗口一致）。
/// 插入到 <head> 后面，不放在 doctype 前面（否则页面会进入怪异模式）。
fn inject_guard(bytes: Vec<u8>) -> Vec<u8> {
    let tag = format!("<script>{}</script>", guard_js());
    let html = String::from_utf8_lossy(&bytes).into_owned();
    let lower = html.to_ascii_lowercase();
    let at = ["<head", "<html"]
        .iter()
        .find_map(|t| lower.find(t).and_then(|i| lower[i..].find('>').map(|j| i + j + 1)))
        .unwrap_or(0);
    let mut out = html;
    out.insert_str(at, &tag);
    out.into_bytes()
}

/// 和前端 src/guard.ts 做的事一样。正式版才屏蔽快捷键，开发版保留方便调试。
pub fn guard_js() -> String {
    let block_keys = !cfg!(debug_assertions);
    format!(
        r#"(function(){{var k={block_keys};addEventListener("contextmenu",function(e){{var t=e.target;if(!(t&&t.closest&&t.closest("input,textarea,[contenteditable]")))e.preventDefault();}},true);addEventListener("dragstart",function(e){{if(e.target&&e.target.tagName==="IMG")e.preventDefault();}},true);if(k)addEventListener("keydown",function(e){{var c=e.ctrlKey||e.metaKey,x=e.key.toLowerCase();if(e.key==="F5"||e.key==="F12"||e.key==="F3"||(c&&(x==="r"||x==="p"||x==="f"||x==="g"||x==="u"))||(c&&e.shiftKey&&(x==="i"||x==="j"||x==="c")))e.preventDefault();}},true);}})();"#
    )
}

fn csp(network: bool) -> String {
    // 不能写 'self'：WebKit 在沙箱 iframe（origin 为 null）里把 'self' 当成允许任意 https 地址，
    // 实测会放行外部图片和 fetch。只写插件协议本身的地址。
    let own = "http://plugin.localhost plugin:";
    let net = if network { " https: http: wss: ws:" } else { "" };
    format!(
        "default-src {own} data: blob:{net}; script-src {own} 'unsafe-inline' 'unsafe-eval' blob:{net}; \
         style-src {own} 'unsafe-inline'{net}; connect-src {own}{net}; frame-src {own}{net}; \
         form-action {own}; base-uri {own}"
    )
}

fn respond(status: u16, mime: &str, body: Vec<u8>, csp: Option<String>) -> Response<Cow<'static, [u8]>> {
    let mut r = Response::builder()
        .status(status)
        .header("Content-Type", mime)
        // 插件页面在没有 allow-same-origin 的沙箱里（origin 为 null），加载模块脚本需要 CORS
        .header("Access-Control-Allow-Origin", "*")
        // 开发时改了文件，重新加载就能看到
        .header("Cache-Control", "no-store");
    if let Some(c) = csp {
        r = r.header("Content-Security-Policy", c);
    }
    r.body(Cow::Owned(body)).expect("响应头都是合法的")
}

fn not_found(path: &str) -> Response<Cow<'static, [u8]>> {
    let msg = tr(&format!("找不到文件：{path}"), &format!("File not found: {path}"));
    respond(404, "text/plain; charset=utf-8", msg.into_bytes(), None)
}

fn mime(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "txt" | "md" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------- 主窗口用的命令 ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginView {
    id: String,
    path: String,
    enabled: bool,
    grants: Vec<String>,
    allow_background: bool,
    allow_backend: bool,
    manifest: Option<Manifest>,
    warnings: Vec<String>,
    icon_url: Option<String>,
    problem: Option<Problem>,
    ui_open: bool,
    bg_running: bool,
    backend_running: bool,
}

#[tauri::command]
pub async fn plugins_list(app: AppHandle) -> Vec<PluginView> {
    let list = state(&app).list.lock().unwrap();
    list.iter()
        .map(|p| PluginView {
            id: p.record.id.clone(),
            path: p.dir().display().to_string(),
            enabled: p.record.enabled,
            grants: p.record.grants.clone(),
            allow_background: p.record.allow_background,
            allow_backend: p.record.allow_backend,
            icon_url: p.manifest.as_ref().and_then(|m| m.icon.as_ref()).map(|i| plugin_url(&p.record.id, i)),
            manifest: p.manifest.clone(),
            warnings: p.warnings.clone(),
            problem: p.problem.clone(),
            ui_open: app.get_webview_window(&label("ui", &p.record.id)).is_some(),
            bg_running: p.last_pong.is_some(),
            backend_running: p.backend.is_some(),
        })
        .collect()
}

/// 安装确认框要显示的内容
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    path: String,
    manifest: Option<Manifest>,
    errors: Vec<String>,
    warnings: Vec<String>,
    /// 已经有同 id 的插件
    replaces: bool,
}

fn inspect(app: &AppHandle, path: &Path) -> Inspection {
    let s = state(app);
    let (manifest, errors, warnings) = match manifest::load(path, &s.shell_version) {
        Ok((m, w)) => (Some(m), vec![], w),
        Err(e) => (None, e, vec![]),
    };
    let replaces = manifest.as_ref().is_some_and(|m| s.list.lock().unwrap().iter().any(|p| p.record.id == m.id));
    Inspection { path: path.display().to_string(), manifest, errors, warnings, replaces }
}

/// 选择本地插件文件夹（开发者模式）。取消时返回 None。
#[tauri::command]
pub async fn plugin_pick_folder(app: AppHandle) -> Option<Inspection> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app.dialog().file().blocking_pick_folder()?;
    let path = picked.into_path().ok()?;
    Some(inspect(&app, &path))
}

/// 用户在确认框里点了「同意并加载」
#[tauri::command]
pub async fn plugin_add(app: AppHandle, path: String) -> Result<(), String> {
    let s = state(&app);
    let (m, warnings) = manifest::load(Path::new(&path), &s.shell_version).map_err(|e| e.join("\n"))?;
    let id = m.id.clone();
    stop_plugin(&app, &id);
    let new_ui;
    {
        let mut list = s.list.lock().unwrap();
        let record = Record {
            id: id.clone(),
            source: Source::Dev { path },
            enabled: true,
            grants: m.permissions.clone(),
            allow_background: m.background.is_some(),
            allow_backend: m.backend.is_some(),
        };
        let has_ui = m.ui.is_some();
        let p = Plugin { record, manifest: Some(m), warnings, problem: None, backend: None, last_pong: None };
        let is_new = match list.iter_mut().find(|x| x.record.id == id) {
            Some(old) => {
                *old = p;
                false
            }
            None => {
                list.push(p);
                true
            }
        };
        s.save(&list)?;
        new_ui = is_new && has_ui;
    }
    // 新装的、有界面的插件自动放到首页；纯后台插件不放
    if new_ui {
        crate::home::on_plugin_installed(&app, &id);
    }
    start_background(&app, &id)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn plugin_set_enabled(app: AppHandle, id: String, enabled: bool) -> Result<(), String> {
    stop_plugin(&app, &id);
    {
        let s = state(&app);
        let mut list = s.list.lock().unwrap();
        let p = list.iter_mut().find(|p| p.record.id == id).ok_or_else(|| tr("找不到插件", "Plugin not found"))?;
        p.record.enabled = enabled;
        if enabled {
            load_into(p, &s.shell_version);
        }
        s.save(&list)?;
    }
    if enabled {
        start_background(&app, &id)?;
    }
    changed(&app);
    Ok(())
}

/// 重新读取插件文件夹。权限有变化时返回确认框内容，插件保持停止，等用户确认。
#[tauri::command]
pub async fn plugin_reload(app: AppHandle, id: String) -> Result<Option<Inspection>, String> {
    let ui_was_open = stop_plugin(&app, &id);
    let (needs_confirm, dir) = {
        let s = state(&app);
        let mut list = s.list.lock().unwrap();
        let p = list.iter_mut().find(|p| p.record.id == id).ok_or_else(|| tr("找不到插件", "Plugin not found"))?;
        load_into(p, &s.shell_version);
        (p.problem.as_ref().is_some_and(|x| x.kind == "needsConfirm"), p.dir())
    };
    changed(&app);
    if needs_confirm {
        return Ok(Some(inspect(&app, &dir)));
    }
    start_background(&app, &id)?;
    if ui_was_open {
        let _ = open_ui(&app, &id);
    }
    Ok(None)
}

/// 清除「已崩溃 / 无响应」状态，重新启动后台页（后端在下次调用时启动）
#[tauri::command]
pub async fn plugin_restart(app: AppHandle, id: String) -> Result<(), String> {
    {
        let mut list = state(&app).list.lock().unwrap();
        let p = list.iter_mut().find(|p| p.record.id == id).ok_or_else(|| tr("找不到插件", "Plugin not found"))?;
        if matches!(p.problem.as_ref().map(|x| x.kind), Some("crashed" | "unresponsive")) {
            p.problem = None;
        }
    }
    start_background(&app, &id)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn plugin_remove(app: AppHandle, id: String, delete_data: bool) -> Result<(), String> {
    stop_plugin(&app, &id);
    let s = state(&app);
    {
        let mut list = s.list.lock().unwrap();
        list.retain(|p| p.record.id != id);
        s.save(&list)?;
    }
    if delete_data {
        let dir = s.data_dir(&id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| tr(&format!("删除插件数据失败：{e}"), &format!("Failed to delete plugin data: {e}")))?;
        }
    }
    crate::home::on_plugin_removed(&app, &id);
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn plugin_open(app: AppHandle, id: String) -> Result<(), String> {
    open_ui(&app, &id)
}

/// 用系统默认程序打开插件日志（还没有日志时打开日志文件夹）
#[tauri::command]
pub async fn plugin_open_log(app: AppHandle, id: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let path = state(&app).log_path(&id);
    let target = if path.exists() { path } else { path.parent().map(Path::to_path_buf).unwrap_or_default() };
    let _ = std::fs::create_dir_all(&target);
    app.opener().open_path(target.display().to_string(), None::<&str>).map_err(|e| e.to_string())
}

/// 打开插件窗口的开发者工具（优先界面窗口，其次后台页）
#[tauri::command]
pub async fn plugin_devtools(app: AppHandle, id: String) -> Result<(), String> {
    let w = app
        .get_webview_window(&label("ui", &id))
        .or_else(|| app.get_webview_window(&label("bg", &id)))
        .ok_or_else(|| tr("插件窗口没有打开", "No plugin window is open"))?;
    w.open_devtools();
    Ok(())
}

// ---------- 插件窗口（宿主页）用的命令 ----------

/// 插件身份只从窗口 label 判断，不相信参数
#[tauri::command]
pub async fn plugin_call(app: AppHandle, webview: tauri::Webview, method: String, params: Value) -> Result<Value, ApiError> {
    let (_, id) = parse_label(webview.label()).ok_or_else(|| ApiError::new("permission_denied", "not a plugin window".into()))?;
    api::dispatch(&app, &id, Caller::Web, &method, params).await
}

#[tauri::command]
pub async fn plugin_heartbeat(app: AppHandle, webview: tauri::Webview) {
    let Some((kind, id)) = parse_label(webview.label()) else { return };
    if kind != "bg" {
        return;
    }
    let mut list = state(&app).list.lock().unwrap();
    if let Some(p) = list.iter_mut().find(|p| p.record.id == id) {
        if p.last_pong.is_some() {
            p.last_pong = Some(Instant::now());
        }
    }
}
