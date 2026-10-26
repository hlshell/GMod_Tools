//! GMA (Garry's Mod Addon) 格式核心：读写 / 打包 / 解包 / CRC 校验。
//!
//! 二进制结构（GMAD v3，小端）：
//! ```text
//! "GMAD"          4 字节 标识
//! version         1 字节 (3)
//! steam_id        8 字节 u64
//! timestamp       8 字节 u64 (Unix)
//! padding         1 字节 (0x00)
//! addon_name      null 结尾字符串
//! meta_json       null 结尾字符串 (title/type/tags/ignore 的 JSON)
//! addon_version   4 字节 u32
//! 文件条目块:
//!   loop:
//!     index       4 字节 u32 (从 1 开始)
//!     file_path   null 结尾字符串
//!     size        8 字节 u64
//!     crc32       4 字节 u32
//!   index == 0 结束
//! 之后依次为各文件的原始数据
//! ```

use std::fs;
use std::path::{Path, PathBuf};

pub const GMA_MAGIC: [u8; 4] = *b"GMAD";
pub const GMA_VERSION: u8 = 0x03;

/// GMA 头元信息。
#[derive(Debug, Clone)]
pub struct GmaMeta {
    pub steam_id: u64,
    pub timestamp: u64,
    pub addon_name: String,
    pub addon_type: String,
    pub tags: Vec<String>,
    pub ignore: Vec<String>,
    pub description: String,
    pub addon_version: u32,
}

impl Default for GmaMeta {
    fn default() -> Self {
        Self {
            steam_id: 0,
            timestamp: 0,
            addon_name: String::new(),
            addon_type: "ServerContent".into(),
            tags: Vec::new(),
            ignore: Vec::new(),
            description: String::new(),
            addon_version: 1,
        }
    }
}

/// 单个文件条目（位于条目块）。
#[derive(Debug, Clone)]
pub struct GmaEntry {
    pub index: u32,
    pub path: String,
    pub size: u64,
    pub crc32: u32,
}

/// 解析后的完整 GMA 信息。
#[derive(Debug, Clone)]
pub struct GmaInfo {
    pub meta: GmaMeta,
    pub entries: Vec<GmaEntry>,
    pub data_offset: u64,
}

pub struct PackResult {
    pub file_count: usize,
    pub total_size: u64,
}

/// 单文件 CRC 校验结果。
#[derive(Debug, Clone)]
#[allow(dead_code)] // path/size/crc 为校验 API 的完整输出字段，GUI 目前仅消费 ok
pub struct EntryStatus {
    pub path: String,
    pub size: u64,
    pub expected_crc: u32,
    pub actual_crc: u32,
    pub ok: bool,
}

/// 将文件夹打包为 .gma。
///
/// `on_progress(done, total, current_file)` 用于回报进度。
pub fn pack_folder(
    folder: &Path,
    meta: &GmaMeta,
    out: &Path,
    mut on_progress: impl FnMut(u64, u64, &str),
) -> Result<PackResult, String> {
    if !folder.is_dir() {
        return Err(format!("{} 不是有效的文件夹", folder.display()));
    }
    let mut items: Vec<FileItem> = Vec::new();
    collect_files(folder, folder, &meta.ignore, &mut items)?;
    items.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    let total: u64 = items.iter().map(|i| i.size).sum();

    let mut f = fs::File::create(out)
        .map_err(|e| format!("无法创建 {}: {}", out.display(), e))?;

    // 头
    let mut header = Vec::new();
    header.extend_from_slice(&GMA_MAGIC);
    header.push(GMA_VERSION);
    header.extend_from_slice(&meta.steam_id.to_le_bytes());
    let ts = if meta.timestamp != 0 {
        meta.timestamp
    } else {
        unix_now()
    };
    header.extend_from_slice(&ts.to_le_bytes());
    header.push(0x00);
    write_cstring(&mut header, &meta.addon_name);
    write_cstring(&mut header, &build_meta_json(meta));
    write_cstring(&mut header, "Author Name"); // 与 gmad 官方格式一致的 author 字段
    header.extend_from_slice(&meta.addon_version.max(1).to_le_bytes());

    // 条目块
    for (i, item) in items.iter().enumerate() {
        header.extend_from_slice(&((i as u32) + 1).to_le_bytes());
        write_cstring(&mut header, &item.rel_path);
        header.extend_from_slice(&item.size.to_le_bytes());
        header.extend_from_slice(&item.crc.to_le_bytes());
    }
    header.extend_from_slice(&0u32.to_le_bytes());

    use std::io::Write;
    f.write_all(&header)
        .map_err(|e| format!("写入头失败: {}", e))?;

    // 文件数据
    let mut written: u64 = 0;
    for item in &items {
        let bytes = fs::read(&item.abs_path)
            .map_err(|e| format!("读取 {} 失败: {}", item.abs_path.display(), e))?;
        f.write_all(&bytes)
            .map_err(|e| format!("写入失败: {}", e))?;
        written += item.size;
        on_progress(written, total, &item.rel_path);
    }

    Ok(PackResult {
        file_count: items.len(),
        total_size: total,
    })
}

