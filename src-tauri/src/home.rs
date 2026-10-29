//! 首页：插件和快捷方式混在一个网格里，保存在数据目录的 home.json。
//! 自动提取的图标、自定义图片放在数据目录的 icons/，通过 appfile:// 协议给主窗口显示。

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::{platform, plugins, siteicon, tr};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    /// "plugin"、"url" 或 "file"（程序、脚本、普通文件、文件夹都算 file）
    #[serde(rename = "type")]
    pub kind: String,
    /// 显示名。插件为空时显示插件自己的名字（跟随界面语言）
    #[serde(default)]
    pub name: String,
    /// 插件 id / 网址 / 本地路径
    pub target: String,
    #[serde(default)]
    pub args: String,
    #[serde(default)]
    pub icon: Icon,
    /// 自动提取的图标文件名（icons/ 里），提取失败为 None
    #[serde(default)]
    pub auto_icon: Option<String>,
    /// bat / cmd / ps1 不显示控制台窗口
    #[serde(default)]
    pub hide_console: bool,
    /// 以管理员身份运行（只有 Windows）
    #[serde(default)]
    pub run_as_admin: bool,
    /// 最近一次从首页打开的时间（毫秒），0 = 没打开过
    #[serde(default)]
    pub last_used: u64,
    /// 固定顺序里的位置（保存时按列表顺序重写）
    #[serde(default)]
    pub order: usize,
    /// 预留：图标分组
    #[serde(default)]
    pub group: Option<String>,
    /// 预留：小组件尺寸，现在只有 "1x1"
    #[serde(default = "one_by_one")]
    pub size: String,
}

fn one_by_one() -> String {
    "1x1".into()
}

/// kind："auto"（插件图标 / 程序图标，没有就用首字母）、"letter"（首字母 + 颜色）、"image"（自定义图片）
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Icon {
    pub kind: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
}

