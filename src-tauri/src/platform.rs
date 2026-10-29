//! 平台适配层：所有 Windows / macOS / Linux 的差异都集中在这里。
//! 差异说明见 docs/platform.md，改这里时同步更新文档。

use std::process::{Child, Command};

/// 当前平台在清单里的写法，用于 backend.entry 的按平台映射。
pub fn platform_key() -> String {
    let os = match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        _ => "linux",
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        _ => "x64",
    };
    format!("{os}-{arch}")
}

/// 查找 Python 的候选命令（按顺序尝试）。
/// macOS 从 Dock 启动的程序拿不到终端里的 PATH，所以要写全 Homebrew 等常见路径。
pub fn python_candidates() -> Vec<(&'static str, &'static [&'static str])> {
    if cfg!(windows) {
        vec![("py", &["-3"]), ("python", &[]), ("python3", &[])]
    } else if cfg!(target_os = "macos") {
        vec![
            ("/opt/homebrew/bin/python3", &[]),
            ("/usr/local/bin/python3", &[]),
            ("/Library/Frameworks/Python.framework/Versions/Current/bin/python3", &[]),
            ("python3", &[]),
        ]
    } else {
        vec![("python3", &[]), ("python", &[])]
    }
}

pub fn node_candidates() -> Vec<&'static str> {
    if cfg!(target_os = "macos") {
        vec!["/opt/homebrew/bin/node", "/usr/local/bin/node", "node"]
    } else {
        vec!["node"]
    }
}

/// 启动子进程前的平台设置：Windows 不弹黑框；Unix 放进独立进程组，方便整组结束。
pub fn prepare_command(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
}

/// 持有它就能结束插件进程以及它启动的所有子进程。被丢弃时也会结束整组进程。
/// Windows：Job Object（壳崩溃时系统也会自动结束）。Unix：进程组。
pub struct ProcessGuard {
    #[cfg(windows)]
    job: windows_sys::Win32::Foundation::HANDLE,
    #[cfg(unix)]
    pgid: i32,
}

// Job 句柄可以跨线程使用
unsafe impl Send for ProcessGuard {}
unsafe impl Sync for ProcessGuard {}

#[cfg(windows)]
impl ProcessGuard {
    pub fn new(child: &Child) -> Result<Self, String> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::JobObjects::*;
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if ok == 0 || AssignProcessToJobObject(job, child.as_raw_handle() as _) == 0 {
                let err = std::io::Error::last_os_error().to_string();
                CloseHandle(job);
                return Err(err);
            }
            Ok(Self { job })
        }
    }

    pub fn kill(&self) {
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.job, 1);
        }
    }
}

#[cfg(windows)]
impl Drop for ProcessGuard {
    fn drop(&mut self) {
        // 设置了 KILL_ON_JOB_CLOSE，关闭句柄时还活着的进程都会被结束
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.job);
        }
    }
}

#[cfg(unix)]
impl ProcessGuard {
    pub fn new(child: &Child) -> Result<Self, String> {
        Ok(Self { pgid: child.id() as i32 })
    }

    pub fn kill(&self) {
        unsafe {
            libc::killpg(self.pgid, libc::SIGKILL);
        }
    }
}

#[cfg(unix)]
impl Drop for ProcessGuard {
    fn drop(&mut self) {
        self.kill();
    }
}

// ---------- 首页快捷方式 ----------

