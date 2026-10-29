#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod backend;
mod home;
mod manifest;
mod platform;
mod plugins;
mod settings;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use settings::Settings;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, RunEvent, WindowEvent, Wry,
};

pub struct AppState {
    pub settings: Mutex<Settings>,
    tray: TrayIcon,
}

static LANG_EN: AtomicBool = AtomicBool::new(false);

/// Rust 端给用户看的文字：按当前界面语言二选一
pub fn tr(zh: &str, en: &str) -> String {
    if LANG_EN.load(Ordering::Relaxed) { en } else { zh }.to_string()
}

pub fn platform_name() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        _ => "linux",
    }
}

/// 把 "system" 换成系统当前的浅色 / 深色
pub fn resolved_theme(app: &AppHandle, setting: &str) -> &'static str {
    match setting {
        "dark" => "dark",
        "light" => "light",
        _ => match app.get_webview_window("main").and_then(|w| w.theme().ok()) {
            Some(tauri::Theme::Dark) => "dark",
            _ => "light",
        },
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
    platform: &'static str,
    data_dir: String,
    /// 系统透明效果能不能用（Win11 / macOS）
    glass_supported: bool,
}

#[tauri::command]
fn app_info(app: AppHandle) -> Result<AppInfo, String> {
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        platform: platform_name(),
        data_dir: dir.display().to_string(),
        glass_supported: platform::glass_supported(),
    })
}