/// 读取并解析 .gma 文件。
pub fn read_gma(path: &Path) -> Result<GmaInfo, String> {
    let bytes = fs::read(path).map_err(|e| format!("读取 {} 失败: {}", path.display(), e))?;
    parse_gma(&bytes)
}

/// addon.json 中可读回的字段（GMod 工坊元数据）。
#[derive(Debug, Default, Clone)]
pub struct AddonJson {
    pub title: String,
    pub addon_type: String,
    pub tags: Vec<String>,
    pub ignore: Vec<String>,
}

/// 读取素材根目录的 addon.json，返回其中字段（缺失的字段保持默认空值）。
pub fn read_addon_json(path: &Path) -> Result<AddonJson, String> {
    let s = fs::read_to_string(path)
        .map_err(|e| format!("读取 addon.json 失败: {}", e))?;
    let v: serde_json::Value =
        serde_json::from_str(&s).map_err(|e| format!("addon.json 解析失败: {}", e))?;
    let mut a = AddonJson::default();
    if let Some(t) = v.get("title").and_then(|x| x.as_str()) {
        a.title = t.to_owned();
    }
    if let Some(t) = v.get("type").and_then(|x| x.as_str()) {
        a.addon_type = t.to_owned();
    }
    if let Some(arr) = v.get("tags").and_then(|x| x.as_array()) {
        a.tags = arr.iter().filter_map(|x| x.as_str().map(String::from)).collect();
    }
    if let Some(arr) = v.get("ignore").and_then(|x| x.as_array()) {
        a.ignore = arr.iter().filter_map(|x| x.as_str().map(String::from)).collect();
    }
    Ok(a)
}

