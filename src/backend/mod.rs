//! GMod GMA 工作室 — Slint 后端（框架无关层）
//!
//! 本模块把原 Tauri 版的后端命令全部移植为**纯 Rust 函数**：
//!   - 不依赖 tauri / 窗口系统，可被任何 UI 层（Slint / CLI）直接调用；
//!   - 需要"流式进度"的命令统一接收一个 `&mut Sink`（即 `FnMut(String)`），
//!     回传与前端相同的线协议：`P|进度` `S|状态` `L|日志` `E|错误` `D|完成`；
//!   - 长耗时操作本身是阻塞的，由调用方（Slint 层）放到线程里执行，避免卡 UI。
//!
//! 核心格式逻辑（GMA 读写 / VPK / VTF / MDL）在 `gma` / `upload` 子模块，与 UI 完全解耦。

pub mod gma;
pub mod goldsrc;
pub mod upload;
pub mod viewer;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ----------------------------- 线协议 / 流式回调 -----------------------------

/// 后端向 UI 层回报"进度 / 状态 / 日志 / 错误 / 完成"的统一回调。
/// 消息格式与旧 Tauri 前端一致，便于逐段解析：
///   `P|<0..1>` 进度  `S|<text>` 状态  `L|<text>` 日志  `E|<text>` 错误  `D` 完成
pub type Sink = Box<dyn FnMut(String) + Send>;

/// 便捷构造 Sink。
pub fn sink<F: FnMut(String) + Send + 'static>(f: F) -> Sink {
    Box::new(f)
}

/// 按语言选择文案。
pub fn tr(lang: &str, zh: &str, en: &str) -> String {
    if lang == "en" {
        en.into()
    } else {
        zh.into()
    }
}

// ----------------------------- DTO -----------------------------

#[derive(Deserialize)]
pub struct MetaDto {
    pub addon_name: String,
    pub addon_type: String,
    pub tags: Vec<String>,
    pub ignore: Vec<String>,
    pub description: String,
    pub addon_version: i64,
    pub steam_id: String,
}

impl MetaDto {
    pub fn to_gma_meta(&self) -> gma::GmaMeta {
        gma::GmaMeta {
            steam_id: self.steam_id.trim().parse().unwrap_or(0),
            timestamp: 0,
            addon_name: self.addon_name.clone(),
            addon_type: if self.addon_type.is_empty() {
                "ServerContent".into()
            } else {
                self.addon_type.clone()
            },
            tags: self.tags.clone(),
            ignore: self.ignore.clone(),
            description: self.description.clone(),
            addon_version: self.addon_version.max(1) as u32,
        }
    }
}

#[derive(Deserialize)]
pub struct UploadArgsDto {
    pub gmpublish: String,
    pub gma: String,
    pub icon: Option<String>,
    pub workshop_id: Option<String>,
    pub changes: Option<String>,
    pub mode: String,
    pub lang: String,
}

impl UploadArgsDto {
    pub fn to_args(&self) -> upload::UploadArgs {
        let mode = if self.mode == "update" {
            upload::UploadMode::Update
        } else {
            upload::UploadMode::Create
        };
        upload::UploadArgs {
            gmpublish: PathBuf::from(&self.gmpublish),
            gma: PathBuf::from(&self.gma),
            icon: self.icon.as_ref().filter(|s| !s.is_empty()).map(PathBuf::from),
            workshop_id: self.workshop_id.as_ref().filter(|s| !s.is_empty()).cloned(),
            changes: self.changes.as_ref().filter(|s| !s.is_empty()).cloned(),
            mode,
        }
    }
}

#[derive(Serialize, Clone)]
pub struct AddonJsonDto {
    pub title: String,
    pub addon_type: String,
    pub tags: Vec<String>,
    pub ignore: Vec<String>,
}

#[derive(Serialize, Clone, Default)]
pub struct OutDto {
    pub status: String,
    pub text: String,
    pub item_id: Option<String>,
}

#[derive(Serialize, Clone, Default)]
pub struct PackOutDto {
    pub status: String,
    pub gma: String,
}

// ----------------------------- 简单命令（无需线程） -----------------------------

pub fn is_dir(path: String) -> bool {
    PathBuf::from(path).is_dir()
}

/// 系统文件对话框。注意：某些平台需要在主线程调用，调用方请勿放到后台线程。
pub fn browse(role: String, title: String) -> Option<String> {
    let d = rfd::FileDialog::new().set_title(&title);
    let picked = match role.as_str() {
        "folder" => d.pick_folder(),
        "save" => d.set_file_name("addon.gma").add_filter("GMA", &["gma"]).save_file(),
        "exe" => d.add_filter("exe", &["exe"]).pick_file(),
        "icon" => d.add_filter("图片", &["jpg", "jpeg", "png"]).pick_file(),
        "vpk" => d.add_filter("VPK", &["vpk"]).pick_file(),
        "vpksave" => d.set_file_name("addon.vpk").add_filter("VPK", &["vpk"]).save_file(),
        "vtf" => d.add_filter("VTF", &["vtf"]).pick_file(),
        "qc" => d.add_filter("QC", &["qc"]).pick_file(),
        "vtfsave" => d.set_file_name("texture.vtf").add_filter("VTF", &["vtf"]).save_file(),
        "png" => d.add_filter("PNG", &["png"]).pick_file(),
        "pngsave" => d.set_file_name("texture.png").add_filter("PNG", &["png"]).save_file(),
        "mdl" => d.add_filter("MDL", &["mdl"]).pick_file(),
        "smdsave" => d.set_file_name("model.smd").add_filter("SMD", &["smd"]).save_file(),
        "import" => d.add_filter("模型包 (GMA/VPK)", &["gma", "vpk"]).pick_file(),
        _ => d.pick_file(),
    };
    picked.map(|p| p.to_string_lossy().into_owned())
}

pub fn detect_gmpublish() -> Option<String> {
    upload::find_gmpublish().map(|p| p.to_string_lossy().into_owned())
}

pub fn detect_gmad() -> Option<String> {
    upload::find_gmad().map(|p| p.to_string_lossy().into_owned())
}

pub fn detect_studiomdl() -> Option<String> {
    upload::find_studiomdl().map(|p| p.to_string_lossy().into_owned())
}

pub fn read_addon_json(folder: String) -> Option<AddonJsonDto> {
    let path = PathBuf::from(folder).join("addon.json");
    match gma::read_addon_json(&path) {
        Ok(a) => Some(AddonJsonDto {
            title: a.title,
            addon_type: a.addon_type,
            tags: a.tags,
            ignore: a.ignore,
        }),
        Err(_) => None,
    }
}

pub fn read_gma(path: String, lang: String) -> Result<OutDto, String> {
    let info = gma::read_gma(&PathBuf::from(&path)).map_err(|e| e.to_string())?;
    let l = lang.as_str();
    let mut t = String::new();
    t.push_str(&format!("{}: {}\n", tr(l, "名称", "Name"), info.meta.addon_name));
    t.push_str(&format!("{}: {}\n", tr(l, "类型", "Type"), info.meta.addon_type));
    t.push_str(&format!("{}: {}\n", tr(l, "版本", "Version"), info.meta.addon_version));
    t.push_str(&format!("SteamID: {}\n", info.meta.steam_id));
    if !info.meta.tags.is_empty() {
        t.push_str(&format!("{}: {}\n", tr(l, "标签", "Tags"), info.meta.tags.join(", ")));
    }
    t.push_str(&format!("{}: {}\n{} ----\n", tr(l, "文件数", "Files"), info.entries.len(), tr(l, "文件列表", "File list")));
    for e in &info.entries {
        t.push_str(&format!("#{}  {}  ({} B, {:08X})\n", e.index, e.path, e.size, e.crc32));
    }
    Ok(OutDto {
        status: tr(&lang, "读取完成", "Read complete"),
        text: t,
        item_id: None,
    })
}

// ----------------------------- 流式命令（阻塞，调用方负责线程） -----------------------------

/// 打包 GMA：生成 addon.json + 打包 .gma，逐条向 sink 回传进度。
pub fn pack(source: String, meta: MetaDto, out: String, lang: String, sink: &mut Sink) -> Result<PackOutDto, String> {
    let folder = PathBuf::from(&source);
    // 输出路径规范化：相对文件名 → 素材文件夹上一级目录；空 → 素材文件夹名；强制 .gma 扩展名
    let mut out_path = PathBuf::from(&out);
    if out.trim().is_empty() {
        let name = folder
            .file_name()
            .map(|n| format!("{}.gma", n.to_string_lossy()))
            .unwrap_or_else(|| "addon.gma".into());
        out_path = folder.parent().map(|p| p.join(&name)).unwrap_or(PathBuf::from(&name));
    } else if !out_path.is_absolute() {
        if let Some(parent) = folder.parent() {
            out_path = parent.join(&out_path);
        }
    }
    if out_path.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() != Some("gma") {
        out_path.set_extension("gma");
    }
    let out_disp = out_path.display().to_string();
    let gma_meta = meta.to_gma_meta();
    let l = lang.clone();
    if let Err(e) = gma::write_addon_json(&folder, &gma_meta) {
        let _ = sink(format!("L|{}: {e}", tr(&l, "生成 addon.json 失败", "Failed to write addon.json")));
    } else {
        let _ = sink(format!("L|{}", tr(&l, "已生成 addon.json", "addon.json created")));
    }
    let res = gma::pack_folder(&folder, &gma_meta, &out_path, |cur, total, name| {
        let f = if total == 0 { 0.0 } else { cur as f32 / total as f32 };
        let _ = sink(format!("P|{f}"));
        let _ = sink(format!("S|{name}"));
        let _ = sink(format!("L|{}: {name}", tr(&l, "打包", "Packing")));
    });
    match res {
        Ok(r) => {
            let _ = sink("P|1.0".to_string());
            let _ = sink(format!("S|{}", tr(&l, "打包完成", "Pack finished")));
            let _ = sink(format!("L|{}: {} 个文件 → {}", tr(&l, "打包完成", "Packed"), r.file_count, out_disp));
            let _ = sink("D".to_string());
            Ok(PackOutDto {
                status: tr(&l, "打包完成", "Pack finished"),
                gma: out_disp,
            })
        }
        Err(e) => {
            let _ = sink(format!("S|{}", tr(&l, "打包失败", "Pack failed")));
            let _ = sink(format!("E|{}: {e}", tr(&l, "打包失败", "Pack failed")));
            let _ = sink("D".to_string());
            Err(e)
        }
    }
}