#[tauri::command]
fn open_data_dir(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    app.opener().open_path(dir.display().to_string(), None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(app: AppHandle, state: tauri::State<AppState>, settings: Settings) -> Result<(), String> {
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    let old_autostart = state.settings.lock().unwrap().autostart;
    if settings.autostart != old_autostart {
        use tauri_plugin_autostart::ManagerExt;
        let al = app.autolaunch();
        let r = if settings.autostart { al.enable() } else { al.disable() };
        r.map_err(|e| tr(&format!("设置开机自启失败：{e}"), &format!("Failed to change start-up setting: {e}")))?;
    }
    settings.save(&dir).map_err(|e| tr(&format!("保存设置失败：{e}"), &format!("Failed to save settings: {e}")))?;
    LANG_EN.store(settings.language == "en", Ordering::Relaxed);
    let menu = tray_menu(&app, &settings.language).map_err(|e| e.to_string())?;
    state.tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    plugins::broadcast_theme(&app, &settings.theme);
    apply_window_style(&app, &settings);
    clean_wallpapers(&dir, settings.wallpaper.as_deref());
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

/// 主窗口的系统主题和透明效果。毛玻璃 + 没有壁纸 + 系统支持时才打开 Mica / vibrancy。
fn apply_window_style(app: &AppHandle, s: &Settings) {
    let Some(w) = app.get_webview_window("main") else { return };
    let _ = w.set_theme(match s.theme.as_str() {
        "dark" => Some(tauri::Theme::Dark),
        "light" => Some(tauri::Theme::Light),
        _ => None,
    });
    if s.ui_style == "glass" && s.wallpaper.is_none() && platform::glass_supported() {
        use tauri::window::{Effect, EffectState, EffectsBuilder};
        let effect = if cfg!(target_os = "macos") { Effect::UnderWindowBackground } else { Effect::Mica };
        let _ = w.set_effects(EffectsBuilder::new().effect(effect).state(EffectState::FollowsWindowActiveState).build());
    } else {
        let _ = w.set_effects(None);
    }
}

/// 删除 wallpaper/ 里不再使用的旧壁纸
fn clean_wallpapers(dir: &std::path::Path, keep: Option<&str>) {
    let Ok(rd) = std::fs::read_dir(dir.join("wallpaper")) else { return };
    for e in rd.flatten() {
        if Some(e.file_name().to_string_lossy().as_ref()) != keep {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

const MAX_WALLPAPER: u64 = 30 * 1024 * 1024;

/// 选择壁纸图片，复制到数据目录的 wallpaper/，返回文件名（设置保存后才生效）
#[tauri::command]
async fn wallpaper_pick(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let Some(picked) = app.dialog().file().add_filter(tr("图片", "Images"), &["jpg", "jpeg", "png", "webp"]).blocking_pick_file() else {
        return Ok(None);
    };
    let src = picked.into_path().map_err(|e| e.to_string())?;
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if !matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp") {
        return Err(tr("只支持 jpg、png、webp 图片", "Only jpg, png and webp images are supported"));
    }
    if std::fs::metadata(&src).map_err(|e| e.to_string())?.len() > MAX_WALLPAPER {
        return Err(tr("图片太大了（最大 30 MB）", "Image is too large (max 30 MB)"));
    }
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("wallpaper");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let name = format!("wallpaper-{ms}.{ext}");
    std::fs::copy(&src, dir.join(&name)).map_err(|e| tr(&format!("复制图片失败：{e}"), &format!("Failed to copy image: {e}")))?;
    Ok(Some(name))
}

/// appfile:// 协议：只给主窗口读数据目录里的 icons/ 和 wallpaper/
fn serve_appfile(app: &AppHandle, caller: &str, req: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<std::borrow::Cow<'static, [u8]>> {
    let resp = |status: u16, mime: &str, body: Vec<u8>| {
        tauri::http::Response::builder().status(status).header("Content-Type", mime).body(std::borrow::Cow::Owned(body)).unwrap()
    };
    let path = req.uri().path().trim_start_matches('/').to_string();
    let (sub, name) = path.split_once('/').unwrap_or(("", ""));
    let safe = caller == "main" && matches!(sub, "icons" | "wallpaper") && !name.is_empty() && !name.contains(['/', '\\']) && !name.contains("..");
    if !safe {
        return resp(403, "text/plain", b"forbidden".to_vec());
    }
    let Ok(dir) = app.path().app_local_data_dir() else { return resp(500, "text/plain", vec![]) };
    let mime = match name.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    };
    match std::fs::read(dir.join(sub).join(name)) {
        Ok(bytes) => resp(200, mime, bytes),
        Err(_) => resp(404, "text/plain", b"not found".to_vec()),
    }
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn tray_menu(app: &AppHandle, lang: &str) -> tauri::Result<Menu<Wry>> {
    let (open, quit) = match lang {
        "en" => ("Open Slacking Desk", "Quit"),
        _ => ("打开摸鱼工作台", "退出"),
    };
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", open, true, None::<&str>)?,
            &MenuItem::with_id(app, "quit", quit, true, None::<&str>)?,
        ],
    )
}

fn main() {
    let app = tauri::Builder::default()
        // 必须最先注册：第二次启动时只唤起已有窗口
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| show_main(app)))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        // 开机自启时带 --autostart，启动后只留在托盘
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--autostart"])))
        .register_uri_scheme_protocol("plugin", |ctx, req| plugins::serve(ctx.app_handle(), ctx.webview_label(), &req))
        .register_uri_scheme_protocol("appfile", |ctx, req| serve_appfile(ctx.app_handle(), ctx.webview_label(), &req))
        .setup(|app| {
            let handle = app.handle().clone();
            let dir = handle.path().app_local_data_dir()?;
            for sub in ["plugins", "plugin-data", "logs/plugins", "cache", "icons", "wallpaper"] {
                std::fs::create_dir_all(dir.join(sub))?;
            }
            let mut settings = Settings::load(&dir);
            {
                // 开机自启以系统里的真实状态为准（用户可能在系统设置里关掉了）
                use tauri_plugin_autostart::ManagerExt;
                if let Ok(on) = handle.autolaunch().is_enabled() {
                    settings.autostart = on;
                }
            }
            LANG_EN.store(settings.language == "en", Ordering::Relaxed);

            let tray = TrayIconBuilder::with_id("main")
                .icon(handle.default_window_icon().cloned().expect("缺少应用图标"))
                .tooltip("Slacking Desk")
                .menu(&tray_menu(&handle, &settings.language)?)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;

            apply_window_style(&handle, &settings);
            app.manage(AppState { settings: Mutex::new(settings), tray });
            let ps = plugins::init(dir.clone(), handle.package_info().version.to_string());
            let (ui_plugins, installed) = ps.home_seed();
            app.manage(ps);
            app.manage(home::init(dir, &ui_plugins, &installed));
            plugins::start_all(&handle);
            // 主窗口默认隐藏（配置里 visible: false），不是开机自启才显示
            if !std::env::args().any(|a| a == "--autostart") {
                show_main(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // 关闭主窗口 = 收到托盘，真正退出走托盘菜单
            WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::Destroyed => plugins::on_window_destroyed(window.app_handle(), window.label()),
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            open_data_dir,
            get_settings,
            save_settings,
            plugins::plugins_list,
            plugins::plugin_pick_folder,
            plugins::plugin_add,
            plugins::plugin_set_enabled,
            plugins::plugin_reload,
            plugins::plugin_restart,
            plugins::plugin_remove,
            plugins::plugin_open,
            plugins::plugin_open_log,
            plugins::plugin_devtools,
            plugins::plugin_call,
            plugins::plugin_heartbeat,
            wallpaper_pick,
            home::home_list,
            home::home_add_url,
            home::home_add_paths,
            home::home_pick_files,
            home::home_add_plugin,
            home::home_update,
            home::home_remove,
            home::home_reorder,
            home::home_launch,
            home::home_pick_target,
            home::home_pick_image,
        ])
        .build(tauri::generate_context!())
        .expect("启动失败");

    app.run(|app, event| match event {
        RunEvent::Exit => plugins::kill_all(app),
        // macOS：点 Dock 图标时重新显示主窗口
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => show_main(app),
        _ => {}
    });
}
