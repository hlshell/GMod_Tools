//! 创意工坊上传：调用 GMod 自带的 gmpublish.exe。
//!
//! gmpublish.exe 位于 `<GarrysMod>/bin/`，负责 Steam 登录鉴权与上传，
//! 常用命令：
//!   create:  gmpublish create -addon <x.gma> -icon <x.jpg>
//!   update:  gmpublish update -addon <x.gma> -id <workshopid> -changes "..."
//!
//! 运行前提：本机已安装并登录 Steam 客户端。

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::sync::{Arc, Mutex};

#[derive(PartialEq, Clone, Copy, Default)]
pub enum UploadMode {
    #[default]
    Create,
    Update,
}

pub struct UploadArgs {
    pub gmpublish: PathBuf,
    pub gma: PathBuf,
    pub icon: Option<PathBuf>,
    pub workshop_id: Option<String>,
    pub changes: Option<String>,
    pub mode: UploadMode,
}

/// 自动探测 gmpublish.exe：优先环境变量，随后扫描所有盘符的常见 Steam 库位置。
pub fn find_gmpublish() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("GARRYSMOD_BIN") {
        let f = PathBuf::from(p).join("gmpublish.exe");
        if f.is_file() {
            return Some(f);
        }
    }
    find_gmod_bin("gmpublish.exe")
}

/// 自动探测 gmad.exe（GMod 自带的 GMA 打包器），与 gmpublish 同目录。
pub fn find_gmad() -> Option<PathBuf> {
    find_gmod_bin("gmad.exe")
}

/// 自动探测 studiomdl.exe（MDL 打包用），复用同一套 Steam 库枚举。
pub fn find_studiomdl() -> Option<PathBuf> {
    find_gmod_bin("studiomdl.exe")
}

fn find_gmod_bin(name: &str) -> Option<PathBuf> {
    let sub = format!(r"steamapps\common\GarrysMod\bin\{name}");
    // 候选的 Steam 库根目录名（挂在任意盘符下）
    let roots = [
        "Steam",
        "SteamLibrary",
        "SteamLibrary2",
        "Steam2",
        "Program Files (x86)\\Steam",
        "Program Files\\Steam",
        "Program Files (x86)\\SteamLibrary",
    ];
    // 枚举所有盘符（A:\ ~ Z:\）
    for drive in b'A'..=b'Z' {
        let drive_root = format!("{}:\\", drive as char);
        let drive_path = Path::new(&drive_root);
        if !drive_path.is_dir() {
            continue;
        }
        // 盘符根本身可能就是一个 Steam 库（如 I:\SteamLibrary）
        let direct = drive_path.join(&sub);
        if direct.is_file() {
            return Some(direct);
        }
        for root in &roots {
            let p = drive_path.join(root).join(&sub);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// 运行 gmpublish，逐行回传输出，返回退出码。
pub fn run_gmpublish(
    args: &UploadArgs,
    on_line: impl FnMut(String) + Send + 'static,
) -> Result<i32, String> {
    let mut cmd = Command::new(&args.gmpublish);
    match args.mode {
        UploadMode::Create => {
            cmd.arg("create");
            if let Some(icon) = &args.icon {
                cmd.arg("-icon").arg(icon);
            }
            cmd.arg("-addon").arg(&args.gma);
        }
        UploadMode::Update => {
            cmd.arg("update");
            if let Some(id) = &args.workshop_id {
                cmd.arg("-id").arg(id);
            }
            cmd.arg("-addon").arg(&args.gma);
            if let Some(c) = &args.changes {
                cmd.arg("-changes").arg(c);
            }
        }
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    // 不弹出额外命令行窗口（Windows CREATE_NO_WINDOW = 0x08000000）
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("无法启动 gmpublish: {}", e))?;
    let stdout = child.stdout.take().ok_or("无法读取 gmpublish 输出")?;
    let stderr = child.stderr.take().ok_or("无法读取 gmpublish 错误输出")?;

    let on_line = Arc::new(Mutex::new(on_line));
    let ol_stderr = Arc::clone(&on_line);
    let stderr_handle = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            if let Ok(l) = line {
                ol_stderr.lock().unwrap()(l);
            }
        }
    });
    for line in BufReader::new(stdout).lines() {
        if let Ok(l) = line {
            on_line.lock().unwrap()(l);
        }
    }

    let status = child.wait().map_err(|e| format!("等待进程失败: {}", e))?;
    let code = status.code().unwrap_or(-1);

    // 等待 stderr 线程全部读完，避免进程失败时诊断信息丢失
    let _ = stderr_handle.join();
    Ok(code)
}