/// 上传创意工坊：调用 gmpublish.exe，逐行回传输出。
pub fn upload(args: UploadArgsDto, sink: &mut Sink) -> Result<OutDto, String> {
    let lang = args.lang.clone();
    let l = lang.clone();
    let args = args.to_args();
    if !args.gmpublish.is_file() {
        return Err(tr(&l, "gmpublish.exe 不存在", "gmpublish.exe not found"));
    }
    if !args.gma.is_file() {
        return Err(tr(&l, ".gma 文件不存在", ".gma file not found"));
    }
    if args.mode == upload::UploadMode::Update && args.workshop_id.is_none() {
        return Err(tr(&l, "更新模式需要物品 ID", "Update mode requires an item ID"));
    }
    // sink 是借用，而 run_gmpublish 需要 'static 闭包 → 共享到 Arc<Mutex>，闭包与后续回传都走它。
    let sink_shared = std::sync::Arc::new(std::sync::Mutex::new(std::mem::replace(sink, Box::new(|_s: String| {}))));
    let sink_for_line = std::sync::Arc::clone(&sink_shared);
    let item_id = std::sync::Arc::new(std::sync::Mutex::new(None));
    let item_url = std::sync::Arc::new(std::sync::Mutex::new(None));
    let item_id_shared = std::sync::Arc::clone(&item_id);
    let item_url_shared = std::sync::Arc::clone(&item_url);
    let on_line = move |line: String| {
        {
            let mut guard = item_id_shared.lock().unwrap();
            if guard.is_none() {
                if let Some(pi) = line.find("item ID is") {
                    let digits: String = line[pi + "item ID is".len()..].chars().take_while(|c| c.is_ascii_digit()).collect();
                    if !digits.is_empty() {
                        *guard = Some(digits);
                    }
                }
            }
        }
        {
            let mut gu = item_url_shared.lock().unwrap();
            if gu.is_none() {
                if let Some(pl) = line.find("It is located at") {
                    let rest: String = line[pl + "It is located at".len()..].trim().to_string();
                    if !rest.is_empty() {
                        *gu = Some(rest);
                    }
                }
            }
        }
        if let Some(t) = translate_gmpublish_line(&line, &lang) {
            let mut g = sink_for_line.lock().unwrap();
            let _ = g(format!("L|{t}"));
        }
    };
    let mut send = |m: String| {
        let mut g = sink_shared.lock().unwrap();
        let _ = g(m);
    };
    let res = upload::run_gmpublish(&args, on_line);
    match res {
        Ok(0) => {
            send("P|1.0".to_string());
            send(format!("S|{}", tr(&l, "上传完成", "Upload finished")));
            let id = item_id.lock().unwrap().clone().or_else(|| args.workshop_id.clone());
            let url = item_url.lock().unwrap().clone();
            if let Some(u) = &url {
                send(format!("L|{}: {u}", tr(&l, "工坊链接", "Workshop link")));
            } else if let Some(i) = &id {
                send(format!("L|{}: http://steamcommunity.com/sharedfiles/filedetails/?id={i}", tr(&l, "工坊链接", "Workshop link")));
            }
            if let Some(i) = &id {
                send(format!("L|{}: {i}", tr(&l, "物品 ID", "Item ID")));
            }
            send("D".to_string());
            Ok(OutDto {
                status: tr(&l, "上传完成", "Upload finished"),
                text: String::new(),
                item_id: id,
            })
        }
        Ok(c) => {
            send(format!("S|{}", tr(&l, "上传失败", "Upload failed")));
            send(format!("E|{}: {c}", tr(&l, "gmpublish 退出码", "gmpublish exit code")));
            send("D".to_string());
            Err(format!("{}: {c}", tr(&l, "gmpublish 退出码", "gmpublish exit code")))
        }
        Err(e) => {
            send(format!("S|{}", tr(&l, "上传失败", "Upload failed")));
            send(format!("E|{e}"));
            send("D".to_string());
            Err(e)
        }
    }
}

/// 解包 GMA 到目录。
pub fn extract(path: String, out: String, lang: String, sink: &mut Sink) -> Result<OutDto, String> {
    let src = PathBuf::from(&path);
    let dst = PathBuf::from(&out);
    let out_disp = out.clone();
    let l = lang.clone();
    let res = gma::extract_gma(&src, &dst, |cur, total, name| {
        let f = if total == 0 { 0.0 } else { cur as f32 / total as f32 };
        let _ = sink(format!("P|{f}"));
        let _ = sink(format!("S|{name}"));
        let _ = sink(format!("L|{}: {name}", tr(&l, "解包", "Extract")));
    });
    match res {
        Ok(()) => {
            let _ = sink("P|1.0".to_string());
            let _ = sink(format!("S|{}", tr(&l, "解包完成", "Extract finished")));
            let _ = sink(format!("L|{} → {out_disp}", tr(&l, "解包完成", "Extracted")));
            let _ = sink("D".to_string());
            Ok(OutDto {
                status: tr(&l, "解包完成", "Extract finished"),
                text: format!("{} → {out_disp}", tr(&l, "解包完成", "Extracted")),
                item_id: None,
            })
        }
        Err(e) => {
            let _ = sink(format!("S|{}", tr(&l, "解包失败", "Extract failed")));
            let _ = sink(format!("E|{}: {e}", tr(&l, "解包失败", "Extract failed")));
            let _ = sink("D".to_string());
            Err(e)
        }
    }
}

// ----------------------------- VPK 工具 -----------------------------

pub fn vpk_pack(dir: String, out: String, lang: String, sink: &mut Sink) -> Result<OutDto, String> {
    let src = PathBuf::from(&dir);
    let out_path = PathBuf::from(&out);
    let l = lang.clone();
    let _ = sink(format!("S|{}", tr(&l, "正在打包 VPK", "Packing VPK")));
    match valve_pak::VPK::from_directory(&src) {
        Ok(vpk) => {
            let n = vpk.file_count();
            let _ = sink(format!("L|{}: {n}", tr(&l, "发现文件", "Found files")));
            match vpk.save(&out_path) {
                Ok(()) => {
                    let size = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
                    let _ = sink("P|1.0".to_string());
                    let _ = sink(format!("S|{}", tr(&l, "VPK 打包完成", "VPK packed")));
                    let _ = sink(format!("L|{} → {}", tr(&l, "已生成", "Saved to"), out_path.display()));
                    let _ = sink("D".to_string());
                    Ok(OutDto {
                        status: tr(&l, "VPK 打包完成", "VPK packed"),
                        text: format!("{n} {} · {size} B", tr(&l, "个文件", "files")),
                        item_id: None,
                    })
                }
                Err(e) => {
                    let _ = sink(format!("S|{}", tr(&l, "VPK 打包失败", "VPK pack failed")));
                    let _ = sink(format!("E|{}", e));
                    let _ = sink("D".to_string());
                    Err(e.to_string())
                }
            }
        }
        Err(e) => {
            let _ = sink(format!("S|{}", tr(&l, "VPK 打包失败", "VPK pack failed")));
            let _ = sink(format!("E|{}", e));
            let _ = sink("D".to_string());
            Err(e.to_string())
        }
    }
}

pub fn vpk_unpack(path: String, out: String, lang: String, sink: &mut Sink) -> Result<OutDto, String> {
    let src = PathBuf::from(&path);
    let dst = PathBuf::from(&out);
    let out_disp = out.clone();
    let l = lang.clone();
    let _ = sink(format!("S|{}", tr(&l, "正在解包 VPK", "Unpacking VPK")));
    match valve_pak::VPK::open(&src) {
        Ok(vpk) => {
            let paths: Vec<String> = vpk.file_paths().cloned().collect();
            let total = paths.len();
            let mut ok = 0usize;
            for (i, p) in paths.iter().enumerate() {
                let out_file = dst.join(p);
                if let Some(parent) = out_file.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let res: Result<(), String> = match vpk.get_file(p) {
                    Ok(mut f) => f.save(&out_file).map_err(|e| e.to_string()),
                    Err(e) => Err(e.to_string()),
                };
                match res {
                    Ok(()) => ok += 1,
                    Err(e) => {
                        let _ = sink(format!("L|E|{}: {} ({})", tr(&l, "失败", "Failed"), p, e));
                    }
                }
                let f = if total == 0 { 1.0 } else { (i + 1) as f32 / total as f32 };
                let _ = sink(format!("P|{f}"));
                let _ = sink(format!("L|{}: {p}", tr(&l, "解包", "Extract")));
            }
            let _ = sink("P|1.0".to_string());
            let _ = sink(format!("S|{} ({ok}/{total})", tr(&l, "VPK 解包完成", "VPK extracted")));
            let _ = sink(format!("L|{} → {out_disp}", tr(&l, "解包完成", "Extracted")));
            let _ = sink("D".to_string());
            Ok(OutDto {
                status: tr(&l, "VPK 解包完成", "VPK extracted"),
                text: format!("{ok}/{total} → {out_disp}"),
                item_id: None,
            })
        }
        Err(e) => {
            let _ = sink(format!("S|{}", tr(&l, "VPK 解包失败", "VPK extract failed")));
            let _ = sink(format!("E|{}", e));
            let _ = sink("D".to_string());
            Err(e.to_string())
        }
    }
}

// 目录树辅助：构建层级 + 树线输出（├─ └─ │，目录带 / 后缀）
struct TNode {
    children: std::collections::BTreeMap<String, TNode>,
}
fn emit_tree(node: &TNode, prefix: &str, out: &mut String) {
    let total = node.children.len();
    for (i, (name, child)) in node.children.iter().enumerate() {
        let last = i + 1 == total;
        out.push_str(prefix);
        out.push_str(if last { "└─ " } else { "├─ " });
        let is_dir = !child.children.is_empty();
        out.push_str(name);
        if is_dir {
            out.push('/');
        }
        out.push('\n');
        if is_dir {
            let np = format!("{}{}", prefix, if last { "    " } else { "│   " });
            emit_tree(child, &np, out);
        }
    }
}
fn emit_bone_tree(idx: usize, prefix: &str, is_last: bool, names: &[String], children: &[Vec<usize>], out: &mut String) {
    out.push_str(prefix);
    out.push_str(if is_last { "└─ " } else { "├─ " });
    out.push_str(&format!("[{}] {}", idx, names[idx]));
    out.push('\n');
    let kids = &children[idx];
    if !kids.is_empty() {
        let np = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
        for (i, kid) in kids.iter().enumerate() {
            emit_bone_tree(*kid, &np, i + 1 == kids.len(), names, children, out);
        }
    }
}

pub fn vpk_list(path: String, lang: String) -> Result<OutDto, String> {
    let src = PathBuf::from(&path);
    let l = lang.as_str();
    let vpk = valve_pak::VPK::open(&src).map_err(|e| e.to_string())?;
    let mut paths: Vec<String> = vpk.file_paths().cloned().collect();
    paths.sort();
    let n = paths.len();
    let mut root = TNode { children: std::collections::BTreeMap::new() };
    for p in &paths {
        let mut node = &mut root;
        for part in p.split('/').filter(|s| !s.is_empty()) {
            node = node.children.entry(part.to_string()).or_insert_with(|| TNode { children: std::collections::BTreeMap::new() });
        }
    }
    let mut t = format!("{}: {n}\n", tr(l, "文件数", "Files"));
    emit_tree(&root, "", &mut t);
    Ok(OutDto {
        status: tr(l, "读取完成", "Read complete"),
        text: t,
        item_id: None,
    })
}

// ----------------------------- 模型包（GMA/VPK）导入预览 -----------------------------

/// 判断路径是否为 GMA 文件（按扩展名）。
fn is_gma_path(p: &Path) -> bool {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() == Some("gma")
}

