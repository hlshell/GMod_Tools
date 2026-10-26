//! 将 Slint UI 编译进二进制。把所有 .slint（main + components + pages）作为整体编译，
//! components / pages 通过 `import` 被 main.slint 引用，slint-build 会自动解析相对路径。
//! 另在 Windows 下用 windres 把应用图标 (app.ico) 嵌入 exe 文件图标。
use std::path::Path;

fn main() {
    // slint-build 在 Windows 默认线程栈（约 1MB）上解析深层嵌套的 .slint 可能触发
    // 栈溢出（STATUS_STACK_OVERFLOW，exit code 0xc00000fd）。
    // 将 Slint 编译放到 64MB 大栈线程中执行以规避。
    let compile_thread = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            slint_build::compile("src/ui/main.slint").expect("Slint UI 编译失败");
        });
    match compile_thread {
        Ok(handle) => handle.join().expect("Slint UI 编译线程执行失败"),
        Err(_) => {
            // 无法创建大栈线程时退回到当前线程编译
            slint_build::compile("src/ui/main.slint").expect("Slint UI 编译失败");
        }
    }

    // ---------- 嵌入 Windows 应用图标（exe 文件图标 / 资源管理器可见） ----------
    #[cfg(target_os = "windows")]
    {
        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
        let res = Path::new(&out_dir).join("gma_slint.res");
        let status = std::process::Command::new("windres")
            .arg("app.rc")
            .args(["-O", "coff", "-o"])
            .arg(&res)
            .status();
        match status {
            Ok(s) if s.success() => {
                println!("cargo:rustc-link-arg-bins={}", res.display());
            }
            _ => {
                // windres 不可用时不阻断构建；窗口/任务栏图标已由 Window.icon 提供
                eprintln!("warning: windres 不可用，跳过 exe 图标嵌入（窗口/任务栏图标仍生效）");
            }
        }
    }
}
