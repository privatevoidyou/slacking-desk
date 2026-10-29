//! 进程插件宿主：子进程 + stdio 上的 JSON-RPC 2.0（一行一条 JSON）。
//! stdout 只传协议消息；stderr 是日志，写进 logs/plugins/<id>.log。

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::AppHandle;
use tokio::sync::oneshot;

use crate::api::{self, ApiError, Caller};
use crate::manifest::{BackendSpec, Entry};
use crate::platform::{self, ProcessGuard};
use crate::tr;

const CALL_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_LINE: usize = 4 * 1024 * 1024;
const LOG_LIMIT: u64 = 1024 * 1024;
const TAIL_LINES: usize = 20;

pub struct Backend {
    tx: Mutex<Option<mpsc::Sender<String>>>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Result<Value, ApiError>>>>,
    next_id: AtomicU64,
    guard: ProcessGuard,
    stopping: AtomicBool,
    exited: AtomicBool,
    /// stderr 最后几行，崩溃时显示给用户
    pub tail: Mutex<VecDeque<String>>,
}

/// 后端进程退出时调用：(哪个后端, 是否意外退出, 退出码)
pub type OnExit = Box<dyn Fn(&Arc<Backend>, bool, String) + Send>;

pub struct SpawnArgs<'a> {
    pub app: AppHandle,
    pub id: &'a str,
    pub dir: &'a Path,
    pub spec: &'a BackendSpec,
    pub log_path: PathBuf,
    pub init: Value,
    pub on_exit: OnExit,
}

