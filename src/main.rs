// ============================================================
// GMod 工具箱 — Slint 主程序
// 职责：把 Slint UI 的按钮/拖拽/输入 接到 backend（框架无关层），
//       长耗时操作放后台线程、经 event-loop 回流更新 UI，
//       实现设置持久化与模型查看的软件渲染。
// ============================================================
// 无条件使用 Windows GUI 子系统，构建产物不弹命令行窗口（debug/release 均如此）
#![windows_subsystem = "windows"]

slint::include_modules!();

use backend::{read_addon_json, OutDto, Sink};
use slint::winit_030::{WinitWindowAccessor, CustomApplicationHandler, EventResult};
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer, BackendSelector};
use std::path::Path;
use std::sync::{Arc, Mutex};
use winit; // 直接依赖 winit（完整 event 类型，含 FileDrop 拖放）

mod backend;

// ----------------------------- 国际化字典 -----------------------------
fn i18n(tag: &str, lang: i32) -> String {
    let (zh, en): (&str, &str) = match tag {
        "appTitle" => ("GMod 工具箱", "GMod Tools"),
        "tabMain" => ("打包与上传", "Pack & Upload"),
        "tabTools" => ("其他工具", "Other Tools"),
        "tabAbout" => ("关于", "About"),
        "cardPack" => ("打包 GMA", "Pack GMA"),
        "btnCheck" => ("完整性检查", "Integrity"),
        "btnPack" => ("打包 GMA", "Pack GMA"),
        "lblSource" => ("素材文件夹", "Source folder"),
        "phSource" => ("选择 addon 文件夹（含 addon.json 将自动读取）", "Select addon folder (addon.json auto-loaded)"),
        "lblName" => ("Addon 名称", "Addon name"),
        "phName" => ("addon 标题", "addon title"),
        "lblType" => ("Addon 类型", "Addon type"),
        "lblVer" => ("Addon 版本", "Addon version"),
        "lblSteamId" => ("作者 SteamID", "Author SteamID"),
        "phSteamId" => ("可选，64 位数字", "optional, 64-bit id"),
        "lblTags" => ("标签（逗号）", "Tags (comma)"),
        "phTags" => ("最多 2 个，如 fun, roleplay", "max 2, e.g. fun, roleplay"),
        "lblIgnore" => ("忽略通配符", "Ignore patterns"),
        "phIgnore" => ("如 *.psd, *.svn*, .git, *.tmp", "e.g. *.psd, *.svn*, .git, *.tmp"),
        "lblDesc" => ("描述", "Description"),
        "phDesc" => ("可选", "optional"),
        "lblOut" => ("输出文件名", "Output filename"),
        "phOut" => ("文件名（自动输出到素材文件夹上一级目录）", "filename (auto output to parent of source folder)"),
        "btnBrowse" => ("浏览", "Browse"),
        "cardUpload" => ("上传创意工坊", "Upload to Workshop"),
        "lblGma" => (".gma 文件", ".gma file"),
        "phGma" => ("打包后自动填入", "auto-filled after packing"),
        "lblIcon" => ("图标（jpg）", "Icon (jpg)"),
        "phIcon" => ("工坊展示图标（建议 512×512）", "workshop preview icon (512×512 recommended)"),
        "lblMode" => ("上传方式", "Upload mode"),
        "modeNew" => ("创建新物品", "Create new item"),
        "modeUpdate" => ("更新已有物品", "Update existing item"),
        "lblWid" => ("物品 ID", "Item ID"),
        "phWid" => ("工坊物品 ID（数字）", "workshop item ID (number)"),
        "lblChanges" => ("更新描述（只能英文）", "Update description (English only)"),
        "phChanges" => ("本次更新描述（仅支持英文）", "update description (English only)"),
        "btnUpload" => ("上传工坊", "Upload"),
        "cardUnpack" => ("解包工具", "Unpack"),
        "lblUgma" => ("GMA 文件", "GMA file"),
        "phUgma" => ("选择 .gma 文件", "select a .gma file"),
        "lblUout" => ("解包到", "Extract to"),
        "phUout" => ("选择输出目录", "select output folder"),
        "btnRead" => ("读取信息", "Read info"),
        "btnExtract" => ("开始解包", "Extract"),
        "cardAbout" => ("关于", "About"),
        "aboutSub" => ("GMod 创意工坊打包 · 上传 · 解包工具箱", "GMod Workshop pack · upload · extract toolkit"),
        "aboutTools" => ("备用手动配置", "Manual backup config"),
        "aboutNote" => ("以下为备用手段：当程序无法自动探测到 gmpublish.exe / gmad.exe / studiomdl.exe 时，可在此手动指定路径。", "Backup settings: if auto-detection fails, configure manually here."),
        "lblGmp" => ("gmpublish.exe", "gmpublish.exe"),
        "phGmp" => ("自动探测，无需手动设置", "auto-detected, no manual setup"),
        "lblGmad" => ("gmad.exe", "gmad.exe"),
        "phGmad" => ("GarrysMod\\bin\\gmad.exe（备用）", "GarrysMod\\bin\\gmad.exe (backup)"),
        "btnDetect" => ("自动探测", "Auto-detect"),
        "aboutInfo" => ("应用信息", "App info"),
        "aboutName" => ("名称", "Name"),
        "aboutVer" => ("版本", "Version"),
        "aboutTech" => ("技术", "Tech"),
        "aboutTechVal" => ("Rust + Slint · 单文件自包含", "Rust + Slint · single self-contained exe"),
        "aboutAuthor" => ("作者", "Author"),
        "aboutTeam" => ("团队", "Team"),
        "subVpk" => ("VPK", "VPK"),
        "subVtf" => ("VTF", "VTF"),
        "subMdl" => ("MDL", "MDL"),
        "subView" => ("模型查看", "Model Viewer"),
        "vpkPack" => ("VPK 打包", "Pack VPK"),
        "vpkUnpack" => ("VPK 解包", "Unpack VPK"),
        "lblVpkSrc" => ("源文件夹", "Source folder"),
        "phVpkSrc" => ("选择要打包为 VPK 的文件夹", "folder to pack into a VPK"),
        "lblVpkOut" => ("输出 .vpk", "Output .vpk"),
        "phVpkOut" => ("可留空，自动用源文件夹名", "optional, defaults to source folder name"),
        "lblVpkFile" => ("VPK 文件", "VPK file"),
        "phVpkFile" => ("选择 .vpk 文件", "select a .vpk file"),
        "lblVpkOutdir" => ("解包到", "Extract to"),
        "phVpkOutdir" => ("选择输出目录", "select output folder"),
        "vpkPackBtn" => ("打包 VPK", "Pack VPK"),
        "vpkListBtn" => ("读取列表", "List files"),
        "vpkUnpackBtn" => ("开始解包", "Extract"),
        "vtfConv" => ("VTF → PNG 转换", "VTF → PNG"),
        "lblVtf" => ("VTF 文件", "VTF file"),
        "phVtf" => ("选择 .vtf 纹理文件", "select a .vtf texture"),
        "lblVtfOut" => ("输出 PNG", "Output PNG"),
        "phVtfOut" => ("可留空，自动同目录同名.png", "optional, defaults to same-dir .png"),
        "btnVtfInfo" => ("读取信息", "Read info"),
        "btnVtfConv" => ("转换 PNG", "To PNG"),
        "vtfEncode" => ("PNG → VTF 编码", "PNG → VTF encode"),
        "lblPng" => ("源 PNG 图片", "Source PNG"),
        "phPng" => ("选择 .png（需 2 的幂尺寸，自动适配）", "select a .png (power-of-two, auto-fitted)"),
        "lblVtfOut2" => ("输出 VTF", "Output VTF"),
        "phVtfOut2" => ("可留空，自动同目录同名.vtf", "optional, defaults to same-dir .vtf"),
        "btnVtfEncode" => ("转为 VTF", "To VTF"),
        "lblVtfFormat" => ("编码格式", "Encode format"),
        "vtfEncodeNote" => ("Source 纹理尺寸必须为 2 的幂（如 512×512/1024×1024）；非 2 幂会自动缩放适配。", "Source textures must be power-of-two; non-PoT is auto-resized."),
        "lblMdl" => ("MDL 文件", "MDL file"),
        "phMdl" => ("选择 .mdl 模型文件", "select a .mdl model"),
        "btnMdlInfo" => ("读取信息", "Read info"),
        "mdlUnpack" => ("MDL 解包（.mdl → .smd）", "MDL unpack (.mdl → .smd)"),
        "mdlUnpackBtn" => ("解包为 .smd", "To .smd"),
        "mdlPack" => ("MDL 打包（.qc → .mdl）", "MDL pack (.qc → .mdl)"),
        "mdlPackBtn" => ("打包为 .mdl", "To .mdl"),
        "lblQc" => ("QC 文件", "QC file"),
        "phQc" => ("选择模型 .qc（引用同目录 .smd/.vta 源素材）", "select model .qc (references sibling .smd/.vta)"),
        "lblSmdl" => ("studiomdl.exe", "studiomdl.exe"),
        "phSmdl" => ("自动探测或手动指定", "auto-detect or set manually"),
        "mdlPackNote" => ("通过 studiomdl.exe 编译 .qc；输出位置由 .qc 的 $modelname 决定（相对 GarrysMod 根目录）。", "Compile .qc with studiomdl.exe; output location is set by $modelname in .qc."),
        "mdlNote" => ("解包需同目录配套 .vvd + .vtx（dx90/dx80/sw 自动探测），输出 .smd 到指定目录。", "Unpack needs sibling .vvd + .vtx, outputs a .smd."),
        "btnViewLoad" => ("加载预览", "Load preview"),
        "btnWireframe" => ("线框", "Wireframe"),
        "btnShaded" => ("着色", "Shaded"),
        "btnBones" => ("骨骼", "Bones"),
        "btnTexture" => ("材质", "Texture"),
        "viewOptions" => ("显示选项", "Display"),
        "eggRunLog" => ("运行记录", "Runtime Log"),
        "viewHint" => ("方向键按钮旋转 · ＋/－缩放（软件渲染线框预览）", "Buttons to rotate · zoom (software-rendered wireframe preview)"),
        _ => ("", ""),
    };
    if lang == 1 {
        en.into()
    } else {
        zh.into()
    }
}

// ----------------------------- 控制器 -----------------------------
/// 设置身体组选择槽（0..5）的可见性 / 组名 / 当前子模型名
fn set_bg_slot(ui: &MainWindow, i: usize, visible: bool, name: String, current: String) {
    match i {
        0 => {
            ui.set_bg_name_0(name.into());
            ui.set_bg_current_0(current.into());
            ui.set_bg_visible_0(visible);
        }
        1 => {
            ui.set_bg_name_1(name.into());
            ui.set_bg_current_1(current.into());
            ui.set_bg_visible_1(visible);
        }
        2 => {
            ui.set_bg_name_2(name.into());
            ui.set_bg_current_2(current.into());
            ui.set_bg_visible_2(visible);
        }
        3 => {
            ui.set_bg_name_3(name.into());
            ui.set_bg_current_3(current.into());
            ui.set_bg_visible_3(visible);
        }
        4 => {
            ui.set_bg_name_4(name.into());
            ui.set_bg_current_4(current.into());
            ui.set_bg_visible_4(visible);
        }
        5 => {
            ui.set_bg_name_5(name.into());
            ui.set_bg_current_5(current.into());
            ui.set_bg_visible_5(visible);
        }
        _ => {}
    }
}

/// 本地时间：HH:MM:SS（供独立运行记录时间戳）
fn now_hm() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

