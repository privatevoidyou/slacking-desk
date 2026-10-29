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
}

impl Default for Settings {
    fn default() -> Self {
        Self { language: "zh-CN".into(), theme: "system".into() }
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