impl Backend {
    pub fn spawn(a: SpawnArgs) -> Result<Arc<Backend>, String> {
        let (program, args) = command_for(a.spec, a.dir)?;
        let log = Arc::new(Mutex::new(Logger::open(a.log_path)));
        log.lock().unwrap().line(&format!("[shell] 启动后端：{program} {}", args.join(" ")));

        let mut cmd = Command::new(&program);
        cmd.args(&args)
            .current_dir(a.dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("PYTHONUTF8", "1")
            .env("PYTHONUNBUFFERED", "1")
            .env("PYTHONIOENCODING", "utf-8");
        platform::prepare_command(&mut cmd);
        let mut child = cmd
            .spawn()
            .map_err(|e| tr(&format!("启动后端失败（{program}）：{e}"), &format!("Failed to start backend ({program}): {e}")))?;
        let guard = match ProcessGuard::new(&child) {
            Ok(g) => g,
            Err(e) => {
                let _ = child.kill();
                return Err(tr(&format!("无法管理后端进程：{e}"), &format!("Cannot manage backend process: {e}")));
            }
        };

        let mut stdin = child.stdin.take().expect("stdin 已设为 piped");
        let stdout = child.stdout.take().expect("stdout 已设为 piped");
        let stderr = child.stderr.take().expect("stderr 已设为 piped");
        let (tx, rx) = mpsc::channel::<String>();

        let backend = Arc::new(Backend {
            tx: Mutex::new(Some(tx)),
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            guard,
            stopping: AtomicBool::new(false),
            exited: AtomicBool::new(false),
            tail: Mutex::new(VecDeque::new()),
        });

        // 写线程：插件不读 stdin 时，阻塞的只是这个线程
        std::thread::spawn(move || {
            for line in rx {
                if stdin.write_all(line.as_bytes()).and_then(|_| stdin.write_all(b"\n")).and_then(|_| stdin.flush()).is_err() {
                    break;
                }
            }
            // rx 结束（stop 时）→ stdin 随之关闭，插件读到 EOF
        });

        // stderr → 日志
        {
            let log = log.clone();
            let b = backend.clone();
            std::thread::spawn(move || {
                let mut r = BufReader::new(stderr);
                let mut buf = Vec::new();
                while read_line_limited(&mut r, &mut buf) {
                    let line = String::from_utf8_lossy(&buf).trim_end().to_string();
                    log.lock().unwrap().line(&line);
                    let mut tail = b.tail.lock().unwrap();
                    if tail.len() == TAIL_LINES {
                        tail.pop_front();
                    }
                    tail.push_back(line);
                }
            });
        }

        // stdout → 协议消息；读到 EOF 说明进程退出了
        {
            let b = backend.clone();
            let app = a.app.clone();
            let id = a.id.to_string();
            let on_exit = a.on_exit;
            std::thread::spawn(move || {
                let mut r = BufReader::new(stdout);
                let mut buf = Vec::new();
                while read_line_limited(&mut r, &mut buf) {
                    let text = String::from_utf8_lossy(&buf);
                    let text = text.trim();
                    if text.is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<Value>(text) {
                        Ok(msg) if msg.is_object() => b.handle_message(&app, &id, msg),
                        _ => log.lock().unwrap().line(&format!("[shell] stdout 里出现了非协议内容（日志请写 stderr）：{}", truncate(text, 300))),
                    }
                }
                let code = child.wait().map(|s| s.code().map_or("signal".into(), |c| c.to_string())).unwrap_or_default();
                b.exited.store(true, Ordering::SeqCst);
                // 退出后结束整组进程，清理插件留下的子进程
                b.guard.kill();
                for (_, p) in b.pending.lock().unwrap().drain() {
                    let _ = p.send(Err(ApiError::internal(tr("插件后端已退出", "Plugin backend exited"))));
                }
                let unexpected = !b.stopping.load(Ordering::SeqCst);
                log.lock().unwrap().line(&format!("[shell] 后端退出，退出码 {code}"));
                on_exit(&b, unexpected, code);
            });
        }

        let _ = backend.send(json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize", "params": a.init }));
        Ok(backend)
    }

    fn send(&self, msg: Value) -> Result<(), ApiError> {
        let tx = self.tx.lock().unwrap();
        match tx.as_ref() {
            Some(tx) if tx.send(msg.to_string()).is_ok() => Ok(()),
            _ => Err(ApiError::internal(tr("插件后端已停止", "Plugin backend stopped"))),
        }
    }

    /// 壳 → 插件的调用，30 秒超时
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, ApiError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        if let Err(e) = self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })) {
            self.pending.lock().unwrap().remove(&id);
            return Err(e);
        }
        match tokio::time::timeout(CALL_TIMEOUT, rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err(ApiError::internal(tr("插件后端已退出", "Plugin backend exited"))),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(ApiError::new("timeout", tr(&format!("后端 30 秒内没有响应 {method}"), &format!("Backend did not answer {method} within 30 s"))))
            }
        }
    }

    fn handle_message(self: &Arc<Self>, app: &AppHandle, plugin_id: &str, msg: Value) {
        if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
            // 插件 → 壳的调用（没有 id 的是通知，不回复）
            let req_id = msg.get("id").cloned();
            let method = method.to_string();
            let params = msg.get("params").cloned().unwrap_or(Value::Null);
            let (b, app, plugin_id) = (self.clone(), app.clone(), plugin_id.to_string());
            tauri::async_runtime::spawn(async move {
                let result = api::dispatch(&app, &plugin_id, Caller::Backend, &method, params).await;
                if let Some(id) = req_id {
                    let reply = match result {
                        Ok(v) => json!({ "jsonrpc": "2.0", "id": id, "result": v }),
                        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": e.message, "data": { "code": e.code } } }),
                    };
                    let _ = b.send(reply);
                }
            });
            return;
        }
        // 插件对壳的调用的回复
        let Some(id) = msg.get("id").and_then(|i| i.as_u64()) else { return };
        if id == 0 {
            return; // initialize 的回复，不需要处理
        }
        let Some(p) = self.pending.lock().unwrap().remove(&id) else { return };
        let result = match msg.get("error") {
            Some(e) => Err(ApiError::new(
                api::known_code(e.pointer("/data/code").and_then(|c| c.as_str()).unwrap_or("internal")),
                e.get("message").and_then(|m| m.as_str()).unwrap_or("error").to_string(),
            )),
            None => Ok(msg.get("result").cloned().unwrap_or(Value::Null)),
        };
        let _ = p.send(result);
    }

    /// 正常关闭：发 shutdown → 关 stdin → 3 秒后还没退出就强制结束整组进程
    pub fn stop(self: &Arc<Self>) {
        if self.stopping.swap(true, Ordering::SeqCst) {
            return;
        }
        let _ = self.send(json!({ "jsonrpc": "2.0", "method": "shutdown" }));
        self.tx.lock().unwrap().take();
        let b = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(3));
            if !b.exited.load(Ordering::SeqCst) {
                b.guard.kill();
            }
        });
    }

    /// 壳退出时用，不等待
    pub fn kill_now(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        self.guard.kill();
    }
}

