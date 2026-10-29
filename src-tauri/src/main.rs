#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod backend;
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
}

#[tauri::command]
fn app_info(app: AppHandle) -> Result<AppInfo, String> {
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    Ok(AppInfo { version: app.package_info().version.to_string(), platform: platform_name(), data_dir: dir.display().to_string() })
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
    settings.save(&dir).map_err(|e| tr(&format!("保存设置失败：{e}"), &format!("Failed to save settings: {e}")))?;
    LANG_EN.store(settings.language == "en", Ordering::Relaxed);
    let menu = tray_menu(&app, &settings.language).map_err(|e| e.to_string())?;
    state.tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    plugins::broadcast_theme(&app, &settings.theme);
    *state.settings.lock().unwrap() = settings;
    Ok(())
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
        .register_uri_scheme_protocol("plugin", |ctx, req| plugins::serve(ctx.app_handle(), ctx.webview_label(), &req))
        .setup(|app| {
            let handle = app.handle().clone();
            let dir = handle.path().app_local_data_dir()?;
            for sub in ["plugins", "plugin-data", "logs/plugins", "cache"] {
                std::fs::create_dir_all(dir.join(sub))?;
            }
            let settings = Settings::load(&dir);
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

            app.manage(AppState { settings: Mutex::new(settings), tray });
            app.manage(plugins::init(dir, handle.package_info().version.to_string()));
            plugins::start_all(&handle);
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