/// 本地时间：YYYY-MM-DD HH:MM:SS（供彩蛋窗 LED 跑马灯）
fn now_full() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// 把已加载模型导出为参考 SMD v1（nodes + skeleton + triangles）。
/// 三角面材质按皮肤族重映射；顶点骨骼取首根（参考文件用途，足够外部工具导入）。
fn export_smd(mesh: &backend::MdlMeshView, path: &str, skin_family: u32) -> Result<(), String> {
    let mut out = String::new();
    out.push_str("version 1\nnodes\n");
    for (i, b) in mesh.bones.iter().enumerate() {
        out.push_str(&format!("{} \"{}\" {}\n", i, b.name.replace('"', "'"), b.parent.max(0)));
    }
    out.push_str("end\nskeleton\ntime 0\n");
    for b in &mesh.bones {
        out.push_str(&format!(
            "{} {} {} {} {} {} {}\n",
            b.parent.max(0),
            b.pos[0], b.pos[1], b.pos[2],
            b.rot[0], b.rot[1], b.rot[2]
        ));
    }
    out.push_str("end\ntriangles\n");
    for t in 0..mesh.tri_count as usize {
        let i0 = mesh.indices[t * 3] as usize;
        let i1 = mesh.indices[t * 3 + 1] as usize;
        let i2 = mesh.indices[t * 3 + 2] as usize;
        let mat = if mesh.skins.is_empty() {
            mesh.mat_of_tri.get(t).copied().unwrap_or(-1)
        } else {
            backend::skin_resolve(&mesh.skins, mesh.tri_skinref.get(t).copied().unwrap_or(-1), skin_family)
        };
        let mname = if mat >= 0 && (mat as usize) < mesh.textures.len() {
            mesh.textures[mat as usize].name.clone()
        } else {
            "material".to_string()
        };
        out.push_str(&mname.replace('"', "'"));
        out.push('\n');
        for &vi in &[i0, i1, i2] {
            let b0 = mesh.bone_indices.get(vi).map(|b| b[0].max(0)).unwrap_or(0);
            let p = &mesh.positions[vi * 3..vi * 3 + 3];
            let n = &mesh.normals[vi * 3..vi * 3 + 3];
            let u = mesh.uvs.get(vi * 2).copied().unwrap_or(0.0);
            let v = mesh.uvs.get(vi * 2 + 1).copied().unwrap_or(0.0);
            out.push_str(&format!("{} {} {} {} {} {} {} {} {}\n", b0, p[0], p[1], p[2], n[0], n[1], n[2], u, v));
        }
    }
    out.push_str("end\n");
    std::fs::write(path, out).map_err(|e| e.to_string())
}

struct Ctrl {
    ui: slint::Weak<MainWindow>,
    mesh: Option<backend::MdlMeshView>,
    cam: backend::viewer::Camera,
    /// 每个身体组选中的子模型索引
    body_sel: Vec<i32>,
    /// 当前姿势编辑：选中骨骼 + 欧拉旋转（度）
    pose_bone: i32,
    pose_rot: [f32; 3],
    /// 方向键按住状态（左/右/上/下），按住时降采样快速旋转
    key_held: [bool; 4],
    /// 拖拽渲染节流累计（像素），减少高频 pointer-move 的软件重渲
    drag_accum: f32,
    /// 序列动画播放状态
    anim_seq: i32,          // 选中序列序号（-1 = 未选）
    anim_cache: Vec<Vec<[f32; 3]>>, // 每帧每骨骼旋转增量（度），选中序列解码结果
    anim_frames: usize,     // 当前序列总帧数
    anim_frame: usize,      // 当前播放帧
    anim_playing: bool,     // 是否正在播放（UI 侧 Timer 驱动 view_anim_tick）
    anim_fps: f32,          // 当前序列 fps（HLMV 按序列帧率播放）
    anim_acc: f32,          // 帧推进累计量（单位：帧）
    /// 当前皮肤族（HLMV 皮肤切换）
    skin_family: u32,
    /// 背景色预设索引（0=深灰 1=黑 2=蓝 3=白）
    bg_color: u32,
    /// GMA/VPK 导入：提取目录 + 模型显示名列表 + 当前选中模型文件名
    archive_dir: Option<std::path::PathBuf>,
    archive_models: Vec<String>,
}

impl Ctrl {
    fn new(ui: &MainWindow) -> Self {
        Self {
            ui: ui.as_weak(),
            mesh: None,
            cam: backend::viewer::Camera::default(),
            body_sel: Vec::new(),
            pose_bone: -1,
            pose_rot: [0.0, 0.0, 0.0],
            key_held: [false, false, false, false],
            drag_accum: 0.0,
            anim_seq: -1,
            anim_cache: Vec::new(),
            anim_frames: 0,
            anim_frame: 0,
            anim_playing: false,
            anim_fps: 25.0,
            anim_acc: 0.0,
            skin_family: 0,
            bg_color: 0,
            archive_dir: None,
            archive_models: Vec::new(),
        }
    }

    fn set_field(&self, ui: &MainWindow, id: &str, v: String) {
        let sv = slint::SharedString::from(v);
        match id {
            "source_folder" => ui.set_source_folder(sv.clone()),
            "addon_name" => ui.set_addon_name(sv.clone()),
            "addon_version" => ui.set_addon_version(sv.clone()),
            "steam_id" => ui.set_steam_id(sv.clone()),
            "tags" => ui.set_tags(sv.clone()),
            "ignore" => ui.set_ignore(sv.clone()),
            "description" => ui.set_description(sv.clone()),
            "out_gma" => ui.set_out_gma(sv.clone()),
            "gma_file" => ui.set_gma_file(sv.clone()),
            "icon_path" => ui.set_icon_path(sv.clone()),
            "workshop_id" => ui.set_workshop_id(sv.clone()),
            "changes" => ui.set_changes(sv.clone()),
            "extract_gma" => ui.set_extract_gma(sv.clone()),
            "extract_dir" => ui.set_extract_dir(sv.clone()),
            "vpk_src" => ui.set_vpk_src(sv.clone()),
            "vpk_out" => ui.set_vpk_out(sv.clone()),
            "vpk_file" => ui.set_vpk_file(sv.clone()),
            "vpk_dir" => ui.set_vpk_dir(sv.clone()),
            "vtf_file" => {
                ui.set_vtf_file(sv.clone());
                refresh_vtf_preview(self.ui.clone());
            }
            "vtf_out" => ui.set_vtf_out(sv.clone()),
            "png_file" => {
                ui.set_png_file(sv.clone());
                refresh_png_preview(self.ui.clone());
            }
            "vtf_enc_out" => ui.set_vtf_enc_out(sv.clone()),
            "mdl_file" => ui.set_mdl_file(sv.clone()),
            "mdl_out_dir" => ui.set_mdl_out_dir(sv.clone()),
            "qc_file" => ui.set_qc_file(sv.clone()),
            "mdl_pack_out" => ui.set_mdl_pack_out(sv.clone()),
            "studiomdl_about" => ui.set_studiomdl(sv.clone()),
            "view_mdl_file" => ui.set_view_mdl_file(sv.clone()),
            "gmpublish" => ui.set_gmpublish(sv.clone()),
            "gmad" => ui.set_gmad(sv),
            _ => {}
        }
    }

    fn browse(&self, ui: &MainWindow, field_id: &str) {
        let lang = ui.global::<I18n>().get_lang();
        let zh = lang == 0;
        let (role, title): (&str, &str) = match field_id {
            "source_folder" | "extract_dir" | "vpk_src" | "vpk_dir" | "mdl_out_dir" | "mdl_pack_out" => ("folder", if zh { "选择文件夹" } else { "Select folder" }),
            "out_gma" => ("save", if zh { "选择输出位置" } else { "Select output" }),
            "gmpublish" | "gmad" | "studiomdl_about" => ("exe", if zh { "选择 exe" } else { "Select exe" }),
            "gma_file" | "extract_gma" => ("gma", if zh { "选择 .gma" } else { "Select .gma" }),
            "icon_path" => ("icon", if zh { "选择图标" } else { "Select icon" }),
            "vpk_out" => ("vpksave", if zh { "选择输出 .vpk" } else { "Select output .vpk" }),
            "vpk_file" => ("vpk", if zh { "选择 .vpk" } else { "Select .vpk" }),
            "vtf_file" => ("vtf", if zh { "选择 .vtf" } else { "Select .vtf" }),
            "png_file" => ("png", if zh { "选择 .png" } else { "Select .png" }),
            "vtf_out" => ("pngsave", if zh { "选择输出 .png" } else { "Select output .png" }),
            "vtf_enc_out" => ("vtfsave", if zh { "选择输出 .vtf" } else { "Select output .vtf" }),
            "mdl_file" | "view_mdl_file" => ("mdl", if zh { "选择 .mdl" } else { "Select .mdl" }),
            "qc_file" => ("qc", if zh { "选择 .qc" } else { "Select .qc" }),
            _ => ("folder", if zh { "选择" } else { "Select" }),
        };
        if let Some(p) = backend::browse(role.to_string(), title.to_string()) {
            self.set_field(ui, field_id, p.clone());
            if field_id == "source_folder" {
                self.auto_fill_folder(ui);
            }
            // mdl_file 同时联动模型查看字段
            if field_id == "mdl_file" && ui.get_view_mdl_file().is_empty() {
                ui.set_view_mdl_file(p.clone().into());
            }
            // gma_file 联动解包字段
            if field_id == "gma_file" && ui.get_extract_gma().is_empty() {
                ui.set_extract_gma(p.clone().into());
            }
        }
    }

    fn auto_fill_folder(&self, ui: &MainWindow) {
        let folder = ui.get_source_folder();
        let lang = ui.global::<I18n>().get_lang();
        let zh = lang == 0;
        ui.set_log_text("".into());
        if let Some(meta) = backend::read_addon_json(folder.to_string()) {
            if !meta.title.is_empty() {
                ui.set_addon_name(meta.title.into());
            }
            if !meta.addon_type.is_empty() {
                let list = ["ServerContent", "gamemode", "map", "weapon", "vehicle", "npc", "tool", "effects", "model"];
                if let Some(i) = list.iter().position(|t| *t == meta.addon_type) {
                    ui.set_addon_type_index(i as i32);
                }
            }
            if !meta.tags.is_empty() {
                ui.set_tags(meta.tags.join(", ").into());
            }
            if !meta.ignore.is_empty() {
                ui.set_ignore(meta.ignore.join(", ").into());
            }
            self.toast(ui, if zh { "已读取 addon.json，自动补填 Addon 信息" } else { "addon.json loaded, fields auto-filled" }, "info");
            ui.set_status((if zh { "已自动读取 addon.json 并补填" } else { "addon.json loaded and fields filled" }).into());
        } else {
            self.toast(ui, if zh { "素材目录中未找到 addon.json，打包时将自动生成" } else { "No addon.json found; it will be created on pack" }, "info");
            ui.set_status((if zh { "未找到 addon.json，打包时将自动创建" } else { "No addon.json found; it will be created on pack" }).into());
        }
        if ui.get_addon_name().is_empty() {
            let parts: Vec<&str> = folder.trim_end_matches(['\\', '/']).split(['\\', '/']).collect();
            if let Some(n) = parts.last() {
                ui.set_addon_name(n.to_string().into());
            }
        }
        if ui.get_out_gma().is_empty() && !ui.get_addon_name().is_empty() {
            ui.set_out_gma(format!("{}.gma", ui.get_addon_name()).into());
        }
    }

    fn log(&self, ui: &MainWindow, line: &str) {
        let mut t = ui.get_log_text().to_string();
        if !t.is_empty() {
            t.push('\n');
        }
        t.push_str(line);
        ui.set_log_text(t.into());
        // 独立运行记录（带时间戳，供关于页彩蛋窗显示）
        let mut r = ui.get_run_log().to_string();
        if !r.is_empty() {
            r.push('\n');
        }
        r.push_str(&format!("[{}] {}", now_hm(), line));
        ui.set_run_log(r.into());
    }

    /// 轻提示：右下角深色胶囊显示状态（非日志抽屉），2200ms 自动隐藏。
    /// 用于校验结果 / 完成 / 错误等无需展开日志抽屉的操作。
    fn toast(&self, ui: &MainWindow, text: &str, kind: &str) {
        Self::toast_ui(ui, text, kind);
    }