/// 把 GMA/VPK 全部解压到 out_dir（保留内部相对路径），返回文件数。
/// 用于模型查看导入：随后按目录扫描 .mdl 即可预览。
pub fn extract_archive_all(path: &str, out_dir: &Path, lang: &str, sink: &mut Sink) -> Result<usize, String> {
    let src = PathBuf::from(path);
    let l = lang.to_string();
    if is_gma_path(&src) {
        let _ = sink(format!("S|{}", tr(&l, "正在解压 GMA…", "Extracting GMA…")));
        gma::extract_gma(&src, out_dir, |done, total, name| {
            let f = if total == 0 { 0.0 } else { done as f32 / total as f32 };
            let _ = sink(format!("P|{f}"));
            let _ = sink(format!("L|{}: {name}", tr(&l, "解压", "Extract")));
        })
        .map_err(|e| e.to_string())?;
        let _ = sink(format!("P|1.0"));
        Ok(0)
    } else {
        let vpk = valve_pak::VPK::open(&src).map_err(|e| e.to_string())?;
        let paths: Vec<String> = vpk.file_paths().cloned().collect();
        let total = paths.len();
        let _ = sink(format!("S|{}", tr(&l, "正在解压 VPK…", "Extracting VPK…")));
        for (i, p) in paths.iter().enumerate() {
            let out_file = out_dir.join(p);
            if let Some(parent) = out_file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match vpk.get_file(p) {
                Ok(mut f) => {
                    if let Err(e) = f.save(&out_file) {
                        let _ = sink(format!("L|E|{}: {} ({})", tr(&l, "失败", "Failed"), p, e));
                    }
                }
                Err(e) => {
                    let _ = sink(format!("L|E|{}: {} ({})", tr(&l, "失败", "Failed"), p, e));
                }
            }
            let f = if total == 0 { 1.0 } else { (i + 1) as f32 / total as f32 };
            let _ = sink(format!("P|{f}"));
            let _ = sink(format!("L|{}: {p}", tr(&l, "解压", "Extract")));
        }
        let _ = sink(format!("P|1.0"));
        Ok(total)
    }
}

/// 递归收集目录下所有 .mdl（相对路径，按名称排序）。
pub fn find_models(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    walk(&p, root, out);
                } else if p.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() == Some("mdl") {
                    if let Ok(rel) = p.strip_prefix(root) {
                        out.push(rel.to_string_lossy().replace('\\', "/"));
                    }
                }
            }
        }
    }
    walk(root, root, &mut out);
    out.sort();
    out
}

// ----------------------------- VTF 工具 -----------------------------

pub fn vtf_info(path: String, lang: String) -> Result<OutDto, String> {
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    let v = vtf::from_bytes(&data).map_err(|e| e.to_string())?;
    let l = lang.as_str();
    let h = &v.header;
    let t = format!(
        "{}: {}x{}\n{}: {}\n{}: {}\n{}: {}\n{}: {}.{}",
        tr(l, "尺寸", "Size"),
        h.width,
        h.height,
        tr(l, "格式", "Format"),
        h.highres_image_format,
        tr(l, "帧数", "Frames"),
        h.frames,
        tr(l, "Mip 数", "Mip levels"),
        h.mipmap_count,
        tr(l, "版本", "Version"),
        h.version[0],
        h.version[1],
    );
    Ok(OutDto {
        status: tr(l, "读取完成", "Read complete"),
        text: t,
        item_id: None,
    })
}

pub fn vtf_convert(path: String, out: String, lang: String) -> Result<OutDto, String> {
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    let v = vtf::from_bytes(&data).map_err(|e| e.to_string())?;
    let img = v.highres_image.decode(0).map_err(|e| e.to_string())?;
    img.save(&out).map_err(|e| e.to_string())?;
    let out_disp = out.clone();
    Ok(OutDto {
        status: tr(&lang, "转换完成", "Converted"),
        text: format!("{} → {out_disp}", tr(&lang, "已生成", "Saved to")),
        item_id: None,
    })
}

/// 解码 VTF 并导出到临时 PNG，返回其路径（供 UI 预览）
pub fn vtf_preview(path: String) -> Result<String, String> {
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    let v = vtf::from_bytes(&data).map_err(|e| e.to_string())?;
    let img = v.highres_image.decode(0).map_err(|e| e.to_string())?;
    let out = std::env::temp_dir().join("gma_vtf_preview.png");
    img.save(&out).map_err(|e| e.to_string())?;
    Ok(out.to_string_lossy().into_owned())
}

pub fn read_png_data_url(path: String) -> Result<String, String> {
    use base64::Engine;
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
    Ok(format!("data:image/png;base64,{b64}"))
}

pub fn vtf_create(path: String, out: String, format: String, lang: String) -> Result<OutDto, String> {
    use image::GenericImageView;
    let img = image::open(&path).map_err(|e| format!("{}: {e}", tr(&lang, "图片读取失败", "Failed to read image")))?;
    let (w, h) = img.dimensions();
    let (nw, nh) = if w.is_power_of_two() && h.is_power_of_two() {
        (w, h)
    } else {
        (w.next_power_of_two(), h.next_power_of_two())
    };
    let img = if (w, h) != (nw, nh) {
        image::DynamicImage::ImageRgba8(image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Lanczos3))
    } else {
        img
    };
    let fmt = match format.as_str() {
        "dxt1" => vtf::ImageFormat::Dxt1Onebitalpha,
        "rgba" => vtf::ImageFormat::Rgba8888,
        "rgb" => vtf::ImageFormat::Rgb888,
        _ => vtf::ImageFormat::Dxt5,
    };
    let bytes = vtf::create(img, fmt).map_err(|e| format!("{}: {e}", tr(&lang, "VTF 编码失败", "VTF encode failed")))?;
    std::fs::write(&out, &bytes).map_err(|e| e.to_string())?;
    let out_disp = out.clone();
    Ok(OutDto {
        status: tr(&lang, "转换完成", "Converted"),
        text: format!("{} ({w}×{h} → {nw}×{nh}) → {out_disp} ({:.0} KB)", tr(&lang, "已生成", "Saved to"), bytes.len() as f64 / 1024.0),
        item_id: None,
    })
}

// ----------------------------- MDL 工具（Source 模型） -----------------------------

fn m_read_i32(d: &[u8], off: usize) -> i32 {
    if off + 4 > d.len() {
        return 0;
    }
    i32::from_le_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
}
fn m_read_f32(d: &[u8], off: usize) -> f32 {
    if off + 4 > d.len() {
        return 0.0;
    }
    f32::from_le_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
}
fn m_read_u16(d: &[u8], off: usize) -> u16 {
    if off + 2 > d.len() {
        return 0;
    }
    u16::from_le_bytes([d[off], d[off + 1]])
}
fn m_read_cstr(d: &[u8], off: usize) -> String {
    if off >= d.len() {
        return String::new();
    }
    let end = d[off..].iter().position(|&b| b == 0).map(|i| off + i).unwrap_or(d.len());
    String::from_utf8_lossy(&d[off..end]).into_owned()
}
fn m_read_euler(d: &[u8], off: usize) -> (f32, f32, f32) {
    (m_read_f32(d, off), m_read_f32(d, off + 4), m_read_f32(d, off + 8))
}

pub fn mdl_info(path: String, lang: String) -> Result<OutDto, String> {
    let d = std::fs::read(&path).map_err(|e| e.to_string())?;
    if d.len() < 4 || &d[0..4] != b"IDST" {
        return Err(tr(&lang, "不是有效的 Source MDL 文件（缺 IDST 魔数）", "Not a valid Source MDL (missing IDST magic)"));
    }
    let l = lang.as_str();
    let ver = m_read_i32(&d, 4);
    let name = m_read_cstr(&d, 12);
    let flags = m_read_i32(&d, 152);
    let numbones = m_read_i32(&d, 156).max(0) as usize;
    let boneindex = m_read_i32(&d, 160).max(0) as usize;
    let nseq = m_read_i32(&d, 188).max(0);
    let ntex = m_read_i32(&d, 204).max(0);
    let nbody = m_read_i32(&d, 228).max(0) as usize;
    let bodyindex = m_read_i32(&d, 232).max(0) as usize;

    let mut t = String::new();
    t.push_str(&format!(
        "{}: v{}\n{}: {}\n",
        tr(l, "MDL 版本", "MDL version"), ver,
        tr(l, "模型名称", "Model name"), name
    ));
    t.push_str(&format!("├─ {}: {}\n", tr(l, "骨骼数", "Bones"), numbones));
    t.push_str(&format!("├─ {}: {}\n", tr(l, "序列数", "Sequences"), nseq));
    t.push_str(&format!("├─ {}: {}\n", tr(l, "贴图数", "Textures"), ntex));

    // 骨骼树（按 parent 关系）
    if numbones > 0 && boneindex + numbones * 216 <= d.len() {
        let mut names: Vec<String> = Vec::with_capacity(numbones);
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); numbones];
        let mut roots: Vec<usize> = Vec::new();
        for i in 0..numbones {
            let b = boneindex + i * 216;
            let ni = m_read_i32(&d, b).max(0) as usize;
            let parent = m_read_i32(&d, b + 4);
            names.push(m_read_cstr(&d, b + ni));
            if parent < 0 {
                roots.push(i);
            } else if (parent as usize) < numbones {
                children[parent as usize].push(i);
            }
        }
        t.push_str(&format!("├─ {} ({})\n", tr(l, "骨骼", "Bones"), roots.len()));
        for (i, r) in roots.iter().enumerate() {
            emit_bone_tree(*r, "│  ", i + 1 == roots.len(), &names, &children, &mut t);
        }
    } else {
        t.push_str(&format!("├─ {}: 0\n", tr(l, "骨骼", "Bones")));
    }

    // 模型结构树
    if nbody > 0 && bodyindex > 0 && bodyindex + nbody * 16 <= d.len() {
        t.push_str(&format!("└─ {}\n", tr(l, "模型结构", "Model structure")));
        for i in 0..nbody {
            let bp = bodyindex + i * 16;
            let bni = m_read_i32(&d, bp).max(0) as usize;
            let nmod = m_read_i32(&d, bp + 4).max(0) as usize;
            let midx = m_read_i32(&d, bp + 12).max(0) as usize;
            let bpname = m_read_cstr(&d, bp + bni);
            let last = i + 1 == nbody;
            t.push_str(&format!("{}├─ {}[{}] {}\n", if last { "   " } else { "│  " }, tr(l, "部件", "Bodypart"), i, bpname));
            let mbase = bp + midx;
            let child_prefix = format!("{}   ", if last { "   " } else { "│  " });
            for m in 0..nmod {
                let mo = mbase + m * 208;
                if mo + 88 > d.len() {
                    continue;
                }
                let mn = m_read_cstr(&d, mo);
                let nmesh = m_read_i32(&d, mo + 72).max(0);
                let nvert = m_read_i32(&d, mo + 80).max(0);
                let mlast = m + 1 == nmod;
                t.push_str(&format!(
                    "{}{} {}{}: {} ({}: {}, {}: {})\n",
                    child_prefix, if mlast { "└─ " } else { "├─ " },
                    tr(l, "模型", "model"), m, mn,
                    tr(l, "网格", "meshes"), nmesh, tr(l, "顶点", "verts"), nvert
                ));
            }
        }
    }
    t.push_str(&format!("{}: {:#010x}", tr(l, "Flags", "Flags"), flags));
    Ok(OutDto {
        status: tr(l, "读取完成", "Read complete"),
        text: t,
        item_id: None,
    })
}