/// 选择启动命令。Python 和 Node 用系统里装好的（M1 阶段）。
fn command_for(spec: &BackendSpec, dir: &Path) -> Result<(String, Vec<String>), String> {
    let entry = match &spec.entry {
        Entry::Single(e) => e.clone(),
        Entry::PerPlatform(map) => map.get(&platform::platform_key()).cloned().unwrap_or_default(),
    };
    let entry_path = crate::manifest::resolve(dir, &entry)?.display().to_string();
    match spec.runtime.as_str() {
        "python" => {
            let (py, mut args) = find_python()?;
            args.push(entry_path);
            Ok((py, args))
        }
        "node" => {
            let node = platform::node_candidates()
                .into_iter()
                .find(|c| quiet(c).arg("--version").output().is_ok_and(|o| o.status.success()))
                .ok_or_else(|| tr("找不到 Node.js，请先安装 Node.js 并重启摸鱼工作台", "Node.js not found. Install Node.js and restart Slacking Desk"))?;
            Ok((node.to_string(), vec![entry_path]))
        }
        _ => Ok((entry_path, vec![])),
    }
}

static PYTHON: Mutex<Option<(String, Vec<String>)>> = Mutex::new(None);

/// 找 3.10 以上的 Python。只缓存成功的结果：用户装好 Python 后不用重启壳。
fn find_python() -> Result<(String, Vec<String>), String> {
    if let Some(found) = PYTHON.lock().unwrap().clone() {
        return Ok(found);
    }
    for (cmd, pre) in platform::python_candidates() {
        let out = quiet(cmd).args(pre).args(["-c", "import sys;print('%d.%d' % sys.version_info[:2])"]).output();
        let Ok(out) = out else { continue };
        let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let ok = ver.split_once('.').and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?))).is_some_and(|v| v >= (3, 10));
        if out.status.success() && ok {
            let found = (cmd.to_string(), pre.iter().map(|s| s.to_string()).collect());
            *PYTHON.lock().unwrap() = Some(found.clone());
            return Ok(found);
        }
    }
    Err(tr(
        "找不到 Python 3.10 或更高版本。请从 python.org 下载安装（安装时勾选「Add python.exe to PATH」），然后重启摸鱼工作台。",
        "Python 3.10+ not found. Install it from python.org (check \"Add python.exe to PATH\"), then restart Slacking Desk.",
    ))
}

fn quiet(program: &str) -> Command {
    let mut c = Command::new(program);
    c.stdin(Stdio::null()).stderr(Stdio::null());
    platform::prepare_command(&mut c);
    c
}

/// 读一行，超长的行截断并丢弃剩余部分。返回 false 表示 EOF。
fn read_line_limited(r: &mut impl BufRead, buf: &mut Vec<u8>) -> bool {
    buf.clear();
    match r.by_ref().take(MAX_LINE as u64).read_until(b'\n', buf) {
        Ok(0) | Err(_) => false,
        Ok(_) => {
            if buf.len() >= MAX_LINE && buf.last() != Some(&b'\n') {
                let mut skip = Vec::new();
                loop {
                    skip.clear();
                    match r.by_ref().take(MAX_LINE as u64).read_until(b'\n', &mut skip) {
                        Ok(0) | Err(_) => break,
                        Ok(_) if skip.last() == Some(&b'\n') => break,
                        Ok(_) => {}
                    }
                }
            }
            true
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "…"
    }
}

/// 插件日志：超过 1MB 时把旧文件改名为 .log.1，再写新文件
struct Logger {
    path: PathBuf,
    file: Option<std::fs::File>,
    size: u64,
}

impl Logger {
    fn open(path: PathBuf) -> Self {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let file = std::fs::OpenOptions::new().create(true).append(true).open(&path).ok();
        let size = file.as_ref().and_then(|f| f.metadata().ok()).map_or(0, |m| m.len());
        Self { path, file, size }
    }

    fn line(&mut self, text: &str) {
        if self.size > LOG_LIMIT {
            self.file = None;
            let _ = std::fs::rename(&self.path, self.path.with_extension("log.1"));
            *self = Logger::open(self.path.clone());
        }
        if let Some(f) = self.file.as_mut() {
            let line = format!("{} {}\n", now_hms(), text);
            if f.write_all(line.as_bytes()).is_ok() {
                self.size += line.len() as u64;
            }
        }
    }
}

/// 日志时间戳（UTC 时分秒，避免为了本地时区引入依赖）
fn now_hms() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    format!("{:02}:{:02}:{:02}Z", secs / 3600 % 24, secs / 60 % 60, secs % 60)
}