    /// 静态轻提示（供 run_simple 完成回调等无 self 场景调用）
    fn toast_ui(ui: &MainWindow, text: &str, kind: &str) {
        ui.set_toast_text(text.into());
        ui.set_toast_kind(kind.into());
        ui.set_toast_show(true);
        let weak = ui.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(2200), move || {
            if let Some(u) = weak.upgrade() {
                u.set_toast_show(false);
            }
        });
    }

    /// 解析流式消息 P/S/L/E/D 并更新 UI。
    fn handle_msg(ui: &MainWindow, msg: &str) {
        if msg == "D" {
            ui.set_busy(false);
            ui.set_progress_visible(false);
            ui.set_progress(0.0);
            let zh = ui.global::<I18n>().get_lang() == 0;
            ui.set_status((if zh { "就绪" } else { "Ready" }).into());
            // 流式任务完成：toast 轻提示（日志仍保留在抽屉可看）
            Ctrl::toast_ui(ui, if zh { "操作完成" } else { "Done" }, "success");
            return;
        }
        if let Some(p) = msg.strip_prefix("P|") {
            if let Ok(f) = p.parse::<f32>() {
                ui.set_progress(f.clamp(0.0, 1.0));
            }
            return;
        }
        if let Some(s) = msg.strip_prefix("S|") {
            ui.set_status(s.into());
            return;
        }
        if let Some(e) = msg.strip_prefix("E|") {
            let mut t = ui.get_log_text().to_string();
            if !t.is_empty() {
                t.push('\n');
            }
            t.push_str(e);
            ui.set_log_text(t.into());
            Ctrl::toast_ui(ui, e, "error");
            return;
        }
        if let Some(l) = msg.strip_prefix("L|") {
            let mut t = ui.get_log_text().to_string();
            if !t.is_empty() {
                t.push('\n');
            }
            t.push_str(l);
            ui.set_log_text(t.into());
        }
    }

    /// 后台线程跑流式任务，消息经 event-loop 回流。
    fn run_stream<F>(&self, f: F)
    where
        F: FnOnce(&mut Sink) + Send + 'static,
    {
        let weak = self.ui.clone();
        if let Some(ui) = weak.upgrade() {
            ui.set_progress_visible(true);
            ui.set_progress(0.0);
            ui.set_busy(true);
            ui.set_log_text("".into());
        }
        std::thread::spawn(move || {
            let mut sink = backend::sink({
                let weak = weak.clone();
                move |msg| {
                    let weak = weak.clone();
                    let _ = weak.upgrade_in_event_loop(move |ui| Ctrl::handle_msg(&ui, &msg));
                }
            });
            f(&mut sink);
        });
    }

    /// 后台线程跑简单任务，结果一次性回流。
    fn run_simple<F>(&self, f: F)
    where
        F: FnOnce() -> Result<OutDto, String> + Send + 'static,
    {
        let weak = self.ui.clone();
        if let Some(ui) = weak.upgrade() {
            ui.set_busy(true);
            ui.set_log_text("".into());
        }
        std::thread::spawn(move || {
            let r = f();
            let weak = weak.clone();
            let _ = weak.upgrade_in_event_loop(move |ui| {
                match r {
                    Ok(o) => {
                        ui.set_busy(false);
                        if !o.status.is_empty() {
                            ui.set_status(o.status.clone().into());
                        }
                        if !o.text.is_empty() {
                            let mut t = ui.get_log_text().to_string();
                            if !t.is_empty() {
                                t.push('\n');
                            }
                            t.push_str(&o.text);
                            ui.set_log_text(t.into());
                        }
                        // 非日志抽屉：完成态用 toast 轻提示
                        let tip = if !o.status.is_empty() {
                            o.status.clone()
                        } else if !o.text.is_empty() {
                            o.text.clone()
                        } else {
                            String::new()
                        };
                        if !tip.is_empty() {
                            Ctrl::toast_ui(&ui, &tip, "success");
                        }
                    }
                    Err(e) => {
                        ui.set_busy(false);
                        ui.set_status(e.clone().into());
                        let mut t = ui.get_log_text().to_string();
                        if !t.is_empty() {
                            t.push('\n');
                        }
                        t.push_str(&e);
                        ui.set_log_text(t.into());
                        Ctrl::toast_ui(&ui, &e, "error");
                    }
                }
            });
        });
    }

    fn pack(&self, ui: &MainWindow) {
        let lang = ui.global::<I18n>().get_lang();
        let zh = lang == 0;
        let source = ui.get_source_folder();
        let out_name = ui.get_out_gma();
        if source.trim().is_empty() {
            self.toast(ui, if zh { "请先选择素材文件夹" } else { "Choose an asset folder first" }, "error");
            return;
        }
        if out_name.trim().is_empty() {
            self.toast(ui, if zh { "请先填写输出文件名" } else { "Please set output filename" }, "error");
            return;
        }
        // 计算输出路径：素材文件夹上一级目录 + 文件名
        let src_trim = source.trim_end_matches(['\\', '/']);
        let parent = match src_trim.rfind(['\\', '/']) {
            Some(i) => &src_trim[..i],
            None => src_trim,
        };
        let fn0 = out_name.trim();
        let fname = if fn0.to_lowercase().ends_with(".gma") { fn0.to_string() } else { format!("{fn0}.gma") };
        let out = format!("{}\\{}", parent.trim_end_matches(['\\', '/']), fname);

        let meta = backend::MetaDto {
            addon_name: ui.get_addon_name().to_string(),
            addon_type: {
                let list = ["ServerContent", "gamemode", "map", "weapon", "vehicle", "npc", "tool", "effects", "model"];
                list.get(ui.get_addon_type_index() as usize).copied().unwrap_or("ServerContent").to_string()
            },
            tags: ui.get_tags().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            ignore: ui.get_ignore().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            description: ui.get_description().to_string(),
            addon_version: ui.get_addon_version().parse::<i64>().unwrap_or(1),
            steam_id: ui.get_steam_id().to_string(),
        };
        let gma_out = out.clone();
        let weak = self.ui.clone();
        ui.set_log_open(true); // 打包为详细流程，展开日志抽屉看进度
        self.run_stream(move |sink| {
            let r = backend::pack(source.to_string(), meta, out, (if zh { "zh" } else { "en" }).to_string(), sink);
            let _ = weak.upgrade_in_event_loop(move |ui| {
                if let Ok(o) = r {
                    ui.set_gma_file(o.gma.clone().into());
                    ui.set_status(o.status.clone().into());
                }
            });
            let _ = gma_out;
        });
    }

    fn upload(&self, ui: &MainWindow) {
        let lang = ui.global::<I18n>().get_lang();
        let zh = lang == 0;
        let mode = if ui.get_upload_mode() == 1 { "update" } else { "create" };
        // ASCII 检查
        let mut bad: Vec<String> = Vec::new();
        let mut check = |v: &str, label: &str| {
            if !v.is_ascii() {
                bad.push(label.to_string());
            }
        };
        check(&ui.get_changes(), if zh { "更新描述" } else { "Update description" });
        check(&ui.get_addon_name(), if zh { "标题" } else { "Title" });
        check(&ui.get_description(), if zh { "描述" } else { "Description" });
        check(&ui.get_tags(), if zh { "标签" } else { "Tags" });
        if !bad.is_empty() {
            self.toast(ui, &format!("{}: {}", if zh { "gmpublish 只支持英文（ASCII），请改为英文后重试" } else { "gmpublish only supports English (ASCII), please fix" }, bad.join(", ")), "error");
            return;
        }
        let args = backend::UploadArgsDto {
            gmpublish: ui.get_gmpublish().to_string(),
            gma: ui.get_gma_file().to_string(),
            icon: {
                let v = ui.get_icon_path();
                if v.is_empty() { None } else { Some(v.to_string()) }
            },
            workshop_id: {
                let v = ui.get_workshop_id();
                if v.is_empty() { None } else { Some(v.to_string()) }
            },
            changes: {
                let v = ui.get_changes();
                if v.is_empty() { None } else { Some(v.to_string()) }
            },
            mode: mode.to_string(),
            lang: if zh { "zh".to_string() } else { "en".to_string() },
        };
        let weak = self.ui.clone();
        ui.set_log_open(true); // 上传为详细流程，展开日志抽屉看进度
        self.run_stream(move |sink| {
            let r = backend::upload(args, sink);
            let _ = weak.upgrade_in_event_loop(move |ui| {
                if let Ok(o) = r {
                    if let Some(id) = &o.item_id {
                        ui.set_workshop_id(id.clone().into());
                        ui.set_upload_mode(1);
                        let mut t = ui.get_log_text().to_string();
                        if !t.is_empty() {
                            t.push('\n');
                        }
                        t.push_str(&format!("{}: {}", if ui.global::<I18n>().get_lang() == 0 { "已获取工坊物品 ID，已切换为「更新已有物品」" } else { "Workshop item ID acquired, switched to Update" }, id));
                        ui.set_log_text(t.into());
                    }
                }
            });
        });
    }

    fn extract(&self, ui: &MainWindow) {
        let lang = ui.global::<I18n>().get_lang();
        let zh = lang == 0;
        let p = ui.get_extract_gma();
        let out = ui.get_extract_dir();
        if p.trim().is_empty() || out.trim().is_empty() {
            self.toast(ui, if zh { "请选择 .gma 与输出目录" } else { "Select .gma and output folder" }, "error");
            return;
        }
        ui.set_log_open(true); // 解包为详细流程，展开日志抽屉看进度
        self.run_stream(move |sink| {
            let _ = backend::extract(p.to_string(), out.to_string(), (if zh { "zh" } else { "en" }).to_string(), sink);
        });
    }

    fn mdl_pack(&self, ui: &MainWindow) {
        let lang = ui.global::<I18n>().get_lang();
        let zh = lang == 0;
        let qc = ui.get_qc_file();
        let sm = ui.get_studiomdl();
        let out = ui.get_mdl_pack_out();
        if qc.trim().is_empty() {
            self.toast(ui, if zh { "请先选择 .qc 文件" } else { "Select a .qc file first" }, "error");
            return;
        }
        if sm.trim().is_empty() {
            self.toast(ui, if zh { "请先设置 studiomdl.exe（工具页或关于页）" } else { "Set studiomdl.exe first" }, "error");
            return;
        }
        ui.set_log_open(true); // 模型打包为详细流程，展开日志抽屉看进度
        self.run_stream(move |sink| {
            let _ = backend::mdl_pack(qc.to_string(), sm.to_string(), out.to_string(), (if zh { "zh" } else { "en" }).to_string(), sink);
        });
    }

    fn view_load(&mut self, ui: &MainWindow) {
        let lang = ui.global::<I18n>().get_lang();
        let zh = lang == 0;
        let p = ui.get_view_mdl_file();
        if p.trim().is_empty() {
            self.toast(ui, if zh { "请先选择 .mdl 文件" } else { "Select a .mdl file first" }, "info");
            return;
        }
        // 若未初始化 body_sel，先按各身体组 0 号子模型初始化
        if self.body_sel.is_empty() {
            self.body_sel = vec![0; 0];
        }
        ui.set_status((if zh { "正在加载模型预览..." } else { "Loading model preview..." }).into());
        let l = if zh { "zh" } else { "en" };
        let body_sel = self.body_sel.clone();
        match backend::mdl_mesh_data(p.to_string(), body_sel, l.to_string()) {
            Ok(mesh) => {
                let b = mesh.bounds;
                let cx = (b[0] + b[3]) / 2.0;
                let cy = (b[1] + b[4]) / 2.0;
                let cz = (b[2] + b[5]) / 2.0;
                let r = (b[3] - b[0]).max(b[4] - b[1]).max(b[5] - b[2]).max(0.001) / 2.0;
                self.cam = backend::viewer::Camera {
                    yaw: 0.7,
                    pitch: -0.35,
                    dist: r * 3.2,
                    center: [cx, cy, cz],
                    pan: [0.0, 0.0, 0.0],
                };
                let meta = format!(
                    "{} {} · {} {} · {} {} · {} {}",
                    mesh.tri_count,
                    if zh { "三角面" } else { "tris" },
                    mesh.positions.len() / 3,
                    if zh { "顶点" } else { "verts" },
                    mesh.bones.len(),
                    if zh { "骨骼" } else { "bones" },
                    mesh.textures.len(),
                    if zh { "贴图" } else { "tex" }
                );
                ui.set_view_meta(meta.into());
                // 底部居中文件名（去路径）
                let fname = p.rsplit(['\\', '/']).next().unwrap_or("").to_string();
                ui.set_view_mdl_name(fname.into());
                // 身体组选择槽
                let ngroups = mesh.bodygroups.len().min(6);
                for i in 0..6 {
                    if i < ngroups {
                        let bg = &mesh.bodygroups[i];
                        let sel = self.body_sel.get(i).copied().unwrap_or(0);
                        let cur = bg.models.get(sel as usize).cloned().unwrap_or_default();
                        set_bg_slot(ui, i, true, bg.name.clone(), cur);
                    } else {
                        set_bg_slot(ui, i, false, String::new(), String::new());
                    }
                }
                // 统计面板文本
                let mut stats = String::new();
                stats.push_str(&format!("—— {} ——\n", if zh { "序列" } else { "Sequences" }));
                if mesh.sequences.is_empty() {
                    stats.push_str(&format!("{}: 0\n", if zh { "无序列" } else { "none" }));
                } else {
                    for s in &mesh.sequences {
                        stats.push_str(&format!("• {}  ({} {}{})\n", s.name, s.frames, s.fps, if zh { " fps" } else { " fps" }));
                    }
                }
                stats.push_str(&format!("\n—— {} ——\n", if zh { "身体组" } else { "Body groups" }));
                for (i, bg) in mesh.bodygroups.iter().enumerate() {
                    let sel = self.body_sel.get(i).copied().unwrap_or(0);
                    stats.push_str(&format!("• {}: {}/{}\n", bg.name, sel + 1, bg.models.len()));
                }
                stats.push_str(&format!("\n—— {} ——\n", if zh { "命中框" } else { "Hitboxes" }));
                if mesh.hitboxes.is_empty() {
                    stats.push_str(&format!("{}: 0\n", if zh { "无" } else { "none" }));
                } else {
                    for hb in &mesh.hitboxes {
                        stats.push_str(&format!("• {} (bone {})\n", hb.name, hb.bone));
                    }
                }
                stats.push_str(&format!("\n—— {} ——\n", if zh { "附着点" } else { "Attachments" }));
                if mesh.attachments.is_empty() {
                    stats.push_str(&format!("{}: 0\n", if zh { "无" } else { "none" }));
                } else {
                    for at in &mesh.attachments {
                        stats.push_str(&format!("• {} (bone {})\n", at.name, at.bone));
                    }
                }
                stats.push_str(&format!("\n—— {} ——\n", if zh { "材质" } else { "Materials" }));
                for t in &mesh.textures {
                    let ok = if t.path.is_some() {
                        if zh { "已加载" } else { "loaded" }.to_string()
                    } else {
                        if zh { "未找到贴图" } else { "no texture" }.to_string()
                    };
                    stats.push_str(&format!("• {} [{}]\n", t.name, ok));
                }
                stats.push_str(&format!(
                    "\n—— {} ——\n{}: {}\n",
                    if zh { "皮肤族" } else { "Skins" },
                    if zh { "皮肤族数量" } else { "skin families" },
                    mesh.skins.len()
                ));
                ui.set_view_stats(stats.into());
                // 皮肤族（HLMV 皮肤切换）
                self.skin_family = 0;
                ui.set_view_skin_family(0);
                let skin_count = mesh.skins.len().max(1) as i32;
                ui.set_view_skin_count(skin_count);
                let skin_names: Vec<slint::SharedString> = (0..skin_count)
                    .map(|i| format!("{} #{}", if zh { "皮肤" } else { "Skin" }, i + 1).into())
                    .collect();
                let snm: slint::ModelRc<slint::SharedString> =
                    slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(skin_names)));
                ui.set_view_skin_names(snm);
                // 姿势编辑器骨骼选择
                if !mesh.bones.is_empty() {
                    self.pose_bone = 0;
                    ui.set_pose_bone_index(0);
                    ui.set_pose_bone_name(mesh.bones[0].name.clone().into());
                    ui.set_pose_bone_has(true);
                } else {
                    self.pose_bone = -1;
                    ui.set_pose_bone_has(false);
                }
                self.pose_rot = [0.0, 0.0, 0.0];
                ui.set_pose_rot_x(0.0);
                ui.set_pose_rot_y(0.0);
                ui.set_pose_rot_z(0.0);
                // 重置序列动画状态，并把序列名列表写入 UI
                self.anim_seq = -1;
                self.anim_cache = Vec::new();
                self.anim_frames = 0;
                self.anim_frame = 0;
                self.anim_playing = false;
                ui.set_anim_seq(-1);
                ui.set_anim_frames(0);
                ui.set_anim_frame(0.0);
                ui.set_anim_playing(false);
                ui.set_seq_open(false);
                let seq_names: Vec<slint::SharedString> = mesh.sequences.iter().map(|s| s.name.clone().into()).collect();
                let seq_model: slint::ModelRc<slint::SharedString> =
                    slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(seq_names)));
                ui.set_seq_names(seq_model);
                self.mesh = Some(mesh);
                self.redraw_view(ui);
                ui.set_status((if zh { "模型预览已加载" } else { "Model preview loaded" }).into());
            }
            Err(e) => {
                ui.set_status((if zh { "加载失败" } else { "Load failed" }).into());
                self.toast(ui, &e, "error");
            }
        }
    }

    fn redraw_view(&self, ui: &MainWindow) {
        let params = backend::viewer::RenderParams {
            mode: self.render_mode(ui),
            show_bones: ui.get_view_bone(),
            show_hitboxes: ui.get_view_hitbox(),
            show_attachments: ui.get_view_attach(),
            show_grid: ui.get_view_grid(),
            show_axes: ui.get_view_axes(),
            show_normals: ui.get_view_normal(),
            show_bounds: ui.get_view_bounds(),
            skin_family: self.skin_family,
            fov: ui.get_view_fov().clamp(5, 120) as f32 * std::f32::consts::PI / 180.0,
            ortho: ui.get_view_ortho(),
            bg: self.view_bg(ui),
            w: 1080,
            h: 480,
        };
        self.render_to_ui(ui, &params);
    }

    /// 当前渲染模式（HLMV 对齐：UV > 线框 > 平面 > 全亮 > 贴图 > 平滑）。
    fn render_mode(&self, ui: &MainWindow) -> backend::viewer::RenderMode {
        if ui.get_view_uv() {
            backend::viewer::RenderMode::UV
        } else if ui.get_view_wire() {
            backend::viewer::RenderMode::Wire
        } else if ui.get_view_flat() {
            backend::viewer::RenderMode::Flat
        } else if ui.get_view_fullbright() {
            backend::viewer::RenderMode::Fullbright
        } else if ui.get_view_texture() {
            backend::viewer::RenderMode::Textured
        } else {
            backend::viewer::RenderMode::Shaded
        }
    }

    /// 把渲染结果写入 UI（按当前姿势 / 序列动画帧）。
    fn render_to_ui(&self, ui: &MainWindow, params: &backend::viewer::RenderParams) {
        if let Some(mesh) = &self.mesh {
            let pose = self.render_pose(mesh);
            let (w, h, rgba) = backend::viewer::render_preview(mesh, &self.cam, params, pose.as_deref());
            let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&rgba, w, h);
            ui.set_view_preview_image(Image::from_rgba8(buffer));
        }
    }

    /// 低分辨率快速重渲（按住方向键/拖拽时调用，保证流畅）。
    fn redraw_view_lowres(&self, ui: &MainWindow) {
        let params = backend::viewer::RenderParams {
            mode: self.render_mode(ui),
            show_bones: ui.get_view_bone(),
            show_hitboxes: ui.get_view_hitbox(),
            show_attachments: ui.get_view_attach(),
            show_grid: ui.get_view_grid(),
            show_axes: ui.get_view_axes(),
            show_normals: ui.get_view_normal(),
            show_bounds: ui.get_view_bounds(),
            skin_family: self.skin_family,
            fov: ui.get_view_fov().clamp(5, 120) as f32 * std::f32::consts::PI / 180.0,
            ortho: ui.get_view_ortho(),
            bg: self.view_bg(ui),
            w: 560,
            h: 250,
        };
        self.render_to_ui(ui, &params);
    }

    /// 渲染用背景色（HLMV 背景预设：0=跟随主题 1=黑 2=深蓝 3=白）。
    fn view_bg(&self, ui: &MainWindow) -> [u8; 3] {
        match self.bg_color {
            1 => [0, 0, 0],
            2 => [18, 26, 44],
            3 => [255, 255, 255],
            _ => {
                if ui.get_dark() {
                    [16, 20, 26]
                } else {
                    [245, 247, 250]
                }
            }
        }
    }

    /// 导入 GMA/VPK：解压到临时缓存 → 扫描 .mdl → 写入模型列表并加载第一个。
    fn import_archive(&mut self, ui: &MainWindow, path: &str) {
        let zh = ui.global::<I18n>().get_lang() == 0;
        let cache = std::env::temp_dir().join("gma_slint_model_cache");
        let _ = std::fs::remove_dir_all(&cache); // 仅清理由本工具创建的缓存目录
        let _ = std::fs::create_dir_all(&cache);
        let mut sink = backend::sink(|_msg| {});
        match backend::extract_archive_all(path, &cache, if zh { "zh" } else { "en" }, &mut sink) {
            Ok(_) => {
                let models = backend::find_models(&cache);
                if models.is_empty() {
                    self.toast(ui, &(if zh { "档案内未找到 .mdl 模型" } else { "No .mdl found in archive" }), "error");
                    return;
                }
                self.archive_dir = Some(cache);
                self.archive_models = models.clone();
                let names: Vec<slint::SharedString> = models.iter().map(|m| m.clone().into()).collect();
                ui.set_view_model_names(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(names))));
                // 自动加载第一个模型
                let first = self.archive_dir.as_ref().unwrap().join(&models[0]);
                self.set_field(ui, "view_mdl_file", first.to_string_lossy().into_owned());
                self.view_load(ui);
                ui.set_status(
                    (if zh {
                        format!("已导入 {} 个模型，加载第一个", models.len())
                    } else {
                        format!("Imported {} models, loaded first", models.len())
                    })
                    .into(),
                );
            }
            Err(e) => self.toast(ui, &e, "error"),
        }
    }

    /// 渲染用姿势：有序列动画且已解码时优先用当前动画帧，否则用姿势编辑器覆盖。
    fn render_pose(&self, mesh: &backend::MdlMeshView) -> Option<Vec<[f32; 3]>> {
        if !self.anim_cache.is_empty() {
            let f = self.anim_frame.min(self.anim_cache.len().saturating_sub(1));
            return Some(self.anim_cache[f].clone());
        }
        self.current_pose(mesh)
    }

    /// 当前姿势：选中骨骼及其子树按 pose_rot 绕骨骼关节点旋转（度）。返回每骨骼覆盖欧拉。
    fn current_pose(&self, mesh: &backend::MdlMeshView) -> Option<Vec<[f32; 3]>> {
        if self.pose_bone < 0 {
            return None;
        }
        let idx = self.pose_bone as usize;
        if idx >= mesh.bones.len() {
            return None;
        }
        let r = self.pose_rot;
        if r[0].abs() < 0.05 && r[1].abs() < 0.05 && r[2].abs() < 0.05 {
            return None;
        }
        // 收集子树（含自身）
        let mut subtree = vec![idx];
        let mut i = 0;
        while i < subtree.len() {
            let cur = subtree[i];
            for (bi, bn) in mesh.bones.iter().enumerate() {
                if bn.parent == cur as i32 && !subtree.contains(&bi) {
                    subtree.push(bi);
                }
            }
            i += 1;
        }
        let mut overrides = vec![[0.0f32, 0.0, 0.0]; mesh.bones.len()];
        for &bi in &subtree {
            overrides[bi] = r;
        }
        Some(overrides)
    }

    fn save_settings(&self, ui: &MainWindow) {
        #[derive(serde::Serialize)]
        struct S {
            source_folder: String,
            addon_name: String,
            tags: String,
            ignore: String,
            description: String,
            steam_id: String,
            out_gma: String,
            gma_file: String,
            icon_path: String,
            workshop_id: String,
            changes: String,
            extract_gma: String,
            extract_dir: String,
            gmpublish: String,
            gmad: String,
            studiomdl: String,
            addon_version: String,
            addon_type_index: i32,
            mode: i32,
            dark: bool,
            lang: i32,
            view_wire: bool,
            view_shade: bool,
            view_bone: bool,
            view_texture: bool,
            view_hitbox: bool,
            view_attach: bool,
            view_grid: bool,
            view_axes: bool,
            view_ortho: bool,
            view_uv: bool,
            view_flat: bool,
            view_fullbright: bool,
            view_normal: bool,
            view_bounds: bool,
            view_skin_family: i32,
            view_fov: i32,
        }
        let s = S {
            source_folder: ui.get_source_folder().to_string(),
            addon_name: ui.get_addon_name().to_string(),
            tags: ui.get_tags().to_string(),
            ignore: ui.get_ignore().to_string(),
            description: ui.get_description().to_string(),
            steam_id: ui.get_steam_id().to_string(),
            out_gma: ui.get_out_gma().to_string(),
            gma_file: ui.get_gma_file().to_string(),
            icon_path: ui.get_icon_path().to_string(),
            workshop_id: ui.get_workshop_id().to_string(),
            changes: ui.get_changes().to_string(),
            extract_gma: ui.get_extract_gma().to_string(),
            extract_dir: ui.get_extract_dir().to_string(),
            gmpublish: ui.get_gmpublish().to_string(),
            gmad: ui.get_gmad().to_string(),
            studiomdl: ui.get_studiomdl().to_string(),
            addon_version: ui.get_addon_version().to_string(),
            addon_type_index: ui.get_addon_type_index(),
            mode: ui.get_upload_mode(),
            dark: ui.get_dark(),
            lang: ui.global::<I18n>().get_lang(),
            view_wire: ui.get_view_wire(),
            view_shade: ui.get_view_shade(),
            view_bone: ui.get_view_bone(),
            view_texture: ui.get_view_texture(),
            view_hitbox: ui.get_view_hitbox(),
            view_attach: ui.get_view_attach(),
            view_grid: ui.get_view_grid(),
            view_axes: ui.get_view_axes(),
            view_ortho: ui.get_view_ortho(),
            view_uv: ui.get_view_uv(),
            view_flat: ui.get_view_flat(),
            view_fullbright: ui.get_view_fullbright(),
            view_normal: ui.get_view_normal(),
            view_bounds: ui.get_view_bounds(),
            view_skin_family: self.skin_family as i32,
            view_fov: ui.get_view_fov(),
        };
        let dir = settings_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(json) = serde_json::to_string_pretty(&s) {
            let _ = std::fs::write(dir.join("settings.json"), json);
        }
    }

    fn load_settings(&mut self, ui: &MainWindow) {
        #[derive(serde::Deserialize)]
        struct S {
            source_folder: Option<String>,
            addon_name: Option<String>,
            tags: Option<String>,
            ignore: Option<String>,
            description: Option<String>,
            steam_id: Option<String>,
            out_gma: Option<String>,
            gma_file: Option<String>,
            icon_path: Option<String>,
            workshop_id: Option<String>,
            changes: Option<String>,
            extract_gma: Option<String>,
            extract_dir: Option<String>,
            gmpublish: Option<String>,
            gmad: Option<String>,
            studiomdl: Option<String>,
            addon_version: Option<String>,
            addon_type_index: Option<i32>,
            mode: Option<i32>,
            dark: Option<bool>,
            lang: Option<i32>,
            view_wire: Option<bool>,
            view_shade: Option<bool>,
            view_bone: Option<bool>,
            view_texture: Option<bool>,
            view_hitbox: Option<bool>,
            view_attach: Option<bool>,
            view_grid: Option<bool>,
            view_axes: Option<bool>,
            view_ortho: Option<bool>,
            view_uv: Option<bool>,
            view_flat: Option<bool>,
            view_fullbright: Option<bool>,
            view_normal: Option<bool>,
            view_bounds: Option<bool>,
            view_skin_family: Option<i32>,
            view_fov: Option<i32>,
        }
        let path = settings_dir().join("settings.json");
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return,
        };
        let s: S = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return,
        };
        let apply = |id: &str, v: Option<String>| {
            if let Some(v) = v {
                self.set_field(ui, id, v);
            }
        };
        apply("source_folder", s.source_folder);
        apply("addon_name", s.addon_name);
        apply("tags", s.tags);
        apply("ignore", s.ignore);
        apply("description", s.description);
        apply("steam_id", s.steam_id);
        apply("out_gma", s.out_gma);
        apply("gma_file", s.gma_file);
        apply("icon_path", s.icon_path);
        apply("workshop_id", s.workshop_id);
        apply("changes", s.changes);
        apply("extract_gma", s.extract_gma);
        apply("extract_dir", s.extract_dir);
        apply("gmpublish", s.gmpublish);
        apply("gmad", s.gmad);
        apply("studiomdl_about", s.studiomdl);
        apply("addon_version", s.addon_version);
        if let Some(i) = s.addon_type_index {
            ui.set_addon_type_index(i);
        }
        if let Some(m) = s.mode {
            ui.set_upload_mode(m);
        }
        if let Some(d) = s.dark {
            ui.set_dark(d);
        }
        if let Some(l) = s.lang {
            ui.global::<I18n>().set_lang(l);
        }
        if let Some(v) = s.view_wire {
            ui.set_view_wire(v);
        }
        if let Some(v) = s.view_shade {
            ui.set_view_shade(v);
        }
        if let Some(v) = s.view_bone {
            ui.set_view_bone(v);
        }
        if let Some(v) = s.view_texture {
            ui.set_view_texture(v);
        }
        if let Some(v) = s.view_hitbox {
            ui.set_view_hitbox(v);
        }
        if let Some(v) = s.view_attach {
            ui.set_view_attach(v);
        }
        if let Some(v) = s.view_grid {
            ui.set_view_grid(v);
        }
        if let Some(v) = s.view_axes {
            ui.set_view_axes(v);
        }
        if let Some(v) = s.view_ortho {
            ui.set_view_ortho(v);
        }
        if let Some(v) = s.view_uv {
            ui.set_view_uv(v);
        }
        if let Some(v) = s.view_flat {
            ui.set_view_flat(v);
        }
        if let Some(v) = s.view_fullbright {
            ui.set_view_fullbright(v);
        }
        if let Some(v) = s.view_normal {
            ui.set_view_normal(v);
        }
        if let Some(v) = s.view_bounds {
            ui.set_view_bounds(v);
        }
        if let Some(v) = s.view_skin_family {
            self.skin_family = v.max(0) as u32;
            ui.set_view_skin_family(v.max(0));
        }
        if let Some(v) = s.view_fov {
            ui.set_view_fov(v.clamp(5, 120));
        }
    }

    fn on_action(&mut self, cmd: &str) {
        let ui = match self.ui.upgrade() {
            Some(u) => u,
            None => return,
        };
        // 语言切换：改 root.lang 并同步 I18n 全局（驱动文案刷新）
        if cmd == "lang" {
            let nl = if ui.global::<I18n>().get_lang() == 0 { 1 } else { 0 };
            ui.global::<I18n>().set_lang(nl);
            self.save_settings(&ui);
            return;
        }
        // 主题切换：改 root.dark 并同步 Theme 全局（驱动配色刷新）
        if cmd == "theme" {
            let nd = !ui.get_dark();
            ui.set_dark(nd);
            ui.global::<Theme>().set_dark(nd);
            self.save_settings(&ui);
            return;
        }
        // 分支处理
        if let Some(field) = cmd.strip_prefix("browse:") {
            self.browse(&ui, field);
            if field == "view_mdl_file" {
                self.view_load(&ui);
            }
            self.save_settings(&ui);
            return;
        }
        if cmd == "minimize" {
            let _ = ui.window().set_minimized(true);
            return;
        }
        // 图标路径变化：实时加载预览封面
        if cmd == "icon_changed" {
            let p = ui.get_icon_path().to_string();
            if p.trim().is_empty() {
                ui.set_icon_has(false);
            } else if let Ok(img) = slint::Image::load_from_path(std::path::Path::new(&p)) {
                ui.set_icon_preview_image(img);
                ui.set_icon_has(true);
            } else {
                ui.set_icon_has(false);
            }
            return;
        }
        if cmd == "quit" {
            let _ = ui.hide();
            return;
        }
        if let Some(dir) = cmd.strip_prefix("view_rot:") {
            match dir {
                "left" => self.cam.yaw -= 0.12,
                "right" => self.cam.yaw += 0.12,
                "up" => self.cam.pitch = (self.cam.pitch + 0.08).clamp(-1.5, 1.5),
                "down" => self.cam.pitch = (self.cam.pitch - 0.08).clamp(-1.5, 1.5),
                _ => {}
            }
            self.redraw_view(&ui);
            self.save_settings(&ui);
            return;
        }
        // 方向键按住：降采样快速旋转（流畅）；松开做一次全分辨率重渲
        if let Some(dir) = cmd.strip_prefix("view_key_down:") {
            let idx = match dir { "left" => 0, "right" => 1, "up" => 2, "down" => 3, _ => 4 };
            if idx < 4 {
                self.key_held[idx] = true;
                match idx {
                    0 => self.cam.yaw -= 0.08,
                    1 => self.cam.yaw += 0.08,
                    2 => self.cam.pitch = (self.cam.pitch + 0.05).clamp(-1.5, 1.5),
                    3 => self.cam.pitch = (self.cam.pitch - 0.05).clamp(-1.5, 1.5),
                    _ => {}
                }
                self.redraw_view_lowres(&ui);
            }
            return;
        }
        if let Some(dir) = cmd.strip_prefix("view_key_up:") {
            let idx = match dir { "left" => 0, "right" => 1, "up" => 2, "down" => 3, _ => 4 };
            if idx < 4 {
                self.key_held[idx] = false;
                if self.key_held.iter().all(|h| !h) {
                    self.redraw_view(&ui); // 全分辨率回渲
                }
            }
            return;
        }
        if let Some(z) = cmd.strip_prefix("view_zoom:") {
            match z {
                "in" => self.cam.dist = (self.cam.dist * 0.9).max(0.05),
                "out" => self.cam.dist = (self.cam.dist * 1.1).min(500.0),
                _ => {}
            }
            self.redraw_view(&ui);
            self.save_settings(&ui);
            return;
        }
        // 拖拽旋转/平移：view_drag:dx:dy（按累计阈值节流渲染，保证顺滑）
        if let Some(d) = cmd.strip_prefix("view_drag:") {
            let parts: Vec<&str> = d.split(':').collect();
            if parts.len() == 2 {
                if let (Ok(dx), Ok(dy)) = (parts[0].parse::<f32>(), parts[1].parse::<f32>()) {
                    let scale = (self.cam.dist * 0.004).clamp(0.008, 0.6);
                    self.cam.yaw -= dx * 0.012;
                    self.cam.pitch = (self.cam.pitch + dy * 0.010).clamp(-1.5, 1.5);
                    self.cam.pan[0] += dx * scale;
                    self.cam.pan[1] -= dy * scale;
                    self.drag_accum += dx.abs() + dy.abs();
                    if self.drag_accum >= 3.0 {
                        self.drag_accum = 0.0;
                        self.redraw_view_lowres(&ui);
                    }
                }
            }
            return;
        }
        if cmd == "view_redraw" {
            self.redraw_view(&ui);
            return;
        }
        if cmd == "view_reset" {
            if let Some(mesh) = &self.mesh {
                let b = mesh.bounds;
                let r = (b[3] - b[0]).max(b[4] - b[1]).max(b[5] - b[2]).max(0.001) / 2.0;
                self.cam = backend::viewer::Camera {
                    yaw: 0.7,
                    pitch: -0.35,
                    dist: r * 3.2,
                    center: [(b[0] + b[3]) / 2.0, (b[1] + b[4]) / 2.0, (b[2] + b[5]) / 2.0],
                    pan: [0.0, 0.0, 0.0],
                };
            }
            self.redraw_view(&ui);
            return;
        }
        // 视图开关：view_toggle:wire/shade/texture/bone/hitbox/attach/grid/axes/ortho
        if let Some(which) = cmd.strip_prefix("view_toggle:") {
            match which {
                "wire" => {
                    let on = !ui.get_view_wire();
                    ui.set_view_wire(on);
                    if on {
                        ui.set_view_shade(false);
                        ui.set_view_texture(false);
                    }
                }
                "shade" => {
                    let on = !ui.get_view_shade();
                    ui.set_view_shade(on);
                    if on {
                        ui.set_view_wire(false);
                        ui.set_view_texture(false);
                    }
                }
                "texture" => {
                    let on = !ui.get_view_texture();
                    ui.set_view_texture(on);
                    if on {
                        ui.set_view_wire(false);
                        ui.set_view_shade(false);
                    }
                }
                "bone" => ui.set_view_bone(!ui.get_view_bone()),
                "hitbox" => ui.set_view_hitbox(!ui.get_view_hitbox()),
                "attach" => ui.set_view_attach(!ui.get_view_attach()),
                "grid" => ui.set_view_grid(!ui.get_view_grid()),
                "axes" => ui.set_view_axes(!ui.get_view_axes()),
                "ortho" => ui.set_view_ortho(!ui.get_view_ortho()),
                "normal" => ui.set_view_normal(!ui.get_view_normal()),
                "bounds" => ui.set_view_bounds(!ui.get_view_bounds()),
                "flat" => {
                    let on = !ui.get_view_flat();
                    ui.set_view_flat(on);
                    if on {
                        ui.set_view_wire(false);
                        ui.set_view_shade(false);
                        ui.set_view_texture(false);
                        ui.set_view_fullbright(false);
                    }
                }
                "fullbright" => {
                    let on = !ui.get_view_fullbright();
                    ui.set_view_fullbright(on);
                    if on {
                        ui.set_view_wire(false);
                        ui.set_view_shade(false);
                        ui.set_view_texture(false);
                        ui.set_view_flat(false);
                    }
                }
                "uv" => {
                    let on = !ui.get_view_uv();
                    ui.set_view_uv(on);
                    if on {
                        ui.set_view_wire(false);
                        ui.set_view_shade(false);
                        ui.set_view_texture(false);
                        ui.set_view_flat(false);
                        ui.set_view_fullbright(false);
                    }
                }
                _ => {}
            }
            self.redraw_view(&ui);
            self.save_settings(&ui);
            return;
        }
        // 皮肤族切换：view_skin:<i>
        if let Some(i) = cmd.strip_prefix("view_skin:") {
            if let Ok(i) = i.parse::<u32>() {
                let n = self.mesh.as_ref().map(|m| m.skins.len() as u32).unwrap_or(1).max(1);
                self.skin_family = i.min(n - 1);
                ui.set_view_skin_family(self.skin_family as i32);
                self.redraw_view(&ui);
                self.save_settings(&ui);
            }
            return;
        }
        // 背景色：view_bg:<0..3>
        if let Some(i) = cmd.strip_prefix("view_bg:") {
            if let Ok(i) = i.parse::<u32>() {
                self.bg_color = i.min(3);
                self.redraw_view(&ui);
                self.save_settings(&ui);
            }
            return;
        }
        // 视角 FOV：view_fov:<度>
        if let Some(i) = cmd.strip_prefix("view_fov:") {
            if let Ok(i) = i.parse::<i32>() {
                ui.set_view_fov(i.clamp(5, 120));
                self.redraw_view(&ui);
                self.save_settings(&ui);
            }
            return;
        }
        // 导入 GMA/VPK 预览：解压到临时缓存 → 列出模型 → 自动加载第一个
        if cmd == "view_import" {
            let zh = ui.global::<I18n>().get_lang() == 0;
            let title = if zh { "选择 .gma / .vpk 模型包" } else { "Select .gma / .vpk archive" };
            if let Some(p) = backend::browse("import".to_string(), title.to_string()) {
                self.import_archive(&ui, &p);
            }
            return;
        }
        // 从导入的档案中选择模型：view_model_select:<i>
        if let Some(i) = cmd.strip_prefix("view_model_select:") {
            if let (Some(dir), Ok(i)) = (&self.archive_dir, i.parse::<usize>()) {
                if let Some(rel) = self.archive_models.get(i) {
                    let p = dir.join(rel);
                    self.set_field(&ui, "view_mdl_file", p.to_string_lossy().into_owned());
                    self.view_load(&ui);
                }
            }
            return;
        }
        // 导出当前模型为 SMD（参考文件：nodes/triangles）
        if cmd == "view_export_smd" {
            let zh = ui.global::<I18n>().get_lang() == 0;
            if let Some(p) = backend::browse("smdsave".to_string(), if zh { "导出 SMD" } else { "Export SMD" }.to_string()) {
                match &self.mesh {
                    Some(m) => match export_smd(m, &p, self.skin_family) {
                        Ok(()) => self.toast(&ui, &if zh { "SMD 已导出" } else { "SMD exported" }, "ok"),
                        Err(e) => self.toast(&ui, &e, "error"),
                    },
                    None => self.toast(&ui, &(if zh { "没有已加载的模型" } else { "No model loaded" }), "error"),
                }
            }
            return;
        }
        // 身体组切换：view_bodygroup:<组序号>:<delta>
        if let Some(bg) = cmd.strip_prefix("view_bodygroup:") {
            let parts: Vec<&str> = bg.split(':').collect();
            if parts.len() == 2 {
                if let (Ok(i), Ok(delta)) = (parts[0].parse::<usize>(), parts[1].parse::<i32>()) {
                    let n = self.mesh.as_ref().and_then(|m| m.bodygroups.get(i)).map(|g| g.models.len()).unwrap_or(0);
                    if n > 0 {
                        if self.body_sel.len() <= i {
                            self.body_sel.resize(i + 1, 0);
                        }
                        let cur = self.body_sel[i].max(0);
                        let mut next = cur + delta;
                        if next < 0 {
                            next = n as i32 - 1;
                        }
                        if next >= n as i32 {
                            next = 0;
                        }
                        self.body_sel[i] = next;
                        // 重新加载（按新的身体组选择）
                        self.view_load(&ui);
                    }
                }
            }
            return;
        }
        // 姿势：选择骨骼
        if cmd == "pose_bone_prev" {
            if let Some(m) = &self.mesh {
                let n = m.bones.len();
                if n > 0 {
                    let cur = self.pose_bone.max(0) as usize;
                    let i = if cur == 0 { n - 1 } else { cur - 1 };
                    self.pose_bone = i as i32;
                    ui.set_pose_bone_index(i as i32);
                    ui.set_pose_bone_name(m.bones[i].name.clone().into());
                    self.pose_rot = [0.0, 0.0, 0.0];
                    ui.set_pose_rot_x(0.0);
                    ui.set_pose_rot_y(0.0);
                    ui.set_pose_rot_z(0.0);
                    self.redraw_view(&ui);
                }
            }
            return;
        }
        if cmd == "pose_bone_next" {
            if let Some(m) = &self.mesh {
                let n = m.bones.len();
                if n > 0 {
                    let cur = self.pose_bone.max(0) as usize;
                    let i = if cur + 1 >= n { 0 } else { cur + 1 };
                    self.pose_bone = i as i32;
                    ui.set_pose_bone_index(i as i32);
                    ui.set_pose_bone_name(m.bones[i].name.clone().into());
                    self.pose_rot = [0.0, 0.0, 0.0];
                    ui.set_pose_rot_x(0.0);
                    ui.set_pose_rot_y(0.0);
                    ui.set_pose_rot_z(0.0);
                    self.redraw_view(&ui);
                }
            }
            return;
        }
        if cmd == "view_pose_redraw" {
            self.pose_rot = [
                ui.get_pose_rot_x(),
                ui.get_pose_rot_y(),
                ui.get_pose_rot_z(),
            ];
            self.redraw_view_lowres(&ui);
            return;
        }
        // 序列选择：view_seq_select:<序号>（解码该序列骨骼动画）
        if let Some(idx) = cmd.strip_prefix("view_seq_select:") {
            if let Ok(i) = idx.parse::<i32>() {
                if let Some(m) = &self.mesh {
                    if i >= 0 && (i as usize) < m.sequences.len() {
                        let p = ui.get_view_mdl_file();
                        let l = if ui.global::<I18n>().get_lang() == 0 { "zh" } else { "en" };
                        let frames = match backend::mdl_sequence_anim(p.to_string(), i, l.to_string()) {
                            Ok(f) => f,
                            Err(_) => {
                                self.toast(&ui, if l == "zh" { "序列动画解码失败" } else { "Failed to decode sequence animation" }, "error");
                                Vec::new()
                            }
                        };
                        self.anim_seq = i;
                        self.anim_cache = frames;
                        self.anim_fps = m.sequences[i as usize].fps.max(0.1);
                        self.anim_acc = 0.0;
                        self.anim_frames = if !self.anim_cache.is_empty() {
                            self.anim_cache.len()
                        } else {
                            m.sequences[i as usize].frames.max(0) as usize
                        };
                        if self.anim_frames == 0 {
                            self.anim_frames = self.anim_cache.len().max(1);
                        }
                        self.anim_frame = 0;
                        self.anim_playing = false;
                        ui.set_anim_seq(i as i32);
                        ui.set_anim_frames(self.anim_frames as i32);
                        ui.set_anim_frame(0.0);
                        ui.set_anim_playing(false);
                        ui.set_seq_open(false);
                        self.redraw_view(&ui);
                    }
                }
            }
            return;
        }
        // 动画播放/暂停
        if cmd == "view_anim_play" {
            if !self.anim_cache.is_empty() && self.anim_frames > 1 {
                self.anim_playing = true;
                ui.set_anim_playing(true);
            }
            return;
        }
        if cmd == "view_anim_pause" {
            self.anim_playing = false;
            ui.set_anim_playing(false);
            return;
        }
        // 动画帧推进（由 UI 侧 Timer 驱动，40ms 间隔；按序列 fps 累计推进）
        if cmd == "view_anim_tick" {
            if self.anim_playing && !self.anim_cache.is_empty() {
                let n = self.anim_cache.len();
                // 40ms tick × fps → 本次应推进的帧数（累计余数保持帧率精度）
                self.anim_acc += 0.040 * self.anim_fps.max(0.1);
                let step = self.anim_acc.floor() as usize;
                if step > 0 {
                    self.anim_acc -= step as f32;
                    self.anim_frame = (self.anim_frame + step) % n;
                    ui.set_anim_frame(self.anim_frame as f32);
                    self.redraw_view_lowres(&ui);
                }
            }
            return;
        }
        // 拖动帧滑杆（读取滑块当前值）
        if cmd == "view_anim_seek" {
            if !self.anim_cache.is_empty() {
                let v = ui.get_anim_frame().round().max(0.0) as usize;
                self.anim_frame = v.min(self.anim_cache.len().saturating_sub(1));
                ui.set_anim_frame(self.anim_frame as f32);
                self.redraw_view(&ui);
            }
            return;
        }
        // 直接定位帧：view_anim_frame:<n>
        if let Some(n) = cmd.strip_prefix("view_anim_frame:") {
            if let Ok(f) = n.parse::<usize>() {
                if !self.anim_cache.is_empty() {
                    self.anim_frame = f.min(self.anim_cache.len().saturating_sub(1));
                    ui.set_anim_frame(self.anim_frame as f32);
                    self.redraw_view(&ui);
                }
            }
            return;
        }
        if cmd == "egg_tick" {
            // 彩蛋窗 LED 跑马灯：每秒刷新当前日期时间
            ui.set_led_time(now_full().into());
            return;
        }

        // 信息/校验/单次转换类命令由 run_simple 完成回调用 toast 提示，不自动展开日志抽屉；
        // 需详细进度日志的流式命令（打包/上传/解包/VPK 等）在各分支内自行展开。
        match cmd {
            "check" => {
                let p = ui.get_source_folder();
                let lang = ui.global::<I18n>().get_lang();
                let zh = lang == 0;
                if p.trim().is_empty() {
                    self.toast(&ui, if zh { "请先选择素材文件夹" } else { "Choose an asset folder first" }, "error");
                    return;
                }
                self.run_simple(move || backend::check_addon(p.to_string(), if zh { "zh".into() } else { "en".into() }));
            }
            "pack" => self.pack(&ui),
            "upload" => self.upload(&ui),
            "read" => {
                let p = ui.get_extract_gma();
                let lang = ui.global::<I18n>().get_lang();
                let zh = lang == 0;
                if p.trim().is_empty() {
                    self.toast(&ui, if zh { "请先选择 .gma 文件" } else { "Select a .gma file first" }, "error");
                    return;
                }
                self.run_simple(move || backend::read_gma(p.to_string(), if zh { "zh".into() } else { "en".into() }));
            }
            "extract" => self.extract(&ui),
            "vpk_pack" => {
                ui.set_log_open(true);
                let lang = ui.global::<I18n>().get_lang();
                let zh = lang == 0;
                let src = ui.get_vpk_src();
                if src.trim().is_empty() {
                    self.toast(&ui, if zh { "请先选择源文件夹" } else { "Select a source folder first" }, "error");
                    return;
                }
                let out = {
                    let o = ui.get_vpk_out().to_string();
                    if o.trim().is_empty() {
                        format!("{}.vpk", src.trim_end_matches(['\\', '/']))
                    } else {
                        o
                    }
                };
                ui.set_vpk_out(out.clone().into());
                let out2 = out.clone();
                let weak = self.ui.clone();
                self.run_stream(move |sink| {
                    let _ = backend::vpk_pack(src.to_string(), out, (if zh { "zh" } else { "en" }).to_string(), sink);
                    let _ = weak.upgrade_in_event_loop(move |ui| {
                        ui.set_vpk_file(out2.into());
                    });
                });
            }
            "vpk_list" => {
                let p = ui.get_vpk_file();
                let lang = ui.global::<I18n>().get_lang();
                let zh = lang == 0;
                if p.trim().is_empty() {
                    self.toast(&ui, if zh { "请先选择 .vpk 文件" } else { "Select a .vpk file first" }, "error");
                    return;
                }
                let weak = self.ui.clone();
                let p = p.to_string();
                self.run_simple(move || {
                    match backend::vpk_list(p, if zh { "zh".into() } else { "en".into() }) {
                        Ok(o) => {
                            let _ = weak.upgrade_in_event_loop(move |ui| {
                                ui.set_vpk_tree(o.text.clone().into());
                                ui.set_vpk_tree_visible(true);
                            });
                        }
                        Err(e) => {
                            let _ = weak.upgrade_in_event_loop(move |ui| {
                                ui.set_vpk_tree_visible(false);
                                Ctrl::toast_ui(&ui, &e, "error");
                            });
                        }
                    }
                    Ok(backend::OutDto { status: String::new(), text: String::new(), item_id: None })
                });
            }
            "vpk_unpack" => {
                ui.set_log_open(true);
                let lang = ui.global::<I18n>().get_lang();
                let zh = lang == 0;
                let p = ui.get_vpk_file();
                let dir = ui.get_vpk_dir();
                if p.trim().is_empty() || dir.trim().is_empty() {
                    self.toast(&ui, if zh { "请选择 .vpk 与输出目录" } else { "Select .vpk and output folder" }, "error");
                    return;
                }
                self.run_stream(move |sink| {
                    let _ = backend::vpk_unpack(p.to_string(), dir.to_string(), (if zh { "zh" } else { "en" }).to_string(), sink);
                });
            }
            "vtf_info" => {
                let p = ui.get_vtf_file();
                let lang = ui.global::<I18n>().get_lang();
                let zh = lang == 0;
                if p.trim().is_empty() {
                    self.toast(&ui, if zh { "请先选择 .vtf 文件" } else { "Select a .vtf file first" }, "error");
                    return;
                }
                self.run_simple(move || backend::vtf_info(p.to_string(), if zh { "zh".into() } else { "en".into() }));
            }
            "vtf_convert" => {
                let lang = ui.global::<I18n>().get_lang();
                let zh = lang == 0;
                let p = ui.get_vtf_file();
                if p.trim().is_empty() {
                    self.toast(&ui, if zh { "请先选择 .vtf 文件" } else { "Select a .vtf file first" }, "error");
                    return;
                }
                let out = {
                    let o = ui.get_vtf_out().to_string();
                    if o.trim().is_empty() {
                        p.trim_end_matches(|c| c == '.').to_string() + ".png"
                    } else {
                        o
                    }
                };
                ui.set_vtf_out(out.clone().into());
                let weak = self.ui.clone();
                self.run_simple(move || {
                    let r = backend::vtf_convert(p.to_string(), out.clone(), if zh { "zh".into() } else { "en".into() })?;
                    // 读取 PNG 预览
                    if let Ok(url) = backend::read_png_data_url(out.clone()) {
                        let _ = weak.upgrade_in_event_loop(move |ui| {
                            // 此处仅显示预览标记；真实图片显示见下方（data URL 无法直接解码为 Image）
                            let _ = url;
                            ui.set_vtf_preview_visible(true);
                        });
                    }
                    Ok(r)
                });
            }
            "vtf_encode" => {
                let lang = ui.global::<I18n>().get_lang();
                let zh = lang == 0;
                let p = ui.get_png_file();
                if p.trim().is_empty() {
                    self.toast(&ui, if zh { "请先选择源 PNG 图片" } else { "Select a source PNG first" }, "error");
                    return;
                }
                let out = {
                    let o = ui.get_vtf_enc_out().to_string();
                    if o.trim().is_empty() {
                        p.trim_end_matches(|c| c == '.').to_string() + ".vtf"
                    } else {
                        o
                    }
                };
                let fmt = match ui.get_vtf_enc_format() {
                    0 => "dxt5",
                    1 => "dxt1",
                    2 => "rgba",
                    _ => "rgb",
                }
                .to_string();
                ui.set_vtf_enc_out(out.clone().into());
                self.run_simple(move || backend::vtf_create(p.to_string(), out, fmt, if zh { "zh".into() } else { "en".into() }));
            }
            "vtf_preview" => refresh_vtf_preview(self.ui.clone()),
            "png_preview" => refresh_png_preview(self.ui.clone()),
            "mdl_info" => {
                let p = ui.get_mdl_file();
                let lang = ui.global::<I18n>().get_lang();
                let zh = lang == 0;
                if p.trim().is_empty() {
                    self.toast(&ui, if zh { "请先选择 .mdl 文件" } else { "Select a .mdl file first" }, "error");
                    return;
                }
                let weak = self.ui.clone();
                let p = p.to_string();
                self.run_simple(move || {
                    match backend::mdl_info(p, if zh { "zh".into() } else { "en".into() }) {
                        Ok(o) => {
                            let _ = weak.upgrade_in_event_loop(move |ui| {
                                ui.set_mdl_tree(o.text.clone().into());
                                ui.set_mdl_tree_visible(true);
                            });
                        }
                        Err(e) => {
                            let _ = weak.upgrade_in_event_loop(move |ui| {
                                ui.set_mdl_tree_visible(false);
                                Ctrl::toast_ui(&ui, &e, "error");
                            });
                        }
                    }
                    Ok(backend::OutDto { status: String::new(), text: String::new(), item_id: None })
                });
            }
            "mdl_decompile" => {
                let lang = ui.global::<I18n>().get_lang();
                let zh = lang == 0;
                let p = ui.get_mdl_file();
                let dir = ui.get_mdl_out_dir();
                if p.trim().is_empty() || dir.trim().is_empty() {
                    self.toast(&ui, if zh { "请选择 .mdl 与输出目录" } else { "Select .mdl and output folder" }, "error");
                    return;
                }
                let name = p.rsplit(['\\', '/']).next().unwrap_or("model").trim_end_matches(".mdl");
                let out = format!("{}\\{}.smd", dir.trim_end_matches(['\\', '/']), name);
                self.run_simple(move || backend::mdl_decompile(p.to_string(), out, if zh { "zh".into() } else { "en".into() }));
            }
            "mdl_pack" => self.mdl_pack(&ui),
            "detect_smdl" => {
                let lang = ui.global::<I18n>().get_lang();
                let zh = lang == 0;
                let p = backend::detect_studiomdl();
                if let Some(v) = &p {
                    ui.set_studiomdl(v.clone().into());
                }
                let msg = match &p {
                    Some(x) => format!("{}: {x}", if zh { "已自动探测 studiomdl" } else { "Auto-detected studiomdl" }),
                    None => if zh { "未探测到 studiomdl，请手动指定" } else { "studiomdl not found, set manually" }.to_string(),
                };
                self.toast(&ui, &msg, if p.is_some() { "success" } else { "error" });
            }
            "detect_gmpublish" => {
                let lang = ui.global::<I18n>().get_lang();
                let zh = lang == 0;
                match backend::detect_gmpublish() {
                    Some(v) => {
                        ui.set_gmpublish(v.clone().into());
                        self.toast(&ui, &format!("{}: {v}", if zh { "gmpublish.exe 已自动定位" } else { "gmpublish.exe auto-located" }), "success");
                    }
                    None => self.toast(&ui, if zh { "未探测到 gmpublish，请手动浏览" } else { "gmpublish not found, browse manually" }, "error"),
                }
            }
            "detect_gmad" => {
                let lang = ui.global::<I18n>().get_lang();
                let zh = lang == 0;
                match backend::detect_gmad() {
                    Some(v) => {
                        ui.set_gmad(v.clone().into());
                        self.toast(&ui, &format!("{}: {v}", if zh { "gmad.exe 已自动定位" } else { "gmad.exe auto-located" }), "success");
                    }
                    None => self.toast(&ui, if zh { "未探测到 gmad，请手动浏览" } else { "gmad not found, browse manually" }, "error"),
                }
            }
            "open_github" => {
                let _ = backend::open_url("https://github.com/hlshell/GMod_Tools".to_string());
            }
            // 关于页单个探测按钮：一次自动探测全部三个工具路径
            "detect_all" => {
                let lang = ui.global::<I18n>().get_lang();
                let zh = lang == 0;
                if let Some(v) = backend::detect_gmpublish() { ui.set_gmpublish(v.clone().into()); }
                if let Some(v) = backend::detect_gmad() { ui.set_gmad(v.clone().into()); }
                if let Some(v) = backend::detect_studiomdl() { ui.set_studiomdl(v.clone().into()); }
                self.toast(&ui, if zh { "已自动探测 gmpublish / gmad / studiomdl" } else { "Auto-detected gmpublish / gmad / studiomdl" }, "success");
            }
            "view_load" => self.view_load(&ui),
            _ => {}
        }
        self.save_settings(&ui);
    }
}