/// 在素材文件夹根目录写入 addon.json（GMod 工坊元数据，供 gmpublish 上传打标签用）。
pub fn write_addon_json(folder: &Path, meta: &GmaMeta) -> Result<(), String> {
    let path = folder.join("addon.json");
    let obj = serde_json::json!({
        "title": meta.addon_name,
        "type": meta.addon_type,
        "tags": meta.tags,
        "ignore": meta.ignore,
    });
    let content = serde_json::to_string_pretty(&obj)
        .map_err(|e| format!("生成 addon.json 失败: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("写入 {} 失败: {}", path.display(), e))?;
    Ok(())
}

/// 从字节解析 GMA。
pub fn parse_gma(bytes: &[u8]) -> Result<GmaInfo, String> {
    let mut pos = 4usize;
    if bytes.len() < 22 {
        return Err("文件过小，无法解析".into());
    }
    if bytes[0..4] != GMA_MAGIC {
        return Err("不是有效的 GMA 文件（缺少 GMAD 标识）".into());
    }
    let _version = bytes[pos];
    pos += 1;
    let steam_id = read_u64_le(bytes, &mut pos)?;
    let timestamp = read_u64_le(bytes, &mut pos)?;
    let _padding = bytes[pos];
    pos += 1;
    let addon_name = read_cstring(bytes, &mut pos)?;
    let meta_json = read_cstring(bytes, &mut pos)?;
    // gmad 头部在 addon.json 之后还有一个 author 字符串字段，需跳过，否则字段错位
    let _author = read_cstring(bytes, &mut pos)?;
    let addon_version = read_u32_le(bytes, &mut pos)?;

    let mut entries = Vec::new();
    loop {
        let idx = read_u32_le(bytes, &mut pos)?;
        if idx == 0 {
            break;
        }
        let path_str = read_cstring(bytes, &mut pos)?;
        let size = read_u64_le(bytes, &mut pos)?;
        let crc = read_u32_le(bytes, &mut pos)?;
        entries.push(GmaEntry {
            index: idx,
            path: path_str,
            size,
            crc32: crc,
        });
    }
    let data_offset = pos as u64;

    let mut meta = GmaMeta {
        steam_id,
        timestamp,
        addon_name,
        addon_version,
        ..Default::default()
    };
    parse_meta_json(&meta_json, &mut meta);

    Ok(GmaInfo {
        meta,
        entries,
        data_offset,
    })
}

/// 逐文件校验 CRC，返回 (是否全部通过, 各文件状态)。
pub fn validate_gma(
    path: &Path,
    mut on_progress: impl FnMut(u64, u64, &str),
) -> Result<(bool, Vec<EntryStatus>), String> {
    let bytes = fs::read(path).map_err(|e| format!("读取 {} 失败: {}", path.display(), e))?;
    let info = parse_gma(&bytes)?;
    let mut pos = info.data_offset as usize;
    let total = info.entries.iter().map(|e| e.size).sum::<u64>();
    let mut done = 0u64;
    let mut all_ok = true;
    let mut statuses = Vec::new();
    for e in &info.entries {
        if pos + e.size as usize > bytes.len() {
            statuses.push(EntryStatus {
                path: e.path.clone(),
                size: e.size,
                expected_crc: e.crc32,
                actual_crc: 0,
                ok: false,
            });
            all_ok = false;
            continue;
        }
        let slice = &bytes[pos..pos + e.size as usize];
        pos += e.size as usize;
        let actual = crc32fast::hash(slice);
        let ok = actual == e.crc32;
        all_ok = all_ok && ok;
        statuses.push(EntryStatus {
            path: e.path.clone(),
            size: e.size,
            expected_crc: e.crc32,
            actual_crc: actual,
            ok,
        });
        done += e.size;
        on_progress(done, total, &e.path);
    }
    Ok((all_ok, statuses))
}

/// 将 .gma 解包到指定目录。
pub fn extract_gma(
    path: &Path,
    out_dir: &Path,
    mut on_progress: impl FnMut(u64, u64, &str),
) -> Result<(), String> {    let bytes = fs::read(path).map_err(|e| format!("读取 {} 失败: {}", path.display(), e))?;
    let info = parse_gma(&bytes)?;
    let mut pos = info.data_offset as usize;
    let total = info.entries.iter().map(|e| e.size).sum::<u64>();
    let mut done = 0u64;
    for e in &info.entries {
        if !safe_rel_path(&e.path) {
            return Err(format!("拒绝不安全的路径: {}", e.path));
        }
        if pos + e.size as usize > bytes.len() {
            return Err(format!("文件 {} 数据缺失", e.path));
        }
        let slice = &bytes[pos..pos + e.size as usize];
        pos += e.size as usize;
        let out_path = out_dir.join(&e.path);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|er| format!("创建目录 {} 失败: {}", parent.display(), er))?;
        }
        fs::write(&out_path, slice)
            .map_err(|er| format!("写入 {} 失败: {}", out_path.display(), er))?;
        done += e.size;
        on_progress(done, total, &e.path);
    }
    Ok(())
}