fn ext_of(path: &std::path::Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

/// 首页图标的默认名字：去掉 .exe / .lnk / .app 这类扩展名，其他文件保留扩展名
pub fn display_name(path: &std::path::Path) -> String {
    let hide_ext = matches!(ext_of(path).as_str(), "exe" | "lnk" | "app" | "url");
    let name = if hide_ext { path.file_stem() } else { path.file_name() };
    name.map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

/// 没有窗口和控制台的脚本类型，"隐藏控制台"只对它们有效
#[cfg(windows)]
fn is_console_script(path: &std::path::Path) -> bool {
    matches!(ext_of(path).as_str(), "bat" | "cmd" | "ps1")
}

/// 简单的参数拆分：按空格分，双引号里的空格不拆
#[cfg(not(windows))]
fn split_args(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted, mut has) = (vec![], String::new(), false, false);
    for c in s.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                has = true;
            }
            c if c.is_whitespace() && !quoted => {
                if has {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            c => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has {
        out.push(cur);
    }
    out
}

fn shell_err(e: impl std::fmt::Display) -> String {
    crate::tr(&format!("打开失败：{e}"), &format!("Failed to open: {e}"))
}

/// 打开首页上的程序 / 文件 / 文件夹。工作目录是文件所在文件夹。
/// Windows 用 ShellExecuteEx（支持 lnk、"以管理员身份运行"）；ps1 交给 PowerShell 运行。
#[cfg(windows)]
pub fn launch(path: &std::path::Path, args: &str, hide_console: bool, as_admin: bool) -> Result<(), String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
    use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW};
    use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNORMAL};

    let (file, params) = if ext_of(path) == "ps1" {
        ("powershell.exe".to_string(), format!("-NoProfile -ExecutionPolicy Bypass -File \"{}\" {args}", path.display()))
    } else {
        (path.display().to_string(), args.to_string())
    };
    let dir = path.parent().map(|p| p.display().to_string()).unwrap_or_default();
    let (file, params, dir) = (HSTRING::from(file), HSTRING::from(params), HSTRING::from(dir));
    let verb = HSTRING::from(if as_admin { "runas" } else { "open" });
    let show = if hide_console && is_console_script(path) { SW_HIDE } else { SW_SHOWNORMAL };
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
            lpVerb: PCWSTR(verb.as_ptr()),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(params.as_ptr()),
            lpDirectory: PCWSTR(dir.as_ptr()),
            nShow: show.0,
            ..Default::default()
        };
        ShellExecuteExW(&mut info).map_err(|e| {
            // 1223 = 用户在 UAC 提示里点了"否"
            if e.code().0 as u32 & 0xFFFF == 1223 {
                crate::tr("已取消", "Cancelled")
            } else {
                shell_err(e.message())
            }
        })
    }
}

/// macOS：用 open 打开；.app 的参数用 --args 传；.sh 在"终端"里运行
#[cfg(target_os = "macos")]
pub fn launch(path: &std::path::Path, args: &str, _hide_console: bool, _as_admin: bool) -> Result<(), String> {
    let mut cmd = Command::new("/usr/bin/open");
    match ext_of(path).as_str() {
        "app" => {
            cmd.arg("-a").arg(path);
            let a = split_args(args);
            if !a.is_empty() {
                cmd.arg("--args").args(a);
            }
        }
        "sh" => {
            cmd.arg("-a").arg("Terminal").arg(path);
        }
        _ => {
            cmd.arg(path);
        }
    }
    let out = cmd.output().map_err(shell_err)?;
    if out.status.success() {
        Ok(())
    } else {
        Err(shell_err(String::from_utf8_lossy(&out.stderr).trim()))
    }
}

/// Linux：可执行文件直接运行，其他用 xdg-open
#[cfg(all(unix, not(target_os = "macos")))]
pub fn launch(path: &std::path::Path, args: &str, _hide_console: bool, _as_admin: bool) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let exec = path.is_file() && std::fs::metadata(path).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false);
    let mut cmd = if exec {
        let mut c = Command::new(path);
        c.args(split_args(args));
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(path);
        c
    };
    if let Some(dir) = path.parent() {
        cmd.current_dir(dir);
    }
    let mut child = cmd.spawn().map_err(shell_err)?;
    // 回收子进程，避免僵尸进程
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// 提取程序图标保存成 png。只处理 exe / lnk（Windows）和 .app（macOS），其他返回 false 用首字母。
#[cfg(windows)]
pub fn extract_icon(path: &std::path::Path, out: &std::path::Path) -> bool {
    if !matches!(ext_of(path).as_str(), "exe" | "lnk") {
        return false;
    }
    match win_icon(path) {
        Ok((w, h, rgba)) => save_png(out, w, h, &rgba),
        Err(e) => {
            eprintln!("提取图标失败 {}：{e}", path.display());
            false
        }
    }
}