pub fn mdl_skeleton(path: String, out: String, lang: String) -> Result<OutDto, String> {
    let d = std::fs::read(&path).map_err(|e| e.to_string())?;
    if d.len() < 4 || &d[0..4] != b"IDST" {
        return Err(tr(&lang, "不是有效的 Source MDL 文件（缺 IDST 魔数）", "Not a valid Source MDL (missing IDST magic)"));
    }
    let l = lang.as_str();
    let numbones = m_read_i32(&d, 156).max(0) as usize;
    let boneindex = m_read_i32(&d, 160).max(0) as usize;
    if numbones == 0 || boneindex + numbones * 216 > d.len() {
        return Err(tr(l, "该模型没有骨骼数据（静态网格，无法导出骨架）", "Model has no bones (static mesh, cannot export skeleton)"));
    }
    let mut s = String::new();
    s.push_str("version 1\n");
    s.push_str("nodes\n");
    for i in 0..numbones {
        let b = boneindex + i * 216;
        let ni = m_read_i32(&d, b).max(0) as usize;
        let parent = m_read_i32(&d, b + 4);
        let nm = m_read_cstr(&d, b + ni);
        s.push_str(&format!("{} \"{}\" {}\n", i, nm, parent));
    }
    s.push_str("end\n");
    s.push_str("skeleton\n");
    s.push_str("time 0\n");
    for i in 0..numbones {
        let b = boneindex + i * 216;
        let (rx, ry, rz) = m_read_euler(&d, b + 60);
        s.push_str(&format!(
            "{} {} {} {} {} {} {}\n",
            i,
            m_read_f32(&d, b + 32),
            m_read_f32(&d, b + 36),
            m_read_f32(&d, b + 40),
            rx.to_degrees(),
            ry.to_degrees(),
            rz.to_degrees()
        ));
    }
    s.push_str("end\n");
    std::fs::write(&out, s).map_err(|e| e.to_string())?;
    let out_disp = out.clone();
    Ok(OutDto {
        status: tr(l, "导出完成", "Exported"),
        text: format!("{} → {out_disp}", tr(l, "已生成", "Saved to")),
        item_id: None,
    })
}