/// 列出 GMA 内的 .mdl 模型条目（相对路径）。
/// 当前导入预览走全量解压 + find_models；此函数保留供选择性提取场景。
#[allow(dead_code)]
pub fn gma_model_list(path: &Path) -> Result<Vec<String>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("读取 {} 失败: {}", path.display(), e))?;
    let info = parse_gma(&bytes)?;
    Ok(info
        .entries
        .iter()
        .map(|e| e.path.clone())
        .filter(|p| p.to_lowercase().ends_with(".mdl"))
        .collect())
}

/// 提取 GMA 内单个条目到目录（保留相对路径）。保留供选择性提取场景。
#[allow(dead_code)]
pub fn gma_extract_one(path: &Path, rel: &str, out_dir: &Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("读取 {} 失败: {}", path.display(), e))?;
    let info = parse_gma(&bytes)?;
    let mut pos = info.data_offset as usize;
    for e in &info.entries {
        if !safe_rel_path(&e.path) {
            return Err(format!("拒绝不安全的路径: {}", e.path));
        }
        if pos + e.size as usize > bytes.len() {
            return Err(format!("文件 {} 数据缺失", e.path));
        }
        let slice = &bytes[pos..pos + e.size as usize];
        pos += e.size as usize;
        if e.path != rel {
            continue;
        }
        let out_path = out_dir.join(&e.path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|er| format!("创建目录 {} 失败: {}", parent.display(), er))?;
        }
        std::fs::write(&out_path, slice).map_err(|er| format!("写入 {} 失败: {}", out_path.display(), er))?;
        return Ok(());
    }
    Err(format!("GMA 中未找到条目 {}", rel))
}

// ---------------------------------------------------------------------------

struct FileItem {
    rel_path: String,
    abs_path: PathBuf,
    size: u64,
    crc: u32,
}

fn collect_files(
    root: &Path,
    dir: &Path,
    ignore: &[String],
    items: &mut Vec<FileItem>,
) -> Result<(), String> {
    let rd = fs::read_dir(dir).map_err(|e| format!("读取目录 {} 失败: {}", dir.display(), e))?;
    for entry in rd {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .map_err(|_| "路径前缀错误".to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        // addon.json 是文件夹级元数据，不打包进 gma 内容
        if rel == "addon.json" || is_ignored(&rel, ignore) {
            continue;
        }
        let ft = entry
            .file_type()
            .map_err(|e| format!("获取文件类型失败: {}", e))?;
        if ft.is_dir() {
            collect_files(root, &path, ignore, items)?;
        } else if ft.is_file() {
            let bytes = fs::read(&path)
                .map_err(|e| format!("读取 {} 失败: {}", path.display(), e))?;
            let crc = crc32fast::hash(&bytes);
            items.push(FileItem {
                rel_path: rel,
                abs_path: path,
                size: bytes.len() as u64,
                crc,
            });
        }
    }
    Ok(())
}

fn is_ignored(rel: &str, ignore: &[String]) -> bool {
    ignore.iter().any(|pat| {
        let pat = pat.trim();
        if pat.is_empty() {
            return false;
        }
        let name = rel.rsplit('/').next().unwrap_or(rel);
        glob_match(pat, rel) || glob_match(pat, name)
    })
}

/// 简单的 * 与 ? 通配匹配（大小写敏感，与 gmad 的 ignore 语义一致）。
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let mut dp = vec![vec![false; t.len() + 1]; p.len() + 1];
    dp[0][0] = true;
    for i in 0..p.len() {
        if p[i] == '*' {
            dp[i + 1][0] = dp[i][0];
        }
    }
    for i in 0..p.len() {
        for j in 0..=t.len() {
            if dp[i][j] {
                match p[i] {
                    '*' => {
                        dp[i + 1][j] = true;
                        if j < t.len() {
                            dp[i][j + 1] = true;
                        }
                    }
                    '?' => {
                        if j < t.len() {
                            dp[i + 1][j + 1] = true;
                        }
                    }
                    c => {
                        if j < t.len() && c == t[j] {
                            dp[i + 1][j + 1] = true;
                        }
                    }
                }
            }
        }
    }
    dp[p.len()][t.len()]
}

