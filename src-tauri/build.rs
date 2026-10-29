fn main() {
    // 列出所有命令后，每个命令都要在 capabilities 里显式授权：
    // 主窗口拿到管理命令，插件窗口只拿到 plugin_call 和 plugin_heartbeat。
    let commands = &[
        "app_info", "open_data_dir", "get_settings", "save_settings",
        "plugins_list", "plugin_pick_folder", "plugin_add", "plugin_set_enabled", "plugin_reload",
        "plugin_restart", "plugin_remove", "plugin_open", "plugin_open_log", "plugin_devtools",
        "plugin_call", "plugin_heartbeat",
        "wallpaper_pick", "home_list", "home_add_url", "home_add_paths", "home_pick_files", "home_add_plugin", "home_update", "home_remove", "home_reorder", "home_launch", "home_pick_target", "home_pick_image", "home_refetch_icon",
    ];
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(commands)))
        .expect("tauri-build 失败");
}