impl Default for Icon {
    fn default() -> Self {
        Self { kind: "auto".into(), color: None, file: None }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct HomeFile {
    #[serde(default)]
    items: Vec<Item>,
}

pub struct Home {
    root: PathBuf,
    items: Mutex<Vec<Item>>,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn new_id() -> String {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    format!("{:x}{:x}", now_ms(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

fn state(app: &AppHandle) -> &Home {
    app.state::<Home>().inner()
}

fn changed(app: &AppHandle) {
    let _ = app.emit_to("main", "home-changed", ());
}

impl Home {
    fn save(&self, items: &mut [Item]) -> Result<(), String> {
        for (i, it) in items.iter_mut().enumerate() {
            it.order = i;
        }
        let text = serde_json::to_string_pretty(&HomeFile { items: items.to_vec() }).expect("序列化不会失败");
        std::fs::write(self.root.join("home.json"), text)
            .map_err(|e| tr(&format!("保存 home.json 失败：{e}"), &format!("Failed to save home.json: {e}")))?;
        // 删掉不再使用的图标文件
        let used: Vec<&str> = items.iter().flat_map(|i| [i.auto_icon.as_deref(), i.icon.file.as_deref()]).flatten().collect();
        if let Ok(rd) = std::fs::read_dir(self.root.join("icons")) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if !used.contains(&name.as_str()) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        Ok(())
    }

    fn icons_dir(&self) -> PathBuf {
        self.root.join("icons")
    }
}

/// 启动时调用。第一次运行 M2（还没有 home.json）时，把已启用、有界面的插件放到首页。
pub fn init(root: PathBuf, ui_plugins: &[String], installed: &[String]) -> Home {
    let path = root.join("home.json");
    let mut items = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<HomeFile>(&text)
            .unwrap_or_else(|e| {
                eprintln!("home.json 格式错误：{e}");
                // 不覆盖用户的文件：先备份
                let _ = std::fs::copy(&path, root.join("home.json.bak"));
                HomeFile::default()
            })
            .items,
        Err(_) => ui_plugins.iter().map(|id| plugin_item(id)).collect(),
    };
    items.sort_by_key(|i| i.order);
    // 插件已经被删掉的（例如手动改了 installed.json），从首页去掉
    items.retain(|i| i.kind != "plugin" || installed.contains(&i.target));
    let home = Home { root, items: Mutex::new(vec![]) };
    let _ = home.save(&mut items);
    *home.items.lock().unwrap() = items;
    home
}

fn plugin_item(id: &str) -> Item {
    Item {
        id: new_id(),
        kind: "plugin".into(),
        name: String::new(),
        target: id.into(),
        args: String::new(),
        icon: Icon::default(),
        auto_icon: None,
        hide_console: false,
        run_as_admin: false,
        last_used: 0,
        order: 0,
        group: None,
        size: one_by_one(),
    }
}

/// 新安装的插件（有界面）自动放到首页
pub fn on_plugin_installed(app: &AppHandle, id: &str) {
    let s = state(app);
    let mut items = s.items.lock().unwrap();
    if items.iter().any(|i| i.kind == "plugin" && i.target == id) {
        return;
    }
    items.push(plugin_item(id));
    let _ = s.save(&mut items);
    drop(items);
    changed(app);
}

/// 卸载插件时从首页去掉
pub fn on_plugin_removed(app: &AppHandle, id: &str) {
    let s = state(app);
    let mut items = s.items.lock().unwrap();
    items.retain(|i| !(i.kind == "plugin" && i.target == id));
    let _ = s.save(&mut items);
    drop(items);
    changed(app);
}

/// 主窗口显示用：在 Item 基础上加图标地址和"找不到文件"
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemView {
    #[serde(flatten)]
    item: Item,
    auto_icon_url: Option<String>,
    image_url: Option<String>,
    missing: bool,
}

/// 主窗口读取数据目录文件（图标、壁纸）的地址
pub fn appfile_url(rel: &str) -> String {
    if cfg!(windows) {
        format!("http://appfile.localhost/{rel}")
    } else {
        format!("appfile://localhost/{rel}")
    }
}

#[tauri::command]
pub async fn home_list(app: AppHandle) -> Vec<ItemView> {
    let items = state(&app).items.lock().unwrap().clone();
    items
        .into_iter()
        .map(|item| ItemView {
            auto_icon_url: item.auto_icon.as_ref().map(|f| appfile_url(&format!("icons/{f}"))),
            image_url: item.icon.file.as_ref().map(|f| appfile_url(&format!("icons/{f}"))),
            missing: item.kind == "file" && !Path::new(&item.target).exists(),
            item,
        })
        .collect()
}

/// 网址没写协议时补上 https://
fn normalize_url(url: &str) -> Result<String, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err(tr("请填写网址", "Please enter a URL"));
    }
    if url.contains("://") || url.starts_with("mailto:") {
        Ok(url.to_string())
    } else {
        Ok(format!("https://{url}"))
    }
}

fn url_default_name(url: &str) -> String {
    let host = url.split("://").nth(1).unwrap_or(url).split(['/', '?', '#']).next().unwrap_or(url);
    host.trim_start_matches("www.").to_string()
}

#[tauri::command]
pub async fn home_add_url(app: AppHandle, name: String, url: String) -> Result<(), String> {
    let url = normalize_url(&url)?;
    let name = if name.trim().is_empty() { url_default_name(&url) } else { name.trim().to_string() };
    let mut it = plugin_item("");
    it.kind = "url".into();
    it.name = name;
    it.target = url.clone();
    let id = it.id.clone();
    push(&app, vec![it])?;
    // 图标在后台取，取到后刷新首页
    tauri::async_runtime::spawn_blocking(move || fetch_site_icon(&app, &id, &url));
    Ok(())
}

/// 取网站图标，存到 icons/ 并更新这一项。返回有没有取到。
fn fetch_site_icon(app: &AppHandle, id: &str, url: &str) -> bool {
    let Some((bytes, ext)) = siteicon::fetch(url) else { return false };
    let s = state(app);
    let mut items = s.items.lock().unwrap();
    // 取图标期间网址可能被改掉或删掉了
    let Some(it) = items.iter_mut().find(|i| i.id == id && i.target == url) else { return false };
    let file = format!("site-{}.{ext}", new_id());
    let _ = std::fs::create_dir_all(s.icons_dir());
    if std::fs::write(s.icons_dir().join(&file), bytes).is_err() {
        return false;
    }
    it.auto_icon = Some(file);
    let ok = s.save(&mut items).is_ok();
    drop(items);
    changed(app);
    ok
}

/// 编辑框里的"重新获取网站图标"
#[tauri::command]
pub async fn home_refetch_icon(app: AppHandle, id: String) -> Result<(), String> {
    let url = state(&app).items.lock().unwrap().iter().find(|i| i.id == id && i.kind == "url").map(|i| i.target.clone());
    let url = url.ok_or_else(|| tr("找不到这个图标", "Item not found"))?;
    let ok = tauri::async_runtime::spawn_blocking(move || fetch_site_icon(&app, &id, &url)).await.unwrap_or(false);
    if ok {
        Ok(())
    } else {
        Err(tr("没取到这个网站的图标（可能打不开或者没有图标）", "Couldn't get an icon from this website"))
    }
}

fn file_item(s: &Home, path: &Path) -> Item {
    let mut it = plugin_item("");
    it.kind = "file".into();
    it.name = platform::display_name(path);
    it.target = path.display().to_string();
    it.auto_icon = extract_icon(s, path);
    it
}

/// 提取程序图标到 icons/，失败返回 None（显示首字母）
fn extract_icon(s: &Home, path: &Path) -> Option<String> {
    let _ = std::fs::create_dir_all(s.icons_dir());
    let file = format!("auto-{}.png", new_id());
    platform::extract_icon(path, &s.icons_dir().join(&file)).then_some(file)
}

fn push(app: &AppHandle, new: Vec<Item>) -> Result<(), String> {
    let s = state(app);
    let mut items = s.items.lock().unwrap();
    items.extend(new);
    s.save(&mut items)?;
    drop(items);
    changed(app);
    Ok(())
}

/// 拖到首页的文件（可以多个）
#[tauri::command]
pub async fn home_add_paths(app: AppHandle, paths: Vec<String>) -> Result<(), String> {
    let s = state(&app);
    let new = paths.iter().map(|p| file_item(s, Path::new(p))).collect();
    push(&app, new)
}

/// "添加本地程序或文件"：弹出选择框（可多选）
#[tauri::command]
pub async fn home_pick_files(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt;
    let Some(picked) = app.dialog().file().blocking_pick_files() else { return Ok(()) };
    let s = state(&app);
    let new = picked.into_iter().filter_map(|f| f.into_path().ok()).map(|p| file_item(s, &p)).collect();
    push(&app, new)
}

#[tauri::command]
pub async fn home_add_plugin(app: AppHandle, id: String) -> Result<(), String> {
    on_plugin_installed(&app, &id);
    Ok(())
}

/// 编辑框里能改的内容
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemEdit {
    id: String,
    name: String,
    target: String,
    args: String,
    icon: Icon,
    hide_console: bool,
    run_as_admin: bool,
}

#[tauri::command]
pub async fn home_update(app: AppHandle, edit: ItemEdit) -> Result<(), String> {
    let s = state(&app);
    let old = s.items.lock().unwrap().iter().find(|i| i.id == edit.id).cloned().ok_or_else(|| tr("找不到这个图标", "Item not found"))?;
    let target = match old.kind.as_str() {
        "url" => normalize_url(&edit.target)?,
        "file" if edit.target.trim().is_empty() => return Err(tr("请选择位置", "Please choose a location")),
        "file" => edit.target.trim().to_string(),
        _ => old.target.clone(),
    };
    // 换了位置就重新提取图标（在锁外面做，提取可能要一会儿）
    let auto_icon = if old.kind == "file" && target != old.target {
        extract_icon(s, Path::new(&target))
    } else if old.kind == "url" && target != old.target {
        None
    } else {
        old.auto_icon.clone()
    };
    let refetch = (old.kind == "url" && target != old.target).then(|| target.clone());
    let mut items = s.items.lock().unwrap();
    let it = items.iter_mut().find(|i| i.id == edit.id).ok_or_else(|| tr("找不到这个图标", "Item not found"))?;
    it.name = edit.name.trim().to_string();
    if it.kind != "plugin" && it.name.is_empty() {
        it.name = if it.kind == "url" { url_default_name(&target) } else { platform::display_name(Path::new(&target)) };
    }
    it.target = target;
    it.auto_icon = auto_icon;
    it.icon = edit.icon;
    if it.kind == "file" {
        it.args = edit.args;
        it.hide_console = edit.hide_console;
        it.run_as_admin = edit.run_as_admin && cfg!(windows);
    }
    s.save(&mut items)?;
    drop(items);
    changed(&app);
    if let Some(url) = refetch {
        let id = edit.id;
        tauri::async_runtime::spawn_blocking(move || fetch_site_icon(&app, &id, &url));
    }
    Ok(())
}

#[tauri::command]
pub async fn home_remove(app: AppHandle, id: String) -> Result<(), String> {
    let s = state(&app);
    let mut items = s.items.lock().unwrap();
    items.retain(|i| i.id != id);
    s.save(&mut items)?;
    drop(items);
    changed(&app);
    Ok(())
}

/// 固定顺序下拖动后的新顺序
#[tauri::command]
pub async fn home_reorder(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    let s = state(&app);
    let mut items = s.items.lock().unwrap();
    items.sort_by_key(|i| ids.iter().position(|x| *x == i.id).unwrap_or(usize::MAX));
    s.save(&mut items)?;
    drop(items);
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn home_launch(app: AppHandle, id: String) -> Result<(), String> {
    let s = state(&app);
    let it = s.items.lock().unwrap().iter().find(|i| i.id == id).cloned().ok_or_else(|| tr("找不到这个图标", "Item not found"))?;
    match it.kind.as_str() {
        "plugin" => plugins::open_ui(&app, &it.target)?,
        "url" => {
            use tauri_plugin_opener::OpenerExt;
            app.opener().open_url(&it.target, None::<&str>).map_err(|e| tr(&format!("打开网址失败：{e}"), &format!("Failed to open URL: {e}")))?
        }
        _ => {
            let path = PathBuf::from(&it.target);
            if !path.exists() {
                return Err(tr("找不到文件，可以右键编辑重新选择位置", "File not found. Right-click → Edit to choose it again"));
            }
            let (args, hide, admin) = (it.args.clone(), it.hide_console, it.run_as_admin);
            tauri::async_runtime::spawn_blocking(move || platform::launch(&path, &args, hide, admin))
                .await
                .map_err(|e| e.to_string())??;
        }
    }
    let mut items = s.items.lock().unwrap();
    if let Some(x) = items.iter_mut().find(|i| i.id == id) {
        x.last_used = now_ms();
    }
    let _ = s.save(&mut items);
    drop(items);
    changed(&app);
    Ok(())
}

/// 编辑框里"浏览…"：重新选择位置
#[tauri::command]
pub async fn home_pick_target(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog().file().blocking_pick_file()?.into_path().ok().map(|p| p.display().to_string())
}

#[derive(Serialize)]
pub struct PickedImage {
    file: String,
    url: String,
}

const MAX_ICON_IMAGE: u64 = 5 * 1024 * 1024;

/// 编辑框里"自定义图片"：复制到 icons/，保存后才会被引用（没保存的在下次保存时清理）
#[tauri::command]
pub async fn home_pick_image(app: AppHandle) -> Result<Option<PickedImage>, String> {
    use tauri_plugin_dialog::DialogExt;
    let Some(picked) = app.dialog().file().add_filter(tr("图片", "Images"), &["png", "jpg", "jpeg", "webp", "ico", "svg"]).blocking_pick_file() else {
        return Ok(None);
    };
    let src = picked.into_path().map_err(|e| e.to_string())?;
    let size = std::fs::metadata(&src).map_err(|e| e.to_string())?.len();
    if size > MAX_ICON_IMAGE {
        return Err(tr("图片太大了（最大 5 MB）", "Image is too large (max 5 MB)"));
    }
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png").to_lowercase();
    let s = state(&app);
    let _ = std::fs::create_dir_all(s.icons_dir());
    let file = format!("custom-{}.{ext}", new_id());
    std::fs::copy(&src, s.icons_dir().join(&file)).map_err(|e| tr(&format!("复制图片失败：{e}"), &format!("Failed to copy image: {e}")))?;
    let url = appfile_url(&format!("icons/{file}"));
    Ok(Some(PickedImage { file, url }))
}