fn safe_rel_path(p: &str) -> bool {
    if p.is_empty() || p.starts_with('/') || p.starts_with('\\') {
        return false;
    }
    for part in p.split(['/', '\\']) {
        if part == ".." || part == "." {
            return false;
        }
    }
    true
}

fn build_meta_json(meta: &GmaMeta) -> String {
    serde_json::json!({
        "title": meta.addon_name,
        "type": meta.addon_type,
        "tags": meta.tags,
        "ignore": meta.ignore,
    })
    .to_string()
}

fn parse_meta_json(s: &str, meta: &mut GmaMeta) {
    match serde_json::from_str::<serde_json::Value>(s) {
        Ok(v) => {
            if let Some(t) = v.get("title").and_then(|x| x.as_str()) {
                meta.addon_name = t.to_owned();
            }
            if let Some(t) = v.get("type").and_then(|x| x.as_str()) {
                meta.addon_type = t.to_owned();
            }
            if let Some(a) = v.get("tags").and_then(|x| x.as_array()) {
                meta.tags = a.iter().filter_map(|x| x.as_str().map(String::from)).collect();
            }
            if let Some(a) = v.get("ignore").and_then(|x| x.as_array()) {
                meta.ignore = a
                    .iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect();
            }
        }
        Err(_) => {
            if !s.is_empty() {
                meta.description = s.to_owned();
            }
        }
    }
}

fn write_cstring(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
    out.push(0);
}

fn read_u32_le(b: &[u8], pos: &mut usize) -> Result<u32, String> {
    if *pos + 4 > b.len() {
        return Err("数据截断 (u32)".into());
    }
    let v = u32::from_le_bytes(b[*pos..*pos + 4].try_into().unwrap());
    *pos += 4;
    Ok(v)
}

fn read_u64_le(b: &[u8], pos: &mut usize) -> Result<u64, String> {
    if *pos + 8 > b.len() {
        return Err("数据截断 (u64)".into());
    }
    let v = u64::from_le_bytes(b[*pos..*pos + 8].try_into().unwrap());
    *pos += 8;
    Ok(v)
}

