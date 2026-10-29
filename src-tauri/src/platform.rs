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
