use std::path::Path;

use serde::{Deserialize, Serialize};

/// 壳的设置，保存在数据目录的 settings.json。不包含任何密钥。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "zh-CN" 或 "en"
    pub language: String,
    /// "light"、"dark" 或 "system"
    pub theme: String,
    /// 界面风格："standard" 或 "glass"
    pub ui_style: String,
    /// 壁纸文件名（在数据目录的 wallpaper/ 里），None = 不用壁纸
    pub wallpaper: Option<String>,
    /// "cover"（铺满）或 "contain"（完整显示）
    pub wallpaper_fit: String,
    /// 模糊 0–40（像素）
    pub wallpaper_blur: u32,
    /// 变暗 0–80（百分比）
    pub wallpaper_dim: u32,
    /// 首页排序："fixed" 或 "recent"
    pub home_sort: String,
    /// 开机自启。真实状态以系统为准，启动时会用系统里的状态覆盖。
    pub autostart: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "zh-CN".into(),
            theme: "system".into(),
            ui_style: "standard".into(),
            wallpaper: None,
            wallpaper_fit: "cover".into(),
            wallpaper_blur: 0,
            wallpaper_dim: 10,
            home_sort: "fixed".into(),
            autostart: false,
        }
    }
}

impl Settings {
    /// 文件不存在或格式错误时使用默认值（格式错误会在日志里提示）。
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("settings.json");
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                eprintln!("settings.json 格式错误，已使用默认设置：{e}");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(self).expect("设置序列化不会失败");
        std::fs::write(dir.join("settings.json"), text)
    }
}