pub fn mdl_decompile(path: String, out: String, lang: String) -> Result<OutDto, String> {
    let mdl_path = PathBuf::from(&path);
    let mdl = std::fs::read(&mdl_path).map_err(|e| e.to_string())?;
    if mdl.len() < 4 || &mdl[0..4] != b"IDST" {
        return Err(tr(&lang, "不是有效的 Source MDL 文件（缺 IDST 魔数）", "Not a valid Source MDL (missing IDST magic)"));
    }
    let l = lang.as_str();

    let stem = mdl_path.with_extension("");
    let vvd_path = stem.with_extension("vvd");
    if !vvd_path.is_file() {
        return Err(tr(l, "缺少配套的 .vvd 顶点文件", "Missing companion .vvd vertex file"));
    }
    let mut vtx_path: Option<PathBuf> = None;
    for ext in ["dx90.vtx", "dx80.vtx", "sw.vtx", "vtx"] {
        let p = stem.with_extension(ext);
        if p.is_file() {
            vtx_path = Some(p);
            break;
        }
    }
    let vtx_path = vtx_path.ok_or_else(|| tr(l, "缺少配套的 .vtx 网格文件", "Missing companion .vtx mesh file"))?;

    let vvd = std::fs::read(&vvd_path).map_err(|e| e.to_string())?;
    let vtx = std::fs::read(&vtx_path).map_err(|e| e.to_string())?;

    let nvert = m_read_i32(&vvd, 16).max(0) as usize;
    let vdata_start = m_read_i32(&vvd, 56).max(0) as usize;
    if nvert == 0 || vdata_start + nvert * 48 > vvd.len() {
        return Err(tr(l, "VVD 顶点池异常", "VVD vertex pool invalid"));
    }

    let numbones = m_read_i32(&mdl, 156).max(0) as usize;
    let boneindex = m_read_i32(&mdl, 160).max(0) as usize;
    let mut bone_names: Vec<String> = Vec::new();
    let mut bone_parent: Vec<i32> = Vec::new();
    let mut bone_pos: Vec<(f32, f32, f32)> = Vec::new();
    let mut bone_rot: Vec<(f32, f32, f32)> = Vec::new();
    if numbones > 0 && boneindex + numbones * 216 <= mdl.len() {
        for i in 0..numbones {
            let b = boneindex + i * 216;
            let ni = m_read_i32(&mdl, b).max(0) as usize;
            bone_names.push(m_read_cstr(&mdl, b + ni));
            bone_parent.push(m_read_i32(&mdl, b + 4));
            bone_pos.push((m_read_f32(&mdl, b + 32), m_read_f32(&mdl, b + 36), m_read_f32(&mdl, b + 40)));
            bone_rot.push(m_read_euler(&mdl, b + 60));
        }
    }

    let ntex = m_read_i32(&mdl, 204).max(0) as usize;
    let texidx = m_read_i32(&mdl, 208).max(0) as usize;
    let mut material = String::from("unknown_material");
    if ntex > 0 && texidx + ntex * 64 <= mdl.len() {
        let t0 = texidx;
        let ni = m_read_i32(&mdl, t0).max(0) as usize;
        let nm = m_read_cstr(&mdl, t0 + ni);
        if !nm.is_empty() {
            material = nm;
        }
    }

    let mut s = String::new();
    s.push_str("version 1\n");
    s.push_str("nodes\n");
    for i in 0..numbones {
        s.push_str(&format!("{} \"{}\" {}\n", i, bone_names.get(i).map(|x| x.as_str()).unwrap_or(""), bone_parent.get(i).copied().unwrap_or(0)));
    }
    s.push_str("end\n");
    s.push_str("skeleton\n");
    s.push_str("time 0\n");
    for i in 0..numbones {
        let p = bone_pos.get(i).copied().unwrap_or((0.0, 0.0, 0.0));
        let r = bone_rot.get(i).copied().unwrap_or((0.0, 0.0, 0.0));
        s.push_str(&format!("{} {} {} {} {} {} {}\n", i, p.0, p.1, p.2, r.0.to_degrees(), r.1.to_degrees(), r.2.to_degrees()));
    }
    s.push_str("end\n");
    s.push_str("triangles\n");

    let mut tri_count = 0usize;
    let nbp = m_read_i32(&vtx, 28).max(0) as usize;
    let bp_off = m_read_i32(&vtx, 32).max(0) as usize;
    let mut mat_emitted = false;
    for b in 0..nbp {
        let bp = bp_off + b * 8;
        if bp + 8 > vtx.len() {
            continue;
        }
        let nmod = m_read_i32(&vtx, bp).max(0) as usize;
        let moff = m_read_i32(&vtx, bp + 4).max(0) as usize;
        let mo_base = bp + moff;
        for mi in 0..nmod {
            let mo = mo_base + mi * 8;
            if mo + 8 > vtx.len() {
                continue;
            }
            let nlod = m_read_i32(&vtx, mo).max(0) as usize;
            let loff = m_read_i32(&vtx, mo + 4).max(0) as usize;
            let lod_base = mo + loff;
            if nlod == 0 {
                continue;
            }
            let lo = lod_base;
            if lo + 12 > vtx.len() {
                continue;
            }
            let nmesh = m_read_i32(&vtx, lo).max(0) as usize;
            let mesh_off = m_read_i32(&vtx, lo + 4).max(0) as usize;
            let mesh_base = lo + mesh_off;
            for m in 0..nmesh {
                let ms = mesh_base + m * 9;
                if ms + 9 > vtx.len() {
                    continue;
                }
                let nsg = m_read_i32(&vtx, ms).max(0) as usize;
                let sg_off = m_read_i32(&vtx, ms + 4).max(0) as usize;
                let sg_base = ms + sg_off;
                for g in 0..nsg {
                    let sg = sg_base + g * 25;
                    if sg + 25 > vtx.len() {
                        continue;
                    }
                    let gv = m_read_i32(&vtx, sg).max(0) as usize;
                    let gv_off = m_read_i32(&vtx, sg + 4).max(0) as usize;
                    let gi = m_read_i32(&vtx, sg + 8).max(0) as usize;
                    let gi_off = m_read_i32(&vtx, sg + 12).max(0) as usize;
                    let gstrips = m_read_i32(&vtx, sg + 16).max(0) as usize;
                    let gs_off = m_read_i32(&vtx, sg + 20).max(0) as usize;
                    let vpool = sg + gv_off;
                    let ipool = sg + gi_off;
                    let sbase = sg + gs_off;
                    if vpool + gv * 9 > vtx.len() || ipool + gi * 2 > vtx.len() {
                        continue;
                    }
                    if !mat_emitted {
                        s.push_str(&format!("{} 0\n", material));
                        mat_emitted = true;
                    }
                    for sti in 0..gstrips {
                        let st = sbase + sti * 27;
                        if st + 27 > vtx.len() {
                            continue;
                        }
                        let sni = m_read_i32(&vtx, st).max(0) as usize;
                        let sio = m_read_i32(&vtx, st + 4).max(0) as usize;
                        let sfl = vtx[st + 18];
                        if sio + sni > gi {
                            continue;
                        }
                        let trilist = (sfl & 0x2) == 0;
                        let lookup = |li: usize| -> (f32, f32, f32, f32, f32, f32, f32, f32, Vec<(i32, f32)>) {
                            let vt = vpool + li * 9;
                            let oid = m_read_u16(&vtx, vt + 4) as usize;
                            if oid >= nvert {
                                return (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, Vec::new());
                            }
                            let vv = vdata_start + oid * 48;
                            let px = m_read_f32(&vvd, vv + 16);
                            let py = m_read_f32(&vvd, vv + 20);
                            let pz = m_read_f32(&vvd, vv + 24);
                            let nx = m_read_f32(&vvd, vv + 28);
                            let ny = m_read_f32(&vvd, vv + 32);
                            let nz = m_read_f32(&vvd, vv + 36);
                            let u = m_read_f32(&vvd, vv + 40);
                            let v = m_read_f32(&vvd, vv + 44);
                            let nb = vvd[vv + 15] as usize;
                            let mut links: Vec<(i32, f32)> = Vec::new();
                            for k in 0..nb.min(3) {
                                let b = vvd[vv + 12 + k] as i32;
                                let w = m_read_f32(&vvd, vv + k * 4);
                                links.push((b, w));
                            }
                            (px, py, pz, nx, ny, nz, u, v, links)
                        };
                        let strip_idx: Vec<u16> = (0..sni).map(|k| m_read_u16(&vtx, ipool + (sio + k) * 2)).collect();
                        let emit_tri = |s: &mut String, ia: usize, ib: usize, ic: usize, cnt: &mut usize| {
                            if ia >= gv || ib >= gv || ic >= gv {
                                return;
                            }
                            let verts = [lookup(ia), lookup(ib), lookup(ic)];
                            for (idx, vtup) in verts.iter().enumerate() {
                                let (px, py, pz, nx, ny, nz, u, v, links) = vtup;
                                let parent = if links.is_empty() { -1 } else { links[0].0 };
                                s.push_str(&format!(
                                    "{} {} {:.6} {:.6} {:.6} {:.6} {:.6} {:.6} {:.6} {:.6} {}",
                                    parent,
                                    idx,
                                    px,
                                    py,
                                    pz,
                                    nx,
                                    ny,
                                    nz,
                                    u,
                                    v,
                                    links.len()
                                ));
                                for (bn, bw) in links {
                                    s.push_str(&format!(" {} {:.6}", bn, bw));
                                }
                                s.push('\n');
                            }
                            *cnt += 1;
                        };
                        if trilist {
                            let mut t = 0;
                            while t + 2 < sni {
                                emit_tri(&mut s, strip_idx[t] as usize, strip_idx[t + 1] as usize, strip_idx[t + 2] as usize, &mut tri_count);
                                t += 3;
                            }
                        } else {
                            let mut t = 0;
                            while t + 2 < sni {
                                if t % 2 == 0 {
                                    emit_tri(&mut s, strip_idx[t] as usize, strip_idx[t + 1] as usize, strip_idx[t + 2] as usize, &mut tri_count);
                                } else {
                                    emit_tri(&mut s, strip_idx[t + 1] as usize, strip_idx[t] as usize, strip_idx[t + 2] as usize, &mut tri_count);
                                }
                                t += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    s.push_str("end\n");
    if tri_count == 0 {
        return Err(tr(l, "未能解析到任何三角面（网格为空）", "No triangles parsed (empty mesh)"));
    }
    std::fs::write(&out, s).map_err(|e| e.to_string())?;
    let out_disp = out.clone();
    Ok(OutDto {
        status: tr(l, "反编译完成", "Decompiled"),
        text: format!("{}: {} {} → {out_disp}", tr(l, "三角面", "Triangles"), tri_count, tr(l, "个", "pcs")),
        item_id: None,
    })
}

/// MDL 打包：调用 studiomdl.exe 编译 .qc。命令过程逐行流式回传。
pub fn mdl_pack(qc: String, studiomdl: String, out: String, lang: String, sink: &mut Sink) -> Result<OutDto, String> {
    let l = lang.clone();
    let qc_p = PathBuf::from(&qc);
    let sm = PathBuf::from(&studiomdl);
    if !sm.is_file() {
        return Err(tr(&l, "studiomdl.exe 不存在（可到“关于→备用工具”手动指定）", "studiomdl.exe not found (set manually under About→Fallback tools)"));
    }
    if !qc_p.is_file() {
        return Err(tr(&l, ".qc 文件不存在", ".qc file not found"));
    }
    let gmod_root = sm.parent().and_then(|p| p.parent()).unwrap_or_else(|| std::path::Path::new(""));
    let game_dir = gmod_root.join("garrysmod");
    let out_mdl = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let out_mdl_shared = std::sync::Arc::clone(&out_mdl);
    use std::io::BufRead;
    let ch = sink;
    let l2 = l.clone();
    let mut cmd = std::process::Command::new(&sm);
    cmd.arg("-game").arg(&game_dir);
    cmd.arg("-nop4");
    cmd.arg(&qc_p);
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = ch(format!("S|{}", tr(&l2, "打包失败", "Pack failed")));
            let _ = ch(format!("E|{}: {e}", tr(&l2, "无法启动 studiomdl", "Failed to start studiomdl")));
            let _ = ch("D".into());
            return Err(e.to_string());
        }
    };
    let stdout = child.stdout.take().ok_or_else(|| "无法读取 studiomdl 输出".to_string())?;
    let stderr = child.stderr.take().ok_or_else(|| "无法读取 studiomdl 错误输出".to_string())?;
    let om_err = std::sync::Arc::clone(&out_mdl_shared);
    let l_err = l2.clone();
    // stderr 独立线程，实时回传（此处 sink 为 &mut，需要挪进线程；用管道封装）
    let ch_stderr = std::sync::Mutex::new(String::new()); // 占位：真实实现见下方处理
    let _ = ch_stderr;
    let _ = l_err;
    // 由于 sink 是 &mut 借用，无法直接 move 进线程；这里改为单线程轮询读取 stdout，
    // stderr 通过 drain 方式在 join 前同步收集（与旧版行为等价）。
    let mut stderr_lines: Vec<String> = Vec::new();
    {
        for line in std::io::BufReader::new(stderr).lines() {
            if let Ok(l) = line {
                stderr_lines.push(l.clone());
                let _ = ch(format!("L|{l}"));
                if let Some(pi) = l.find("Output MDL") {
                    let rest: String = l[pi + "Output MDL".len()..].trim().trim_start_matches(|c| c == ':' || c == ' ').to_string();
                    if !rest.is_empty() {
                        *om_err.lock().unwrap() = Some(rest);
                    }
                }
            }
        }
    }
    for line in std::io::BufReader::new(stdout).lines() {
        if let Ok(l) = line {
            let _ = ch(format!("L|{l}"));
            if let Some(pi) = l.find("Output MDL") {
                let rest: String = l[pi + "Output MDL".len()..].trim().trim_start_matches(|c| c == ':' || c == ' ').to_string();
                if !rest.is_empty() {
                    *out_mdl_shared.lock().unwrap() = Some(rest);
                }
            }
        }
    }
    let status = child.wait().ok();
    match status {
        Some(s) if s.success() => {
            let _ = ch("P|1.0".into());
            let _ = ch(format!("S|{}", tr(&l2, "打包完成", "Pack finished")));
            let m = out_mdl_shared.lock().unwrap().clone().unwrap_or_else(|| qc.replace(".qc", ".mdl"));
            let _ = ch(format!("L|{}: {m}", tr(&l2, "输出模型", "Output MDL")));
            if !out.trim().is_empty() {
                let src = std::path::PathBuf::from(&m);
                if src.is_file() {
                    let out_dir = std::path::PathBuf::from(&out);
                    if let Ok(_) = std::fs::create_dir_all(&out_dir) {
                        let dest = out_dir.join(src.file_name().unwrap());
                        match std::fs::copy(&src, &dest) {
                            Ok(_) => {
                                let _ = ch(format!("L|{}: {}", tr(&l2, "已复制到", "Copied to"), dest.display()));
                            }
                            Err(e) => {
                                let _ = ch(format!("L|{}: {e}", tr(&l2, "复制失败", "Copy failed")));
                            }
                        }
                    }
                }
            }
            let _ = ch("D".into());
            Ok(OutDto {
                status: tr(&l2, "打包完成", "Pack finished"),
                text: format!("→ {m}"),
                item_id: None,
            })
        }
        Some(s) => {
            let _ = ch(format!("S|{}", tr(&l2, "打包失败", "Pack failed")));
            let _ = ch(format!("E|{}: {}", tr(&l2, "studiomdl 退出码", "studiomdl exit code"), s.code().unwrap_or(-1)));
            let _ = ch("D".into());
            Err(format!("{}: {:?}", tr(&l2, "studiomdl 退出码", "studiomdl exit code"), s.code()))
        }
        None => {
            let _ = ch(format!("S|{}", tr(&l2, "打包失败", "Pack failed")));
            let _ = ch(format!("E|{}", tr(&l2, "进程被终止", "Process terminated")));
            let _ = ch("D".into());
            Err(tr(&l2, "进程被终止", "Process terminated"))
        }
    }
}

// ----------------------------- 模型查看：数据模型 -----------------------------

#[derive(Serialize, Clone)]
pub struct MdlBoneView {
    pub name: String,
    pub parent: i32,
    pub pos: [f32; 3],
    pub rot: [f32; 3], // bind euler（度）
    pub children: Vec<usize>,
}

#[derive(Serialize, Clone)]
pub struct MdlSeqView {
    pub name: String,
    pub activity: String,
    pub frames: i32,
    pub fps: f32,
}

#[derive(Serialize, Clone)]
pub struct MdlBodyGroupView {
    pub name: String,
    pub models: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct MdlHitboxView {
    pub name: String,
    pub bone: i32,
    pub group: i32,
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
}

#[derive(Serialize, Clone)]
pub struct MdlAttachmentView {
    pub name: String,
    pub bone: i32,
    pub pos: [f32; 3],
}

#[derive(Serialize, Clone, Default)]
pub struct MdlTextureView {
    pub name: String,
    pub path: Option<String>,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>, // RGBA8，w*h*4；空 = 无贴图
}

#[derive(Serialize, Clone)]
pub struct MdlMeshView {
    pub bones: Vec<MdlBoneView>,
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub uvs: Vec<f32>,
    pub indices: Vec<u32>,
    pub tri_count: u32,
    pub bounds: [f32; 6],
    pub sequences: Vec<MdlSeqView>,
    pub bodygroups: Vec<MdlBodyGroupView>,
    pub hitboxes: Vec<MdlHitboxView>,
    pub attachments: Vec<MdlAttachmentView>,
    pub textures: Vec<MdlTextureView>,
    pub mat_of_tri: Vec<i32>, // 每个三角形 → textures 索引；-1 = 无贴图（平面着色）
    // 皮肤族：skins[f][slot] → textures 索引（f 为皮肤族，slot 为该三角形的皮肤槽位 tri_skinref）
    pub skins: Vec<Vec<i32>>,
    // 每个三角形的皮肤槽位（mesh 的 skinref），皮肤族切换时据此重映射材质
    pub tri_skinref: Vec<i32>,
    // 顶点蒙皮：每顶点最多 4 根骨骼（索引 + 权重），用于姿势/序列动画使网格随骨骼形变
    pub bone_indices: Vec<[i32; 4]>,
    pub bone_weights: Vec<[f32; 4]>,
}

/// 皮肤族解析：slot（皮肤槽位）→ textures 索引；无皮肤表时原样返回 slot。
pub(crate) fn skin_resolve(skins: &[Vec<i32>], slot: i32, family: u32) -> i32 {
    if skins.is_empty() {
        return slot;
    }
    skins
        .get(family as usize)
        .and_then(|row| row.get(slot.max(0) as usize))
        .copied()
        .unwrap_or(-1)
}

/// 从 .mdl 所在目录向上定位 addon 的 materials 根目录（与 models 平级）。
fn mdl_find_materials_root(mdl_path: &Path) -> Option<PathBuf> {
    let mut dir = mdl_path.parent()?.to_path_buf();
    for _ in 0..12 {
        if let Some(parent) = dir.parent() {
            let m = parent.join("materials");
            if m.is_dir() {
                return Some(m);
            }
        }
        dir = dir.parent()?.to_path_buf();
    }
    None
}

/// 解析 .vmt 中的 `$basetexture`（大小写不敏感），返回归一化路径（带 .vtf 后缀）。
fn parse_vmt_basetexture(vmt: &Path) -> Option<String> {
    let text = std::fs::read_to_string(vmt).ok()?;
    let mut base = None;
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(idx) = line.to_lowercase().find("$basetexture") {
            let rest = &line[idx + "$basetexture".len()..];
            // 取引号内值（也兼容裸值）
            let val = if let Some(open) = rest.find('"') {
                let tail = &rest[open + 1..];
                tail.split('"').next().unwrap_or("").trim().to_string()
            } else {
                rest.split_whitespace().next().unwrap_or("").trim().to_string()
            };
            if !val.is_empty() {
                base = Some(val);
                break;
            }
        }
    }
    let b = base?;
    // 去前导 materials/ 并统一斜杠
    let b = b.replace('\\', "/");
    let b = if let Some(stripped) = b.strip_prefix("materials/") {
        stripped.to_string()
    } else {
        b
    };
    // 后缀统一为 .vtf
    let lower = b.to_lowercase();
    if lower.ends_with(".vtf") || lower.ends_with(".tga") || lower.ends_with(".png") {
        let stem = Path::new(&b).file_stem()?.to_string_lossy().into_owned();
        Some(format!("{stem}.vtf"))
    } else {
        Some(format!("{b}.vtf"))
    }
}

/// 大小写不敏感地在目录里找目标文件名。
fn find_file_ci(dir: &Path, needle: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for e in entries.flatten() {
        if e.file_name().to_string_lossy().eq_ignore_ascii_case(needle) {
            return Some(e.path());
        }
    }
    None
}

/// 解析材质名 → 相邻/位于 addon materials 根下的 .vtf 纹理路径。
fn mdl_resolve_texture(mdl_path: &Path, mat_name: &str) -> Option<PathBuf> {
    let mdl_dir = mdl_path.parent()?;
    let base = mat_name.trim();
    if base.is_empty() {
        return None;
    }
    let norm = base.replace('\\', "/");
    let norm = if let Some(s) = norm.strip_prefix("materials/") { s.to_string() } else { norm };
    let basename = Path::new(&norm).file_stem()?.to_string_lossy().into_owned();
    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut push = |c: PathBuf, cands: &mut Vec<PathBuf>| {
        if !cands.contains(&c) {
            cands.push(c);
        }
    };

    // 1) 模型同目录同名
    push(mdl_dir.join(format!("{basename}.vtf")), &mut candidates);

    // 2) addon materials 根（GMod 模型预览同款：先解析 .vmt 的 $basetexture）
    if let Some(root) = mdl_find_materials_root(mdl_path) {
        let vmt_path = root.join(format!("{norm}.vmt"));
        if vmt_path.is_file() {
            if let Some(bt) = parse_vmt_basetexture(&vmt_path) {
                push(root.join(bt), &mut candidates);
            }
        }
        // 材质同名 .vtf（无 vmt 或 vmt 无 basetexture 时兜底）
        push(root.join(format!("{norm}.vtf")), &mut candidates);
    }

    // 3) 大小写不敏感兜底（相邻目录 / materials 根）
    for c in &candidates {
        if c.is_file() {
            return Some(c.clone());
        }
    }
    if let Some(parent) = mdl_dir.to_path_buf().parent() {
        if let Some(f) = find_file_ci(parent, &format!("{basename}.vtf")) {
            return Some(f);
        }
    }
    if let Some(root) = mdl_find_materials_root(mdl_path) {
        if let Some(f) = find_file_ci(&root, &format!("{basename}.vtf")) {
            return Some(f);
        }
    }
    None
}

/// 解码 .vtf 为 RGBA8（帧 0）。
fn mdl_decode_texture(path: &Path, name: &str) -> MdlTextureView {
    use image::GenericImageView;
    let mut t = MdlTextureView { name: name.to_string(), ..Default::default() };
    if let Ok(data) = std::fs::read(path) {
        if let Ok(v) = vtf::from_bytes(&data) {
            if let Ok(img) = v.highres_image.decode(0) {
                let (w, h) = img.dimensions();
                let total = (w as usize).saturating_mul(h as usize).saturating_mul(4);
                if w > 0 && h > 0 && total <= 256 * 1024 * 1024 {
                    let buf = img.into_rgba8().into_raw();
                    t.width = w;
                    t.height = h;
                    t.rgba = buf;
                    t.path = Some(path.to_string_lossy().into_owned());
                }
            }
        }
    }
    t
}

/// 读取 .mdl+.vvd+.vtx：网格几何（含 UV、逐三角面材质）、骨骼、身体组、
/// 序列、命中框、附着点、贴图。`body_sel[b]` 指定第 b 个身体组选中的子模型索引。
pub fn mdl_mesh_data(path: String, body_sel: Vec<i32>, lang: String) -> Result<MdlMeshView, String> {
    let mdl_path = PathBuf::from(&path);
    let mdl = std::fs::read(&mdl_path).map_err(|e| e.to_string())?;
    if mdl.len() < 8 || &mdl[0..4] != b"IDST" {
        return Err(tr(&lang, "不是有效的 MDL 文件（缺 IDST 魔数）", "Not a valid MDL (missing IDST magic)"));
    }
    // 版本 10 = GoldSrc（HL1/CS1.6 旧格式，单文件内嵌纹理/动画）
    if goldsrc::is_goldsrc(&mdl) {
        return goldsrc::mesh_data(&mdl_path, &body_sel, &lang);
    }
    let l = lang.as_str();
    let stem = mdl_path.with_extension("");
    let vvd_path = stem.with_extension("vvd");
    if !vvd_path.is_file() {
        return Err(tr(l, "缺少配套的 .vvd 顶点文件", "Missing companion .vvd vertex file"));
    }
    let mut vtx_path: Option<PathBuf> = None;
    for ext in ["dx90.vtx", "dx80.vtx", "sw.vtx", "vtx"] {
        let p = stem.with_extension(ext);
        if p.is_file() {
            vtx_path = Some(p);
            break;
        }
    }
    let vtx_path = vtx_path.ok_or_else(|| tr(l, "缺少配套的 .vtx 网格文件", "Missing companion .vtx mesh file"))?;
    let vvd = std::fs::read(&vvd_path).map_err(|e| e.to_string())?;
    let vtx = std::fs::read(&vtx_path).map_err(|e| e.to_string())?;

    let nvert = m_read_i32(&vvd, 16).max(0) as usize;
    let vdata_start = m_read_i32(&vvd, 56).max(0) as usize;
    if nvert == 0 || vdata_start + nvert * 48 > vvd.len() {
        return Err(tr(l, "VVD 顶点池异常", "VVD vertex pool invalid"));
    }

    // ---- 骨骼（bind pose）----
    let numbones = m_read_i32(&mdl, 156).max(0) as usize;
    let boneindex = m_read_i32(&mdl, 160).max(0) as usize;
    let mut bones: Vec<MdlBoneView> = Vec::new();
    if numbones > 0 && boneindex + numbones * 216 <= mdl.len() {
        for i in 0..numbones {
            let b = boneindex + i * 216;
            let ni = m_read_i32(&mdl, b).max(0) as usize;
            bones.push(MdlBoneView {
                name: m_read_cstr(&mdl, b + ni),
                parent: m_read_i32(&mdl, b + 4),
                pos: [m_read_f32(&mdl, b + 32), m_read_f32(&mdl, b + 36), m_read_f32(&mdl, b + 40)],
                rot: [
                    m_read_f32(&mdl, b + 60).to_degrees(),
                    m_read_f32(&mdl, b + 64).to_degrees(),
                    m_read_f32(&mdl, b + 68).to_degrees(),
                ],
                children: Vec::new(),
            });
        }
        for i in 0..bones.len() {
            let p = bones[i].parent;
            if p >= 0 && (p as usize) < bones.len() {
                bones[p as usize].children.push(i);
            }
        }
    }

    // ---- 贴图（材质名 → 相邻 .vtf）----
    let ntex = m_read_i32(&mdl, 204).max(0) as usize;
    let texidx = m_read_i32(&mdl, 208).max(0) as usize;
    let mut textures: Vec<MdlTextureView> = Vec::new();
    if ntex > 0 && texidx + ntex * 64 <= mdl.len() {
        for i in 0..ntex {
            let t0 = texidx + i * 64;
            let ni = m_read_i32(&mdl, t0).max(0) as usize;
            let name = m_read_cstr(&mdl, t0 + ni);
            if let Some(p) = mdl_resolve_texture(&mdl_path, &name) {
                textures.push(mdl_decode_texture(&p, &name));
            } else {
                textures.push(MdlTextureView { name, ..Default::default() });
            }
        }
    }

    // ---- 皮肤族（Source：numskinref@212 / numskinfamilies@216 / skinindex@220，i32 表）----
    let nskinref = m_read_i32(&mdl, 212).max(0) as usize;
    let nskinfamilies = m_read_i32(&mdl, 216).max(0) as usize;
    let skinindex = m_read_i32(&mdl, 220).max(0) as usize;
    let mut skins: Vec<Vec<i32>> = Vec::new();
    if nskinref > 0 && nskinfamilies > 0 && skinindex.saturating_add(nskinfamilies * nskinref * 4) <= mdl.len() {
        for f in 0..nskinfamilies {
            let mut row = Vec::with_capacity(nskinref);
            for s in 0..nskinref {
                let v = m_read_i32(&mdl, skinindex + (f * nskinref + s) * 4);
                row.push(if v >= 0 && (v as usize) < textures.len() { v } else { -1 });
            }
            skins.push(row);
        }
    }

    // ---- 身体组（bodypart = bodygroup，子模型 = model）----
    let nbp = m_read_i32(&mdl, 228).max(0) as usize;
    let bp_off = m_read_i32(&mdl, 232).max(0) as usize;
    let mut bodygroups: Vec<MdlBodyGroupView> = Vec::new();
    // 每个身体组 → 选定子模型内，各 mesh → 皮肤槽位（供 vtx 遍历匹配材质）
    let mut body_model_mesh_mats: Vec<Vec<i32>> = Vec::new();
    if nbp > 0 && bp_off + nbp * 16 <= mdl.len() {
        for b in 0..nbp {
            let bp = bp_off + b * 16;
            let bni = m_read_i32(&mdl, bp).max(0) as usize;
            let nmod = m_read_i32(&mdl, bp + 4).max(0) as usize;
            let midx = m_read_i32(&mdl, bp + 12).max(0) as usize;
            let name = m_read_cstr(&mdl, bp + bni);
            let mbase = bp + midx;
            let mut models: Vec<String> = Vec::new();
            for m in 0..nmod {
                let mo = mbase + m * 208;
                if mo + 88 > mdl.len() {
                    continue;
                }
                models.push(m_read_cstr(&mdl, mo));
            }
            bodygroups.push(MdlBodyGroupView { name, models });
            let sel = body_sel.get(b).copied().unwrap_or(0).max(0) as usize;
            let mut mesh_mats: Vec<i32> = Vec::new();
            if sel < nmod {
                let mo = mbase + sel * 208;
                if mo + 88 <= mdl.len() {
                    let nmesh = m_read_i32(&mdl, mo + 72).max(0) as usize;
                    let mesh_index = m_read_i32(&mdl, mo + 76).max(0) as usize;
                    for m in 0..nmesh {
                        let ms = mo + mesh_index + m * 88;
                        if ms + 88 > mdl.len() {
                            break;
                        }
                        // mesh.material 即皮肤槽位（skinref slot）
                        let slot = m_read_i32(&mdl, ms);
                        mesh_mats.push(if slot >= 0 { slot } else { -1 });
                    }
                }
            }
            body_model_mesh_mats.push(mesh_mats);
        }
    }

    // ---- 几何（vtx 遍历，按身体组选择过滤）----
    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut uvs: Vec<f32> = Vec::new();
    let mut bone_indices: Vec<[i32; 4]> = Vec::new();
    let mut bone_weights: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut mat_of_tri: Vec<i32> = Vec::new();
    let mut tri_skinref: Vec<i32> = Vec::new();
    let mut tri_count = 0u32;
    let mut minx = f32::MAX;
    let mut miny = f32::MAX;
    let mut minz = f32::MAX;
    let mut maxx = f32::MIN;
    let mut maxy = f32::MIN;
    let mut maxz = f32::MIN;

    let vtx_nbp = m_read_i32(&vtx, 28).max(0) as usize;
    let vtx_bp_off = m_read_i32(&vtx, 32).max(0) as usize;
    for b in 0..vtx_nbp {
        let bp = vtx_bp_off + b * 8;
        if bp + 8 > vtx.len() {
            continue;
        }
        let nmod = m_read_i32(&vtx, bp).max(0) as usize;
        let moff = m_read_i32(&vtx, bp + 4).max(0) as usize;
        let mo_base = bp + moff;
        let sel = body_sel.get(b).copied().unwrap_or(0).max(0) as usize;
        if sel >= nmod {
            continue; // 选中的子模型在 vtx 无数据（空身体组）
        }
        let mo = mo_base + sel * 8;
        if mo + 8 > vtx.len() {
            continue;
        }
        let nlod = m_read_i32(&vtx, mo).max(0) as usize;
        let loff = m_read_i32(&vtx, mo + 4).max(0) as usize;
        let lod_base = mo + loff;
        if nlod == 0 {
            continue;
        }
        let lo = lod_base;
        if lo + 12 > vtx.len() {
            continue;
        }
        let nmesh = m_read_i32(&vtx, lo).max(0) as usize;
        let mesh_off = m_read_i32(&vtx, lo + 4).max(0) as usize;
        let mesh_base = lo + mesh_off;
        for m in 0..nmesh {
            let ms = mesh_base + m * 9;
            if ms + 9 > vtx.len() {
                continue;
            }
            let nsg = m_read_i32(&vtx, ms).max(0) as usize;
            let sg_off = m_read_i32(&vtx, ms + 4).max(0) as usize;
            let sg_base = ms + sg_off;
            let slot = body_model_mesh_mats.get(b).and_then(|v| v.get(m)).copied().unwrap_or(-1);
            let mat = skin_resolve(&skins, slot, 0);
            for g in 0..nsg {
                let sg = sg_base + g * 25;
                if sg + 25 > vtx.len() {
                    continue;
                }
                let gv = m_read_i32(&vtx, sg).max(0) as usize;
                let gv_off = m_read_i32(&vtx, sg + 4).max(0) as usize;
                let gi = m_read_i32(&vtx, sg + 8).max(0) as usize;
                let gi_off = m_read_i32(&vtx, sg + 12).max(0) as usize;
                let gstrips = m_read_i32(&vtx, sg + 16).max(0) as usize;
                let gs_off = m_read_i32(&vtx, sg + 20).max(0) as usize;
                let vpool = sg + gv_off;
                let ipool = sg + gi_off;
                let sbase = sg + gs_off;
                if vpool + gv * 9 > vtx.len() || ipool + gi * 2 > vtx.len() {
                    continue;
                }
                for sti in 0..gstrips {
                    let st = sbase + sti * 27;
                    if st + 27 > vtx.len() {
                        continue;
                    }
                    let sni = m_read_i32(&vtx, st).max(0) as usize;
                    let sio = m_read_i32(&vtx, st + 4).max(0) as usize;
                    let sfl = vtx[st + 18];
                    if sio + sni > gi {
                        continue;
                    }
                    let trilist = (sfl & 0x2) == 0;
                    let mut local_map: Vec<u32> = Vec::with_capacity(sni);
                    for k in 0..sni {
                        let li = m_read_u16(&vtx, ipool + (sio + k) * 2) as usize;
                        if li >= gv {
                            local_map.push(u32::MAX);
                            continue;
                        }
                        let oid = m_read_u16(&vtx, vpool + li * 9 + 4) as usize;
                        if oid >= nvert {
                            local_map.push(u32::MAX);
                            continue;
                        }
                        let vv = vdata_start + oid * 48;
                        let (x, y, z) = (m_read_f32(&vvd, vv + 16), m_read_f32(&vvd, vv + 20), m_read_f32(&vvd, vv + 24));
                        let (nx, ny, nz) = (m_read_f32(&vvd, vv + 28), m_read_f32(&vvd, vv + 32), m_read_f32(&vvd, vv + 36));
                        let (u, v) = (m_read_f32(&vvd, vv + 40), m_read_f32(&vvd, vv + 44));
                        // 顶点骨骼权重（VVD 前 16 字节：4 骨骼索引 @+0、4 权重 @+4、numbones @+8）
                        let nb = vvd.get(vv + 8).copied().unwrap_or(0) as usize;
                        let mut bix = [-1i32; 4];
                        let mut bwt = [0.0f32; 4];
                        if nb <= 4 && vv + 9 <= vvd.len() {
                            for k in 0..nb.min(4) {
                                bix[k] = vvd[vv + k] as i32;
                                bwt[k] = (vvd[vv + 4 + k] as f32) / 255.0;
                            }
                        }
                        bone_indices.push(bix);
                        bone_weights.push(bwt);
                        minx = minx.min(x);
                        miny = miny.min(y);
                        minz = minz.min(z);
                        maxx = maxx.max(x);
                        maxy = maxy.max(y);
                        maxz = maxz.max(z);
                        positions.push(x);
                        positions.push(y);
                        positions.push(z);
                        normals.push(nx);
                        normals.push(ny);
                        normals.push(nz);
                        uvs.push(u);
                        uvs.push(v);
                        local_map.push((positions.len() / 3 - 1) as u32);
                    }
                    let mut t = 0;
                    if trilist {
                        while t + 2 < sni {
                            let a = local_map[t];
                            let b2 = local_map[t + 1];
                            let c = local_map[t + 2];
                            if a != u32::MAX && b2 != u32::MAX && c != u32::MAX {
                                indices.push(a);
                                indices.push(b2);
                                indices.push(c);
                                mat_of_tri.push(mat);
                                tri_skinref.push(slot);
                                tri_count += 1;
                            }
                            t += 3;
                        }
                    } else {
                        while t + 2 < sni {
                            let (a, b2, c) = if t % 2 == 0 {
                                (local_map[t], local_map[t + 1], local_map[t + 2])
                            } else {
                                (local_map[t + 1], local_map[t], local_map[t + 2])
                            };
                            if a != u32::MAX && b2 != u32::MAX && c != u32::MAX {
                                indices.push(a);
                                indices.push(b2);
                                indices.push(c);
                                mat_of_tri.push(mat);
                                tri_skinref.push(slot);
                                tri_count += 1;
                            }
                            t += 1;
                        }
                    }
                }
            }
        }
    }
    if positions.is_empty() || tri_count == 0 {
        return Err(tr(l, "未能解析到任何网格几何", "No mesh geometry parsed"));
    }
    if minx > maxx {
        minx = 0.0;
        maxx = 1.0;
        miny = 0.0;
        maxy = 1.0;
        minz = 0.0;
        maxz = 1.0;
    }

    // ---- 序列（名称 / 帧数 / fps；内联数据解析能力有限，播放能力见 UI）----
    let nseq = m_read_i32(&mdl, 188).max(0) as usize;
    let seqidx = m_read_i32(&mdl, 192).max(0) as usize;
    let mut sequences: Vec<MdlSeqView> = Vec::new();
    if nseq > 0 && seqidx + nseq * 276 <= mdl.len() {
        for i in 0..nseq {
            let s = seqidx + i * 276;
            let sni = m_read_i32(&mdl, s + 4).max(0) as usize;
            let ani = m_read_i32(&mdl, s + 8).max(0) as usize;
            sequences.push(MdlSeqView {
                name: m_read_cstr(&mdl, s + sni),
                activity: m_read_cstr(&mdl, s + ani),
                frames: m_read_i32(&mdl, s + 144).clamp(0, 100000),
                fps: m_read_f32(&mdl, s + 148).clamp(0.0, 120.0),
            });
        }
    }

    // ---- 命中框 ----
    let nhs = m_read_i32(&mdl, 172).max(0) as usize;
    let hsidx = m_read_i32(&mdl, 176).max(0) as usize;
    let mut hitboxes: Vec<MdlHitboxView> = Vec::new();
    if nhs > 0 && hsidx + 12 <= mdl.len() {
        let nh = m_read_i32(&mdl, hsidx + 4).max(0) as usize;
        let hi = m_read_i32(&mdl, hsidx + 8).max(0) as usize;
        for i in 0..nh {
            let h = hi + i * 48;
            if h + 48 > mdl.len() {
                break;
            }
            let hni = m_read_i32(&mdl, h + 32).max(0) as usize;
            hitboxes.push(MdlHitboxView {
                name: m_read_cstr(&mdl, h + hni),
                bone: m_read_i32(&mdl, h),
                group: m_read_i32(&mdl, h + 4),
                mins: [m_read_f32(&mdl, h + 8), m_read_f32(&mdl, h + 12), m_read_f32(&mdl, h + 16)],
                maxs: [m_read_f32(&mdl, h + 20), m_read_f32(&mdl, h + 24), m_read_f32(&mdl, h + 28)],
            });
        }
    }

    // ---- 附着点（attachment）----
    let natt = m_read_i32(&mdl, 248).max(0) as usize;
    let attidx = m_read_i32(&mdl, 252).max(0) as usize;
    let mut attachments: Vec<MdlAttachmentView> = Vec::new();
    if natt > 0 && attidx + natt * 88 <= mdl.len() {
        for i in 0..natt {
            let a = attidx + i * 88;
            if a + 40 > mdl.len() {
                break;
            }
            let ani = m_read_i32(&mdl, a).max(0) as usize;
            attachments.push(MdlAttachmentView {
                name: m_read_cstr(&mdl, a + ani),
                bone: m_read_i32(&mdl, a + 8),
                pos: [m_read_f32(&mdl, a + 16), m_read_f32(&mdl, a + 20), m_read_f32(&mdl, a + 24)],
            });
        }
    }

    Ok(MdlMeshView {
        bones,
        positions,
        normals,
        uvs,
        indices,
        tri_count,
        bounds: [minx, miny, minz, maxx, maxy, maxz],
        sequences,
        bodygroups,
        hitboxes,
        attachments,
        textures,
        mat_of_tri,
        bone_indices,
        bone_weights,
        skins,
        tri_skinref,
    })
}

/// 解码一条 mstudioanimvalue_t RLE 序列（Source 动画的标准 RLE 编码）：
/// - valid!=0：后跟 16 位绝对量化值（0..1），覆盖 total 帧；
/// - valid==0：后跟 1 字节带符号 delta（每帧线性累加），覆盖 total 帧。
/// 输出归一化 [0,1] 浮点。
fn decode_animvalue(mdl: &[u8], start: usize, nframes: usize, out: &mut [f32]) -> bool {
    let mut cur = 0usize;
    let mut idx = start;
    let mut abs = 0.0f32;
    while cur < nframes {
        if idx + 2 > mdl.len() {
            return false;
        }
        let valid = mdl[idx];
        let total = mdl[idx + 1] as usize;
        if total == 0 {
            idx += 1;
            continue;
        }
        let n = total.min(nframes - cur);
        if valid != 0 {
            if idx + 4 > mdl.len() {
                return false;
            }
            let v = (((mdl[idx + 2] as u16) << 8) | mdl[idx + 3] as u16) as f32 / 65535.0;
            abs = v;
            for i in 0..n {
                out[cur + i] = v;
            }
            cur += n;
            idx += 4;
        } else {
            if idx + 3 > mdl.len() {
                return false;
            }
            let delta = (mdl[idx + 2] as i8) as f32 / 255.0;
            for i in 0..n {
                out[cur + i] = abs + delta * (i as f32);
            }
            abs += delta * (n as f32);
            cur += n;
            idx += 3;
        }
    }
    true
}

/// 解码指定序列的骨骼动画：返回每帧每骨骼的局部旋转增量（欧拉角，度），供蒙皮/骨骼绘制播放。
/// 基于 StudioMDL 经典格式 best-effort 解码（mstudioanimvalue RLE 旋转通道 → 欧拉增量）；
/// 任何越界或格式不符都会安全降级为 0（不崩）。需真实模型验证旋转通道精度。
pub fn mdl_sequence_anim(path: String, seq_index: i32, lang: String) -> Result<Vec<Vec<[f32; 3]>>, String> {
    let mdl = std::fs::read(&path).map_err(|e| e.to_string())?;
    if mdl.len() < 8 || &mdl[0..4] != b"IDST" {
        return Err(tr(&lang, "不是有效的 MDL 文件", "Not a valid MDL"));
    }
    if goldsrc::is_goldsrc(&mdl) {
        return goldsrc::sequence_anim(std::path::Path::new(&path), seq_index, &lang);
    }
    let nseq = m_read_i32(&mdl, 188).max(0) as usize;
    let seqidx = m_read_i32(&mdl, 192).max(0) as usize;
    if seq_index < 0 || (seq_index as usize) >= nseq {
        return Err(tr(&lang, "序列序号越界", "Sequence index out of range"));
    }
    let s = seqidx + (seq_index as usize) * 276;
    let nframes = m_read_i32(&mdl, s + 144).clamp(0, 100000) as usize;
    let numbones = m_read_i32(&mdl, 156).max(0) as usize;
    if nframes == 0 || numbones == 0 {
        return Ok(vec![vec![[0.0f32, 0.0, 0.0]; numbones]; nframes.max(1)]);
    }
    let animindex = m_read_i32(&mdl, s + 0).max(0) as usize;
    let animtable = s + animindex;
    let mut frames: Vec<Vec<[f32; 3]>> = vec![vec![[0.0f32, 0.0, 0.0]; numbones]; nframes];
    for bone in 0..numbones {
        let off = m_read_i32(&mdl, animtable + bone * 4).max(0) as usize;
        let anim = animtable + off;
        if anim + 8 > mdl.len() {
            continue;
        }
        // mstudioanim_t：bone(1) flags(1) next(2)，随后 6 个 mstudioanimvalue_t（3 位置 + 3 旋转）
        // 旋转通道位于偏移 +4 + 3*4 = +16 起
        let mut chans: Vec<Vec<f32>> = vec![vec![0.0f32; nframes]; 3];
        let mut ok = true;
        for c in 0..3 {
            if !decode_animvalue(&mdl, anim + 16 + c * 4, nframes, &mut chans[c]) {
                ok = false;
                break;
            }
        }
        if !ok {
            continue;
        }
        for f in 0..nframes {
            frames[f][bone] = [
                (chans[0][f] - 0.5) * 360.0,
                (chans[1][f] - 0.5) * 360.0,
                (chans[2][f] - 0.5) * 360.0,
            ];
        }
    }
    Ok(frames)
}

// ----------------------------- 素材完整性检查 -----------------------------

struct CheckState {
    files: usize,
    total: u64,
    junk: Vec<String>,
    stray_gma: Vec<String>,
    has_png_jpg: bool,
    empty_dirs: usize,
}

fn check_walk(dir: &PathBuf, depth: usize, st: &mut CheckState) -> std::io::Result<()> {
    if depth > 8 {
        return Ok(());
    }
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        let lower = name.to_lowercase();
        if e.file_type()?.is_dir() {
            check_walk(&p, depth + 1, st)?;
            if depth >= 1 {
                let mut it = std::fs::read_dir(&p)?;
                if it.next().is_none() {
                    st.empty_dirs += 1;
                }
            }
        } else {
            st.files += 1;
            st.total += e.metadata().map(|m| m.len()).unwrap_or(0);
            if lower == "thumbs.db" || lower == "desktop.ini" || lower == ".ds_store" || lower == ".git" || lower == ".svn" {
                st.junk.push(name);
            }
            if lower.ends_with(".gma") {
                st.stray_gma.push(p.display().to_string());
            }
            if lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
                st.has_png_jpg = true;
            }
        }
    }
    Ok(())
}

pub fn check_addon(path: String, lang: String) -> Result<OutDto, String> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(tr(&lang, "请先选择有效的素材文件夹", "Choose a valid asset folder first"));
    }
    let l = lang.as_str();
    let mut ok = 0usize;
    let mut warn = 0usize;
    let mut err = 0usize;
    let mut t = String::new();

    let addon_path = root.join("addon.json");
    if addon_path.is_file() {
        match gma::read_addon_json(&addon_path) {
            Ok(a) => {
                ok += 1;
                t.push_str(&format!("[OK] addon.json {}: {}\n", tr(l, "标题", "title"), a.title));
                t.push_str(&format!("[OK] addon.json {}: {}\n", tr(l, "类型", "type"), a.addon_type));
            }
            Err(e) => {
                err += 1;
                t.push_str(&format!("[ERR] addon.json {}: {e}\n", tr(l, "解析失败", "parse failed")));
            }
        }
    } else {
        err += 1;
        t.push_str(&format!("[ERR] {}（打包时会自动生成）\n", tr(l, "缺少 addon.json", "Missing addon.json")));
    }

    let mut st = CheckState {
        files: 0,
        total: 0,
        junk: Vec::new(),
        stray_gma: Vec::new(),
        has_png_jpg: false,
        empty_dirs: 0,
    };
    let _ = check_walk(&root, 0, &mut st);

    t.push_str(&format!(
        "{}: {}  ·  {}: {:.1} {}\n",
        tr(l, "文件数", "Files"),
        st.files,
        tr(l, "总大小", "Total size"),
        st.total as f64 / 1048576.0,
        tr(l, "MB", "MB")
    ));

    if !st.junk.is_empty() {
        warn += 1;
        t.push_str(&format!("[WARN] {}: {}\n", tr(l, "发现系统/缓存垃圾文件", "Found junk/system files"), st.junk.join(", ")));
    }
    if !st.stray_gma.is_empty() {
        err += 1;
        t.push_str(&format!("[ERR] {}（不应嵌套 .gma）\n", tr(l, "素材文件夹内含 .gma", "Folder contains .gma")));
    }
    if st.empty_dirs > 0 {
        warn += 1;
        t.push_str(&format!("[WARN] {}: {}\n", tr(l, "空目录", "Empty directories"), st.empty_dirs));
    }
    if !st.has_png_jpg {
        warn += 1;
        t.push_str(&format!("[WARN] {}（工坊上传需要图标，建议放一张 512×512 预览图）\n", tr(l, "未找到图片预览文件", "No preview image found")));
    }
    if st.files == 0 {
        err += 1;
        t.push_str(&format!("[ERR] {}\n", tr(l, "素材文件夹为空", "Asset folder is empty")));
    }
    t.push_str(&format!("----\n[OK] {} · [WARN] {} · [ERR] {}", ok, warn, err));
    let summary = format!("✅ {} · ⚠️ {} · ❌ {}", ok + st.files + warn + err, warn, err);
    Ok(OutDto {
        status: format!("{}: {}", tr(l, "检查完成", "Checked"), summary),
        text: t,
        item_id: None,
    })
}

// ----------------------------- gmpublish 日志汉化 -----------------------------

pub fn translate_gmpublish_line(raw: &str, lang: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if s.starts_with("gmpublish.exe ") {
        return None;
    }
    if lang == "en" {
        return Some(s.to_string());
    }
    const MAP: &[(&str, &str)] = &[
        ("Garry's Mod Workshop Publisher 1.2", "Garry's Mod 工坊发布器 1.2"),
        ("[Compiled", "[编译于"),
        ("Beginning creation process...", "开始创建流程..."),
        ("Preparing Icon File...", "准备图标文件..."),
        ("Preparing Addon File...", "准备 Addon 文件..."),
        ("Requesting a new Workshop Item ID...", "正在申请新的工坊物品 ID..."),
        ("Success! Your new item ID is", "成功！新物品 ID 为"),
        ("Starting Update...", "开始更新..."),
        ("Updating title...", "更新标题..."),
        ("Updating icon...", "更新图标..."),
        ("Updating Tags...", "更新标签..."),
        ("Updating Addon File...", "更新 Addon 文件..."),
        ("Submitting update.. (this can take a long time)", "提交更新中...（可能需要较长时间）"),
        ("Preparing config...", "准备配置..."),
        ("Preparing content...", "准备内容..."),
        ("Uploading content...", "上传内容..."),
        ("Uploading icon...", "上传图标..."),
        ("Success!", "成功！"),
        ("Addon creation finished.", "Addon 创建完成。"),
        ("Your new addon is currently marked as hidden, add some pictures and a description, then make it public!", "你的新 Addon 当前为隐藏状态：添加图片与描述后即可设为公开。"),
        ("It is located at", "访问地址："),
    ];
    for (en, zh) in MAP {
        if s.contains(en) {
            return Some(s.replace(en, zh));
        }
    }
    Some(s.to_string())
}

// ----------------------------- 外部链接（系统浏览器） -----------------------------

pub fn open_url(url: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::ptr;
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let url_wide: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
        let r = unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                ptr::null(),
                url_wide.as_ptr(),
                ptr::null(),
                ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if (r as isize) <= 32 {
            return Err(format!("打开链接失败: {url}"));
        }
    }
    #[cfg(not(windows))]
    {
        let _ = url;
    }
    Ok(())
}