fn settings_dir() -> std::path::PathBuf {
    if let Ok(app) = std::env::var("APPDATA") {
        std::path::PathBuf::from(app).join("gma-slint")
    } else {
        std::path::PathBuf::from(".")
    }
}

// 无边框窗口圆角：通过 DWM 让系统给窗口加圆角（Windows 11 DWMWA_WINDOW_CORNER_PREFERENCE），
// 参考 slint-demo 方案，比 SetWindowRgn / 透明窗口更稳定，不会重绘时闪边框。
fn apply_rounded_corners(ui: &MainWindow) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_ROUND: i32 = 2;
    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: *mut core::ffi::c_void,
            attribute: u32,
            value: *const core::ffi::c_void,
            size: u32,
        ) -> i32;
    }
    let _ = ui.window().with_winit_window(|w: &winit::window::Window| {
        if let Ok(h) = w.window_handle() {
            if let RawWindowHandle::Win32(wh) = h.as_raw() {
                let hwnd = wh.hwnd.get() as *mut core::ffi::c_void;
                let pref = DWMWCP_ROUND;
                unsafe {
                    DwmSetWindowAttribute(
                        hwnd,
                        DWMWA_WINDOW_CORNER_PREFERENCE,
                        &pref as *const i32 as *const core::ffi::c_void,
                        core::mem::size_of::<i32>() as u32,
                    );
                }
            }
        }
    });
}

