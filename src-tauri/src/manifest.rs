//! plugin.json 的读取和校验。规范见 market 仓库 docs/plugin-guide.md。

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tr;

/// 壳认识的全部权限。新增权限时同步更新前端的权限说明文字和插件文档。
pub const PERMISSIONS: &[&str] = &[
    "notify", "hotkey", "windows.read", "windows.control", "wallpaper", "network", "llm", "files", "input",
];

const KNOWN_FIELDS: &[&str] = &[
    "manifestVersion", "id", "name", "version", "author", "description", "icon", "minShellVersion",
    "ui", "background", "backend", "permissions", "i18n",
];

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub manifest_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    pub icon: Option<String>,
    pub min_shell_version: String,
    pub ui: Option<UiSpec>,
    pub background: Option<String>,
    pub backend: Option<BackendSpec>,
    #[serde(default)]
    pub permissions: Vec<String>,
    /// 其他语言的名称和简介，目前只用 "en"
    #[serde(default)]
    pub i18n: HashMap<String, I18nText>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UiSpec {
    pub page: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BackendSpec {
    /// "python"、"node" 或 "binary"
    pub runtime: String,
    pub entry: Entry,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Entry {
    Single(String),
    /// 只有 binary 用：{"windows-x64": "bin/a.exe", "macos-arm64": "bin/a"}
    PerPlatform(HashMap<String, String>),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct I18nText {
    pub name: Option<String>,
    pub description: Option<String>,
}

/// 读取并校验插件文件夹。Err 里是给用户看的错误列表；Ok 时附带警告。
pub fn load(dir: &Path, shell_version: &str) -> Result<(Manifest, Vec<String>), Vec<String>> {
    let path = dir.join("plugin.json");
    let text = std::fs::read_to_string(&path).map_err(|e| {
        vec![tr(&format!("读不到 {}：{e}", path.display()), &format!("Cannot read {}: {e}", path.display()))]
    })?;
    let raw: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| vec![tr(&format!("plugin.json 不是合法的 JSON：{e}"), &format!("plugin.json is not valid JSON: {e}"))])?;

    let mut warnings = Vec::new();
    if let Some(obj) = raw.as_object() {
        for key in obj.keys().filter(|k| !KNOWN_FIELDS.contains(&k.as_str())) {
            warnings.push(tr(&format!("不认识的字段 \"{key}\"，已忽略"), &format!("Unknown field \"{key}\" ignored")));
        }
    }
    // 先拦住更高版本的清单，否则会被当成普通的字段错误
    if let Some(v) = raw.get("manifestVersion").and_then(|v| v.as_u64()) {
        if v > 1 {
            return Err(vec![tr(
                "这个插件需要更新版本的摸鱼工作台（manifestVersion 高于 1）",
                "This plugin needs a newer Slacking Desk (manifestVersion > 1)",
            )]);
        }
    }
    let m: Manifest = serde_json::from_value(raw)
        .map_err(|e| vec![tr(&format!("plugin.json 字段有误：{e}"), &format!("Invalid plugin.json: {e}"))])?;

    let mut errors = Vec::new();
    let mut err = |zh: String, en: String| errors.push(tr(&zh, &en));

    if m.manifest_version != 1 {
        err("manifestVersion 必须是 1".into(), "manifestVersion must be 1".into());
    }
    if !valid_id(&m.id) {
        err(
            format!("id \"{}\" 不合法：只能用小写字母、数字、. 和 -，长度 3–64，首尾不能是符号", m.id),
            format!("Invalid id \"{}\": use 3–64 lowercase letters, digits, '.' or '-'", m.id),
        );
    }
    if m.name.trim().is_empty() {
        err("name 不能为空".into(), "name must not be empty".into());
    }
    if parse_version(&m.version).is_none() {
        err(format!("version \"{}\" 不是 x.y.z 格式", m.version), format!("version \"{}\" is not x.y.z", m.version));
    }
    match (parse_version(&m.min_shell_version), parse_version(shell_version)) {
        (None, _) => err(
            format!("minShellVersion \"{}\" 不是 x.y.z 格式", m.min_shell_version),
            format!("minShellVersion \"{}\" is not x.y.z", m.min_shell_version),
        ),
        (Some(need), Some(have)) if need > have => err(
            format!("需要摸鱼工作台 {} 或更高版本（当前 {shell_version}）", m.min_shell_version),
            format!("Requires Slacking Desk {} or later (current {shell_version})", m.min_shell_version),
        ),
        _ => {}
    }
    if m.ui.is_none() && m.background.is_none() {
        err("ui 和 background 至少要有一个".into(), "Either ui or background is required".into());
    }

    let check_file = |field: &str, rel: &str, errors: &mut Vec<String>| {
        if let Err(e) = resolve(dir, rel) {
            errors.push(tr(&format!("{field}：{e}"), &format!("{field}: {e}")));
        }
    };
    if let Some(ui) = &m.ui {
        check_file("ui.page", &ui.page, &mut errors);
        for (name, v) in [("ui.width", ui.width), ("ui.height", ui.height)] {
            if let Some(v) = v {
                if !(200..=4000).contains(&v) {
                    errors.push(tr(&format!("{name} 要在 200–4000 之间"), &format!("{name} must be 200–4000")));
                }
            }
        }
    }
    if let Some(bg) = &m.background {
        check_file("background", bg, &mut errors);
    }
    if let Some(icon) = &m.icon {
        check_file("icon", icon, &mut errors);
        let lower = icon.to_ascii_lowercase();
        if !(lower.ends_with(".png") || lower.ends_with(".svg")) {
            errors.push(tr("icon 只支持 PNG 或 SVG", "icon must be PNG or SVG"));
        }
    }
    if let Some(b) = &m.backend {
        match (b.runtime.as_str(), &b.entry) {
            ("python" | "node", Entry::Single(e)) => check_file("backend.entry", e, &mut errors),
            ("binary", Entry::Single(e)) => check_file("backend.entry", e, &mut errors),
            ("binary", Entry::PerPlatform(map)) => match map.get(&crate::platform::platform_key()) {
                Some(e) => check_file("backend.entry", e, &mut errors),
                None => errors.push(tr(
                    &format!("backend.entry 没有提供当前平台（{}）的程序", crate::platform::platform_key()),
                    &format!("backend.entry has no program for this platform ({})", crate::platform::platform_key()),
                )),
            },
            ("python" | "node", Entry::PerPlatform(_)) => {
                errors.push(tr("python/node 的 backend.entry 只能是一个文件路径", "backend.entry must be a single path for python/node"))
            }
            (other, _) => errors.push(tr(
                &format!("backend.runtime \"{other}\" 不支持，只能是 python、node 或 binary"),
                &format!("Unsupported backend.runtime \"{other}\" (python, node or binary)"),
            )),
        }
    }
    let mut seen = Vec::new();
    for p in &m.permissions {
        if !PERMISSIONS.contains(&p.as_str()) {
            errors.push(tr(&format!("不认识的权限 \"{p}\""), &format!("Unknown permission \"{p}\"")));
        } else if seen.contains(&p) {
            errors.push(tr(&format!("权限 \"{p}\" 重复"), &format!("Duplicate permission \"{p}\"")));
        }
        seen.push(p);
    }

    if errors.is_empty() {
        Ok((m, warnings))
    } else {
        Err(errors)
    }
}

/// 把清单里的相对路径解析成插件目录里的真实文件。
/// 拒绝绝对路径、`..`，以及通过符号链接指到插件目录外面的文件。
pub fn resolve(dir: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel_path = Path::new(rel);
    if rel.is_empty() || rel_path.components().any(|c| !matches!(c, Component::Normal(_) | Component::CurDir)) {
        return Err(tr(&format!("路径 \"{rel}\" 必须是插件目录里的相对路径"), &format!("Path \"{rel}\" must be relative to the plugin folder")));
    }
    let base = dir.canonicalize().map_err(|e| e.to_string())?;
    let full = base
        .join(rel_path)
        .canonicalize()
        .map_err(|_| tr(&format!("找不到文件 \"{rel}\""), &format!("File not found: \"{rel}\"")))?;
    if !full.starts_with(&base) || !full.is_file() {
        return Err(tr(&format!("\"{rel}\" 不是插件目录里的文件"), &format!("\"{rel}\" is not a file inside the plugin folder")));
    }
    Ok(full)
}

fn valid_id(id: &str) -> bool {
    let ok_char = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-';
    let edge = |c: Option<char>| c.is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    (3..=64).contains(&id.len()) && id.chars().all(ok_char) && edge(id.chars().next()) && edge(id.chars().last())
}

/// 只认 x.y.z（忽略 -beta 之类的后缀）
pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let r = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(r)
}
