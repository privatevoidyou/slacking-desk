#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod settings;

use std::sync::Mutex;

use settings::Settings;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WindowEvent, Wry,
};

struct AppState {
    settings: Mutex<Settings>,
    tray: TrayIcon,
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
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        platform: std::env::consts::OS,
        data_dir: dir.display().to_string(),
    })
}

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(app: AppHandle, state: tauri::State<AppState>, settings: Settings) -> Result<(), String> {
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    settings.save(&dir).map_err(|e| format!("保存设置失败：{e}"))?;
    let menu = tray_menu(&app, &settings.language).map_err(|e| e.to_string())?;
    state.tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
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
        .setup(|app| {
            let handle = app.handle().clone();
            let dir = handle.path().app_local_data_dir()?;
            for sub in ["plugins", "plugin-data", "logs", "cache"] {
                std::fs::create_dir_all(dir.join(sub))?;
            }
            let settings = Settings::load(&dir);

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
            Ok(())
        })
        .on_window_event(|window, event| {
            // 关闭主窗口 = 收到托盘，真正退出走托盘菜单
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![app_info, get_settings, save_settings])
        .build(tauri::generate_context!())
        .expect("启动失败");

    app.run(|_app, _event| {
        // macOS：点 Dock 图标时重新显示主窗口
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = _event {
            show_main(_app);
        }
    });
}