fn read_cstring(b: &[u8], pos: &mut usize) -> Result<String, String> {
    let start = *pos;
    while *pos < b.len() && b[*pos] != 0 {
        *pos += 1;
    }
    if *pos >= b.len() {
        return Err("字符串未以 0 终止".into());
    }
    let s = String::from_utf8_lossy(&b[start..*pos]).into_owned();
    *pos += 1;
    Ok(s)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_basics() {
        assert!(glob_match("*.psd", "icon.psd"));
        assert!(glob_match("*.svn*", "a.svnx"));
        assert!(glob_match("*.git", "folder/.git"));
        assert!(!glob_match("*.psd", "icon.jpg"));
        assert!(glob_match("*.psd", "a/b/c.psd"));
    }

    #[test]
    fn pack_read_validate_extract_roundtrip() {
        let dir = std::env::temp_dir().join(format!("gma_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("lua/autorun")).unwrap();
        fs::write(dir.join("lua/autorun/init.lua"), b"print('hello')\n").unwrap();
        fs::write(dir.join("LICENSE.txt"), b"MIT License\n").unwrap();

        let mut meta = GmaMeta::default();
        meta.addon_name = "TestAddon".into();
        meta.addon_type = "gamemode".into();
        meta.tags = vec!["fun".into()];
        meta.ignore = vec!["*.tmp".into()];

        let out = dir.join("test.gma");
        let r = pack_folder(&dir, &meta, &out, |_, _, _| {});
        assert!(r.is_ok());
        let pr = r.unwrap();
        assert_eq!(pr.file_count, 2);

        // 读回
        let info = read_gma(&out).unwrap();
        assert_eq!(info.meta.addon_name, "TestAddon");
        assert_eq!(info.meta.addon_type, "gamemode");
        assert_eq!(info.meta.tags, vec!["fun"]);
        assert_eq!(info.entries.len(), 2);
        assert_eq!(info.entries[0].path, "LICENSE.txt");
        assert_eq!(info.entries[1].path, "lua/autorun/init.lua");

        // 校验
        let (ok, st) = validate_gma(&out, |_, _, _| {}).unwrap();
        assert!(ok);
        assert!(st.iter().all(|s| s.ok));

        // 解包并比对
        let outdir = dir.join("out");
        extract_gma(&out, &outdir, |_, _, _| {}).unwrap();
        let content = fs::read_to_string(outdir.join("lua/autorun/init.lua")).unwrap();
        assert_eq!(content, "print('hello')\n");
        assert_eq!(
            fs::read_to_string(outdir.join("LICENSE.txt")).unwrap(),
            "MIT License\n"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// addon.json 写读往返。
    #[test]
    fn addon_json_roundtrip() {
        let dir = std::env::temp_dir().join(format!("gma_json_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut meta = GmaMeta::default();
        meta.addon_name = "MyAddon".into();
        meta.addon_type = "gamemode".into();
        meta.tags = vec!["fun".into(), "roleplay".into()];
        meta.ignore = vec!["*.psd".into()];
        write_addon_json(&dir, &meta).unwrap();
        let a = read_addon_json(&dir.join("addon.json")).unwrap();
        assert_eq!(a.title, "MyAddon");
        assert_eq!(a.addon_type, "gamemode");
        assert_eq!(a.tags, vec!["fun", "roleplay"]);
        assert_eq!(a.ignore, vec!["*.psd"]);
        let _ = fs::remove_dir_all(&dir);
    }

    /// 对照官方文档逐字节校验头布局（独立于读取端，防读写共用同一偏移错误）。
    #[test]
    fn header_byte_layout_matches_spec() {
        let dir = std::env::temp_dir().join(format!("gma_hdr_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"hello").unwrap();

        let mut meta = GmaMeta::default();
        meta.steam_id = 0x1122334455667788;
        meta.timestamp = 1_700_000_000;
        meta.addon_name = "TestAddon".into();
        meta.addon_version = 7;

        let out = dir.join("hdr.gma");
        pack_folder(&dir, &meta, &out, |_, _, _| {}).unwrap();
        let b = fs::read(&out).unwrap();

        assert_eq!(&b[0..4], b"GMAD", "magic");
        assert_eq!(b[4], 0x03, "version");
        assert_eq!(
            u64::from_le_bytes(b[5..13].try_into().unwrap()),
            0x1122334455667788,
            "steam_id"
        );
        assert_eq!(
            u64::from_le_bytes(b[13..21].try_into().unwrap()),
            1_700_000_000,
            "timestamp"
        );
        assert_eq!(b[21], 0x00, "padding");

        let name_end = 22 + b[22..].iter().position(|&x| x == 0).unwrap();
        assert_eq!(&b[22..name_end], b"TestAddon", "addon_name NUL 结尾");
        let json_start = name_end + 1;
        let json_end = json_start + b[json_start..].iter().position(|&x| x == 0).unwrap();
        // gmad 头部在 meta_json 之后还有一个 author 字符串字段，需跳过（与写入端/读取端一致）
        let author_start = json_end + 1;
        let author_end = author_start + b[author_start..].iter().position(|&x| x == 0).unwrap();
        assert_eq!(&b[author_start..author_end], b"Author Name", "author 字段");
        assert_eq!(
            u32::from_le_bytes(b[author_end + 1..author_end + 5].try_into().unwrap()),
            7,
            "addon_version 紧跟 author 之后"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