// ----------------------------- 图像预览（选择文件自动载入） -----------------------------
// 读当前 vtf-file/png-file，若为有效文件则后台解码并回填预览图；文件无效则隐藏预览。
fn refresh_vtf_preview(ui_weak: slint::Weak<MainWindow>) {
    let ui = match ui_weak.upgrade() {
        Some(u) => u,
        None => return,
    };
    let p = ui.get_vtf_file();
    if p.trim().is_empty() || !std::path::Path::new(&p).is_file() {
        ui.set_vtf_preview_visible(false);
        return;
    }
    let weak = ui_weak.clone();
    let p = p.to_string();
    std::thread::spawn(move || {
        match backend::vtf_preview(p) {
            Ok(path) => {
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    match slint::Image::load_from_path(std::path::Path::new(&path)) {
                        Ok(img) => {
                            ui.set_vtf_preview_image(img);
                            ui.set_vtf_preview_visible(true);
                        }
                        Err(_) => ui.set_vtf_preview_visible(false),
                    }
                });
            }
            Err(e) => {
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.set_vtf_preview_visible(false);
                    Ctrl::toast_ui(&ui, &e, "error");
                });
            }
        }
    });
}

fn refresh_png_preview(ui_weak: slint::Weak<MainWindow>) {
    let ui = match ui_weak.upgrade() {
        Some(u) => u,
        None => return,
    };
    let p = ui.get_png_file();
    if p.trim().is_empty() || !std::path::Path::new(&p).is_file() {
        ui.set_png_preview_visible(false);
        return;
    }
    let weak = ui_weak.clone();
    let p = p.to_string();
    std::thread::spawn(move || {
        let _ = weak.upgrade_in_event_loop(move |ui| {
            match slint::Image::load_from_path(std::path::Path::new(&p)) {
                Ok(img) => {
                    ui.set_png_preview_image(img);
                    ui.set_png_preview_visible(true);
                }
                Err(_) => ui.set_png_preview_visible(false),
            }
        });
    });
}