#[cfg(windows)]
fn win_icon(path: &std::path::Path) -> windows::core::Result<(u32, u32, Vec<u8>)> {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let factory: IShellItemImageFactory = SHCreateItemFromParsingName(&HSTRING::from(path.as_os_str()), None)?;
        let bmp = factory.GetImage(SIZE { cx: 128, cy: 128 }, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK)?;
        let mut info = BITMAP::default();
        GetObjectW(bmp.into(), std::mem::size_of::<BITMAP>() as i32, Some(&mut info as *mut _ as *mut _));
        let (w, h) = (info.bmWidth, info.bmHeight);
        let mut bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let dc = GetDC(None);
        let lines = GetDIBits(dc, bmp, 0, h as u32, Some(buf.as_mut_ptr() as *mut _), &mut bi, DIB_RGB_COLORS);
        ReleaseDC(None, dc);
        let _ = DeleteObject(bmp.into());
        if lines == 0 {
            return Err(windows::core::Error::from_thread());
        }
        // BGRA（预乘透明度）→ RGBA
        for px in buf.chunks_exact_mut(4) {
            let a = px[3] as u32;
            let (b, g, r) = (px[0] as u32, px[1] as u32, px[2] as u32);
            let un = |c: u32| if a == 0 { 0 } else { (c * 255 / a).min(255) as u8 };
            px[0] = un(r);
            px[1] = un(g);
            px[2] = un(b);
        }
        Ok((w as u32, h as u32, buf))
    }
}

#[cfg(windows)]
fn save_png(out: &std::path::Path, w: u32, h: u32, rgba: &[u8]) -> bool {
    let Ok(file) = std::fs::File::create(out) else { return false };
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().and_then(|mut wr| wr.write_image_data(rgba)).is_ok()
}

/// macOS：读 .app 的 Info.plist 找到 icns，用系统自带的 sips 转成 png
#[cfg(target_os = "macos")]
pub fn extract_icon(path: &std::path::Path, out: &std::path::Path) -> bool {
    if ext_of(path) != "app" {
        return false;
    }
    let plist = path.join("Contents/Info.plist");
    let Ok(o) = Command::new("/usr/bin/plutil").args(["-extract", "CFBundleIconFile", "raw", "-o", "-"]).arg(&plist).output() else { return false };
    let mut name = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if !o.status.success() || name.is_empty() {
        return false;
    }
    if !name.ends_with(".icns") {
        name.push_str(".icns");
    }
    let icns = path.join("Contents/Resources").join(name);
    Command::new("/usr/bin/sips")
        .args(["-s", "format", "png", "-Z", "128"])
        .arg(&icns)
        .arg("--out")
        .arg(out)
        .output()
        .is_ok_and(|o| o.status.success() && out.exists())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn extract_icon(_path: &std::path::Path, _out: &std::path::Path) -> bool {
    false
}

/// 系统透明效果（Win11 Mica / macOS vibrancy）是否可用。Win10 和 Linux 不可用，毛玻璃会退回纯色。
pub fn glass_supported() -> bool {
    #[cfg(windows)]
    {
        use windows::Wdk::System::SystemServices::RtlGetVersion;
        use windows::Win32::System::SystemInformation::OSVERSIONINFOW;
        let mut v = OSVERSIONINFOW { dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32, ..Default::default() };
        unsafe { RtlGetVersion(&mut v).is_ok() && v.dwBuildNumber >= 22000 }
    }
    #[cfg(target_os = "macos")]
    {
        true
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        false
    }
}
