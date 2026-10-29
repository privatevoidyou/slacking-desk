//! 插件 API 分发：先认出是哪个插件、查权限，再执行。
//! Web 插件经宿主页 → plugin_call 命令进来；进程插件经 JSON-RPC 进来，走同一个入口。

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::plugins;
use crate::tr;

#[derive(Clone, Copy, PartialEq)]
pub enum Caller {
    Web,
    Backend,
}

/// 错误码只有 6 个，见插件文档
#[derive(Serialize, Debug)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn new(code: &'static str, message: String) -> Self {
        Self { code, message }
    }
    pub fn internal(message: String) -> Self {
        Self::new("internal", message)
    }
    pub fn invalid(message: String) -> Self {
        Self::new("invalid_params", message)
    }
}

/// 把插件返回的错误码限制在已知范围内
pub fn known_code(code: &str) -> &'static str {
    match code {
        "permission_denied" => "permission_denied",
        "unsupported_platform" => "unsupported_platform",
        "invalid_params" => "invalid_params",
        "not_found" => "not_found",
        "timeout" => "timeout",
        _ => "internal",
    }
}

pub async fn dispatch(app: &AppHandle, id: &str, caller: Caller, method: &str, params: Value) -> Result<Value, ApiError> {
    let need: Option<&str> = match method {
        "app.info" | "storage.get" | "storage.set" | "storage.remove" => None,
        "backend.call" if caller == Caller::Web => None,
        "notify.show" => Some("notify"),
        _ => return Err(ApiError::new("not_found", tr(&format!("没有这个 API：{method}"), &format!("No such API: {method}")))),
    };
    let info = plugins::running_info(app, id).ok_or_else(|| ApiError::internal(tr("插件未启用", "Plugin is not enabled")))?;
    if let Some(p) = need {
        if !info.grants.iter().any(|g| g == p) {
            return Err(ApiError::new(
                "permission_denied",
                tr(&format!("插件没有申请「{p}」权限"), &format!("Plugin has not been granted \"{p}\"")),
            ));
        }
    }

    match method {
        "app.info" => {
            let settings = app.state::<crate::AppState>().settings.lock().unwrap().clone();
            Ok(json!({
                "shellVersion": app.package_info().version.to_string(),
                "platform": crate::platform_name(),
                "language": settings.language,
                "theme": crate::resolved_theme(app, &settings.theme),
                "pluginId": id,
            }))
        }
        "storage.get" | "storage.set" | "storage.remove" => storage(&info.data_dir, method, &params),
        "notify.show" => {
            let title = str_param(&params, "title")?;
            let body = params.get("body").and_then(|b| b.as_str()).unwrap_or("");
            app.notification()
                .builder()
                .title(title)
                .body(body)
                .show()
                .map_err(|e| ApiError::internal(tr(&format!("发送通知失败：{e}"), &format!("Failed to show notification: {e}"))))?;
            Ok(Value::Null)
        }
        "backend.call" => {
            let m = str_param(&params, "method")?;
            let p = params.get("params").cloned().unwrap_or(Value::Null);
            let backend = plugins::ensure_backend(app, id).map_err(ApiError::internal)?;
            backend.request(m, p).await
        }
        _ => unreachable!("上面已经过滤了未知方法"),
    }
}

fn str_param<'a>(params: &'a Value, key: &str) -> Result<&'a str, ApiError> {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::invalid(tr(&format!("缺少字符串参数 {key}"), &format!("Missing string parameter {key}"))))
}

const STORAGE_LIMIT: usize = 1024 * 1024;
static STORAGE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 小型键值存储：plugin-data/<id>/storage.json
fn storage(data_dir: &std::path::Path, method: &str, params: &Value) -> Result<Value, ApiError> {
    let key = str_param(params, "key")?;
    if key.is_empty() || key.len() > 200 {
        return Err(ApiError::invalid(tr("key 长度要在 1–200 之间", "key must be 1–200 characters")));
    }
    let _lock = STORAGE_LOCK.lock().unwrap();
    let path = data_dir.join("storage.json");
    let io_err = |e: std::io::Error| ApiError::internal(tr(&format!("读写存储失败：{e}"), &format!("Storage IO failed: {e}")));
    let mut map: serde_json::Map<String, Value> = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => return Err(io_err(e)),
    };
    match method {
        "storage.get" => return Ok(map.get(key).cloned().unwrap_or(Value::Null)),
        "storage.set" => {
            map.insert(key.to_string(), params.get("value").cloned().unwrap_or(Value::Null));
        }
        _ => {
            map.remove(key);
        }
    }
    let text = serde_json::to_string(&map).expect("JSON 序列化不会失败");
    if text.len() > STORAGE_LIMIT {
        return Err(ApiError::invalid(tr("storage 只适合存小数据，总大小上限 1MB", "storage is for small data (1 MB total)")));
    }
    std::fs::create_dir_all(data_dir).map_err(io_err)?;
    std::fs::write(&path, text).map_err(io_err)?;
    Ok(Value::Null)
}