// ----------------------------- 文件拖放自动填入 -----------------------------
// 拦截 winit 的 FileDrop 事件：用户把文件夹/文件拖到窗口对应区域，按
// 落点所在的卡片/区域 + 文件类型，自动填入对应目录/文件输入框。
const ADDON_TYPES: [&str; 9] = ["ServerContent", "gamemode", "map", "weapon", "vehicle", "npc", "tool", "effects", "model"];

struct DropHandler {
    ui: Arc<Mutex<Option<slint::Weak<MainWindow>>>>,
}

impl DropHandler {
    // 根据文件类型 + 当前页/子 Tab 路由到对应输入框
    fn route_drop(&self, ui: &MainWindow, path: &Path) {
        let is_dir = path.is_dir();
        let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).unwrap_or_default();
        let lang = ui.global::<I18n>().get_lang();
        let zh = lang == 0;
        let p: String = path.to_string_lossy().to_string();
        let page = ui.get_page_index();

        match page {
            // 打包与上传页
            0 => {
                if is_dir {
                    ui.set_source_folder(p.clone().into()); // 打包素材文件夹
                    // 自动解读 addon.json 补填元数据
                    if let Some(a) = read_addon_json(p.clone()) {
                        if !a.title.is_empty() {
                            ui.set_addon_name(a.title.into());
                        }
                        if !a.addon_type.is_empty() {
                            let idx = ADDON_TYPES.iter().position(|t| *t == a.addon_type).unwrap_or(0);
                            ui.set_addon_type_index(idx as i32);
                        }
                        if !a.tags.is_empty() {
                            ui.set_tags(a.tags.join(", ").into());
                        }
                        if !a.ignore.is_empty() {
                            ui.set_ignore(a.ignore.join(", ").into());
                        }
                    }
                    // 自动补输出文件名 = 素材文件夹名.gma
                    let folder_name = Path::new(&p)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    if !folder_name.is_empty() {
                        ui.set_out_gma(format!("{folder_name}.gma").into());
                    }
                } else {
                    match ext.as_str() {
                        "gma" => ui.set_gma_file(p.clone().into()), // 上传 .gma
                        "jpg" | "jpeg" | "png" => ui.set_icon_path(p.clone().into()),
                        _ => ui.set_gma_file(p.clone().into()),
                    }
                }
            }
            // 其他工具页：按子 Tab 路由
            2 => match ui.get_tools_sub() {
                0 => { // VPK
                    if is_dir { ui.set_vpk_src(p.clone().into()); }
                    else if ext == "vpk" { ui.set_vpk_file(p.clone().into()); }
                    else { ui.set_vpk_src(p.clone().into()); }
                }
                1 => { // VTF
                    if ext == "vtf" {
                        ui.set_vtf_file(p.clone().into());
                        refresh_vtf_preview(ui.as_weak());
                    }
                    else if ext == "png" {
                        ui.set_png_file(p.clone().into());
                        refresh_png_preview(ui.as_weak());
                    }
                    else if is_dir { ui.set_vtf_out(p.clone().into()); }
                }
                2 => { // MDL
                    if ext == "mdl" { ui.set_mdl_file(p.clone().into()); }
                    else if ext == "qc" { ui.set_qc_file(p.clone().into()); }
                    else if is_dir { ui.set_mdl_out_dir(p.clone().into()); }
                }
                3 => { // 模型查看
                    if ext == "mdl" {
                        ui.set_view_mdl_file(p.clone().into());
                        let _ = ui.global::<Backend>().invoke_action("view_load".into());
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Ctrl::toast_ui(ui, &format!(
            "{} {}",
            if zh { "已拖入" } else { "Dropped" },
            p
        ), "info");
    }

    fn log(&self, ui: &MainWindow, line: &str) {
        let now = line; // 简化：追加行
        let cur = ui.get_log_text();
        let merged = if cur.is_empty() { now.to_string() } else { format!("{cur}\n{now}") };
        ui.set_log_text(merged.into());
        ui.set_status(line.into());
    }
}

impl CustomApplicationHandler for DropHandler {
    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _winit_window: Option<&winit::window::Window>,
        _slint_window: Option<&slint::Window>,
        event: &winit::event::WindowEvent,
    ) -> EventResult {
        match event {
            // 窗口尺寸变化时重新应用 DWM 圆角
            winit::event::WindowEvent::Resized(_) => {
                if let Some(ui) = self.ui.lock().unwrap().as_ref().and_then(|w| w.upgrade()) {
                    apply_rounded_corners(&ui);
                }
            }
            winit::event::WindowEvent::DroppedFile(path) => {
                if let Some(ui) = self.ui.lock().unwrap().as_ref().and_then(|w| w.upgrade()) {
                    self.route_drop(&ui, Path::new(&path.to_string_lossy().to_string()));
                }
            }
            // 模型查看：方向键按住旋转（down/up 通道，支持系统按键自动重复 → 降采样流畅旋转）
            winit::event::WindowEvent::KeyboardInput { event, .. } => {
                let dir = match &event.logical_key {
                    winit::keyboard::Key::Named(n) => match n {
                        winit::keyboard::NamedKey::ArrowLeft => Some("left"),
                        winit::keyboard::NamedKey::ArrowRight => Some("right"),
                        winit::keyboard::NamedKey::ArrowUp => Some("up"),
                        winit::keyboard::NamedKey::ArrowDown => Some("down"),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(dir) = dir {
                    let cmd = if event.state == winit::event::ElementState::Pressed {
                        format!("view_key_down:{dir}")
                    } else {
                        format!("view_key_up:{dir}")
                    };
                    if let Some(ui) = self.ui.lock().unwrap().as_ref().and_then(|w| w.upgrade()) {
                        let _ = ui.global::<Backend>().invoke_action(cmd.into());
                    }
                }
                // + / - 缩放
                if event.state == winit::event::ElementState::Pressed {
                    if let winit::keyboard::Key::Character(c) = &event.logical_key {
                        let zoom = match c.as_str() {
                            "+" | "=" => Some("view_zoom:in"),
                            "-" | "_" => Some("view_zoom:out"),
                            _ => None,
                        };
                        if let Some(z) = zoom {
                            if let Some(ui) = self.ui.lock().unwrap().as_ref().and_then(|w| w.upgrade()) {
                                let _ = ui.global::<Backend>().invoke_action(z.into());
                            }
                        }
                    }
                }
            }
            // 模型查看：鼠标滚轮缩放
            winit::event::WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y,
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                let cmd = if y > 0.0 { Some("view_zoom:in") } else if y < 0.0 { Some("view_zoom:out") } else { None };
                if let Some(cmd) = cmd {
                    if let Some(ui) = self.ui.lock().unwrap().as_ref().and_then(|w| w.upgrade()) {
                        let _ = ui.global::<Backend>().invoke_action(cmd.into());
                    }
                }
            }
            _ => {}
        }
        EventResult::Propagate
    }
}

// ----------------------------- 入口 -----------------------------
fn main() -> Result<(), slint::PlatformError> {
    let drop_ui: Arc<Mutex<Option<slint::Weak<MainWindow>>>> = Arc::new(Mutex::new(None));
    let backend = BackendSelector::new()
        .with_winit_window_attributes_hook(|attrs| {
            // 无边框（圆角由 DWM DWMWA_WINDOW_CORNER_PREFERENCE 承担，参考 slint-demo）
            attrs.with_decorations(false)
        })
        .with_winit_custom_application_handler(DropHandler { ui: drop_ui.clone() })
        .select()
        .expect("failed to select Slint backend");
    let _ = backend; // select() 已完成 platform 注册

    let ui = MainWindow::new()?;
    *drop_ui.lock().unwrap() = Some(ui.as_weak());

    // 控制器
    let mut ctrl = Ctrl::new(&ui);
    ctrl.load_settings(&ui);
    ui.global::<Theme>().set_dark(ui.get_dark());

    // 启动时自动探测 gmpublish/gmad/studiomdl（空则补填）
    let weak = ui.as_weak();
    if let Some(w) = weak.upgrade() {
        if w.get_gmpublish().is_empty() {
            if let Some(v) = backend::detect_gmpublish() {
                w.set_gmpublish(v.into());
            }
        }
        if w.get_gmad().is_empty() {
            if let Some(v) = backend::detect_gmad() {
                w.set_gmad(v.into());
            }
        }
        if w.get_studiomdl().is_empty() {
            if let Some(v) = backend::detect_studiomdl() {
                w.set_studiomdl(v.into());
            }
        }
        if !w.get_source_folder().is_empty() {
            ctrl.auto_fill_folder(&w);
        }
    }

    // 命令分发
    let ctrl_ref = Arc::new(std::sync::Mutex::new(ctrl));
    let backend_global = ui.global::<Backend>();
    backend_global.on_action({
        let ctrl_ref = ctrl_ref.clone();
        move |cmd| {
            if let Ok(mut c) = ctrl_ref.lock() {
                c.on_action(&cmd);
            }
        }
    });

    // 预览画布拖拽（模型查看：旋转/平移），经 float 回调直达控制器
    ui.on_view_drag_move({
        let ctrl_ref = ctrl_ref.clone();
        move |dx, dy| {
            if let Ok(mut c) = ctrl_ref.lock() {
                c.on_action(&format!("view_drag:{dx}:{dy}"));
            }
        }
    });

    // 自定义标题栏拖拽：位移超阈值后触发一次，交给系统原生窗口拖拽（drag_window）。
    // 系统接管直到鼠标释放，避免逐帧 set_position 造成的窗口闪跳。
    ui.on_drag_start({
        let ui_weak = ui.as_weak();
        move || {
            if let Some(u) = ui_weak.upgrade() {
                let _ = u.window().with_winit_window(|w: &winit::window::Window| w.drag_window());
            }
        }
    });

    // 先显示窗口以创建原生句柄，再应用 DWM 圆角，最后进入事件循环
    ui.show()?;
    apply_rounded_corners(&ui);

    ui.run()
}
