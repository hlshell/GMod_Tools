//! GoldSrc（Half-Life 1 / CS 1.6）MDL v10 解析器。
//!
//! 格式依据（三源交叉验证）：
//! - ValveSoftware/halflife `engine/studio.h`（结构布局与尺寸）
//! - assimp `code/AssetLib/MDL/HalfLife`（tricmds 条带/扇区、顶点骨骼 1 字节、
//!   皮肤表、动画值提取语义、纹理=索引+调色板）
//! - goldsrc-rs（`assert_eq_size` 全部结构尺寸）
//!
//! 输出与 Source 解析器完全一致的 `MdlMeshView`，渲染/动画/UI 全部复用。
//! 已知 best-effort 点（需真实模型验收）：
//! - 顶点/法线烘焙进绑定姿势（用与渲染器一致的 Rz*Ry*Rx 欧拉顺序）；
//! - 序列动画只解旋转通道（位置通道被省略），delta 区间语义严格照抄 assimp；
//! - 序号组 > 0 的外部动画文件（seqgroup 文件）不加载，直接降级为 0。

use super::{MdlAttachmentView, MdlBoneView, MdlBodyGroupView, MdlHitboxView, MdlMeshView, MdlSeqView, MdlTextureView};
use std::path::Path;

const GOLDSRC_VERSION: i32 = 10;
/// mstudioanimvalue_t 单位（2 字节）
const UNIT: usize = 2;

fn i32_at(b: &[u8], o: usize) -> i32 {
    if o + 4 <= b.len() {
        i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
    } else {
        0
    }
}
fn f32_at(b: &[u8], o: usize) -> f32 {
    if o + 4 <= b.len() {
        f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
    } else {
        0.0
    }
}
fn u16_at(b: &[u8], o: usize) -> u16 {
    if o + 2 <= b.len() {
        u16::from_le_bytes([b[o], b[o + 1]])
    } else {
        0
    }
}
fn cstr_at(b: &[u8], o: usize, max: usize) -> String {
    let end = (o..(o + max).min(b.len())).find(|&i| b[i] == 0).unwrap_or((o + max).min(b.len()));
    String::from_utf8_lossy(&b[o.min(b.len())..end]).into_owned()
}

/// 是否为 GoldSrc MDL（IDST + version 10）。
pub fn is_goldsrc(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && &bytes[0..4] == b"IDST" && i32_at(bytes, 4) == GOLDSRC_VERSION
}

fn lmsg(lang: &str, zh: &str, en: &str) -> String {
    if lang.starts_with("zh") || lang.is_empty() {
        zh.to_string()
    } else {
        en.to_string()
    }
}

// ---- 欧拉(度)→3x3（与 viewer 渲染器 Rz*Ry*Rx 一致，保证烘焙与蒙皮同构）----
fn rot_mat(deg: [f32; 3]) -> [f32; 9] {
    let (rx, ry, rz) = (deg[0].to_radians(), deg[1].to_radians(), deg[2].to_radians());
    let (sc, cc) = rx.sin_cos();
    let (sb, cb) = ry.sin_cos();
    let (sa, ca) = rz.sin_cos();
    [
        ca * cb,
        ca * sb * sc - sa * cc,
        ca * sb * cc + sa * sc,
        sa * cb,
        sa * sb * sc + ca * cc,
        sa * sb * cc - ca * sc,
        -sb,
        cb * sc,
        cb * cc,
    ]
}
/// 行主序 4x4（旋转 + 平移）。
fn rigid(rot: &[f32; 9], t: [f32; 3]) -> [f32; 16] {
    [
        rot[0], rot[1], rot[2], t[0],
        rot[3], rot[4], rot[5], t[1],
        rot[6], rot[7], rot[8], t[2],
        0.0, 0.0, 0.0, 1.0,
    ]
}
fn mat4_mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut o = [0.0f32; 16];
    for r in 0..4 {
        for c in 0..4 {
            o[r * 4 + c] = a[r * 4] * b[c] + a[r * 4 + 1] * b[4 + c] + a[r * 4 + 2] * b[8 + c] + a[r * 4 + 3] * b[12 + c];
        }
    }
    o
}
fn mat4_x(m: &[f32; 16], v: [f32; 3]) -> [f32; 3] {
    [
        m[0] * v[0] + m[1] * v[1] + m[2] * v[2] + m[3],
        m[4] * v[0] + m[5] * v[1] + m[6] * v[2] + m[7],
        m[8] * v[0] + m[9] * v[1] + m[10] * v[2] + m[11],
    ]
}

/// 解析 GoldSrc MDL v10 → MdlMeshView。
pub fn mesh_data(mdl_path: &Path, body_sel: &[i32], lang: &str) -> Result<MdlMeshView, String> {
    let b = std::fs::read(mdl_path).map_err(|e| e.to_string())?;
    if b.len() < 244 {
        return Err(lmsg(lang, "GoldSrc MDL 文件过短", "GoldSrc MDL file too short"));
    }

    // ---- 头 ----
    let numbones = i32_at(&b, 140).max(0) as usize;
    let boneindex = i32_at(&b, 144).max(0) as usize;
    let nseq = i32_at(&b, 164).max(0) as usize;
    let seqindex = i32_at(&b, 168).max(0) as usize;
    let ntex = i32_at(&b, 180).max(0) as usize;
    let texindex = i32_at(&b, 184).max(0) as usize;
    let numskinref = i32_at(&b, 192).max(0) as usize;
    let numskinfamilies = i32_at(&b, 196).max(0) as usize;
    let skinindex = i32_at(&b, 200).max(0) as usize;
    let nbp = i32_at(&b, 204).max(0) as usize;
    let bpindex = i32_at(&b, 208).max(0) as usize;
    let natt = i32_at(&b, 212).max(0) as usize;
    let attindex = i32_at(&b, 216).max(0) as usize;
    let nhit = i32_at(&b, 156).max(0) as usize;
    let hitindex = i32_at(&b, 160).max(0) as usize;

    // ---- 骨骼 ----
    let mut bones: Vec<MdlBoneView> = Vec::new();
    if numbones > 0 && boneindex + numbones * 112 <= b.len() {
        for i in 0..numbones {
            let bo = boneindex + i * 112;
            bones.push(MdlBoneView {
                name: cstr_at(&b, bo, 32),
                parent: i32_at(&b, bo + 32),
                pos: [f32_at(&b, bo + 64), f32_at(&b, bo + 68), f32_at(&b, bo + 72)],
                rot: [
                    f32_at(&b, bo + 76).to_degrees(),
                    f32_at(&b, bo + 80).to_degrees(),
                    f32_at(&b, bo + 84).to_degrees(),
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

    // ---- 贴图（8-bit 索引 + 768B 调色板；mip 链 4 级；兼容 16B BMP 头嗅探）----
    let mut textures: Vec<MdlTextureView> = Vec::new();
    if ntex > 0 && texindex + ntex * 80 <= b.len() {
        for i in 0..ntex {
            let t0 = texindex + i * 80;
            let w = i32_at(&b, t0 + 68).max(0) as usize;
            let h = i32_at(&b, t0 + 72).max(0) as usize;
            let off = i32_at(&b, t0 + 76).max(0) as usize;
            let mut t = MdlTextureView {
                name: cstr_at(&b, t0, 64),
                ..Default::default()
            };
            if w == 0 || h == 0 || w * h > 4 * 1024 * 1024 {
                textures.push(t);
                continue;
            }
            let mut po = off;
            if po + 2 <= b.len() && b[po] == b'B' && b[po + 1] == b'M' {
                po += 16; // 16 字节 BMP 头
            }
            let mip = |div: u32| -> usize { ((w >> div).max(1)) * ((h >> div).max(1)) };
            let chain = mip(0) + mip(1) + mip(2) + mip(3);
            if po + chain + 768 > b.len() {
                textures.push(t);
                continue;
            }
            let idx = &b[po..po + w * h];
            let pal = &b[po + chain..po + chain + 768];
            let mut rgba = Vec::with_capacity(w * h * 4);
            for &ix in idx {
                let p = (ix as usize) * 3;
                let r = if p + 2 < 768 { pal[p] } else { 0 };
                let g = if p + 2 < 768 { pal[p + 1] } else { 0 };
                let bl = if p + 2 < 768 { pal[p + 2] } else { 0 };
                rgba.extend_from_slice(&[r, g, bl, 255]);
            }
            t.width = w as u32;
            t.height = h as u32;
            t.rgba = rgba;
            t.path = Some(mdl_path.to_string_lossy().into_owned());
            textures.push(t);
        }
    }

    // ---- 皮肤族（u16 表：numskinfamilies × numskinref）----
    let mut skins: Vec<Vec<i32>> = Vec::new();
    if numskinref > 0 && numskinfamilies > 0 && skinindex + numskinfamilies * numskinref * 2 <= b.len() {
        for f in 0..numskinfamilies {
            let mut row = Vec::with_capacity(numskinref);
            for s in 0..numskinref {
                let v = u16_at(&b, skinindex + (f * numskinref + s) * 2) as i32;
                row.push(if v >= 0 && (v as usize) < textures.len() { v } else { -1 });
            }
            skins.push(row);
        }
    }

    // ---- 序列（内嵌组 0 才有动画数据）----
    let mut sequences: Vec<MdlSeqView> = Vec::new();
    if nseq > 0 && seqindex + nseq * 176 <= b.len() {
        for i in 0..nseq {
            let s = seqindex + i * 176;
            let frames = i32_at(&b, s + 56).clamp(0, 100000);
            sequences.push(MdlSeqView {
                name: cstr_at(&b, s, 32),
                activity: i32_at(&b, s + 40).to_string(),
                frames,
                fps: f32_at(&b, s + 32).clamp(0.0, 120.0),
            });
        }
    }

    // ---- 身体组 / 几何 ----
    let mut bodygroups: Vec<MdlBodyGroupView> = Vec::new();
    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut uvs: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut mat_of_tri: Vec<i32> = Vec::new();
    let mut tri_skinref: Vec<i32> = Vec::new();
    let mut bone_indices: Vec<[i32; 4]> = Vec::new();
    let mut bone_weights: Vec<[f32; 4]> = Vec::new();
    let mut tri_count = 0u32;
    let mut mn = [f32::MAX; 3];
    let mut mx = [f32::MIN; 3];

    // 骨骼绑定世界变换（与渲染器 build_bone_rig 的 bind_world 同构）
    let mut bind_world = vec![[0.0f32; 16]; bones.len()];
    for i in 0..bones.len() {
        let local = rigid(&rot_mat(bones[i].rot), bones[i].pos);
        let p = bones[i].parent;
        bind_world[i] = if p >= 0 && (p as usize) < bones.len() {
            mat4_mul(&bind_world[p as usize], &local)
        } else {
            local
        };
    }
    let bake = |local: [f32; 3], bone: i32| -> [f32; 3] {
        if bone >= 0 && (bone as usize) < bind_world.len() {
            mat4_x(&bind_world[bone as usize], local)
        } else {
            local
        }
    };

    if nbp > 0 && bpindex + nbp * 76 <= b.len() {
        for bp_i in 0..nbp {
            let bp = bpindex + bp_i * 76;
            let nmod = i32_at(&b, bp + 64).max(0) as usize;
            let mbase = bp + i32_at(&b, bp + 72).max(0) as usize;
            let mut models: Vec<String> = Vec::new();
            for m in 0..nmod {
                let mo = mbase + m * 112;
                if mo + 112 <= b.len() {
                    models.push(cstr_at(&b, mo, 64));
                }
            }
            bodygroups.push(MdlBodyGroupView {
                name: cstr_at(&b, bp, 64),
                models,
            });
            let sel = body_sel.get(bp_i).copied().unwrap_or(0).max(0) as usize;
            if sel >= nmod {
                continue;
            }
            let mo = mbase + sel * 112;
            if mo + 112 > b.len() {
                continue;
            }
            let nmesh = i32_at(&b, mo + 72).max(0) as usize;
            let meshindex = mo + i32_at(&b, mo + 76).max(0) as usize;
            let nverts = i32_at(&b, mo + 80).max(0) as usize;
            let vinfo = mo + i32_at(&b, mo + 84).max(0) as usize;
            let vdata = mo + i32_at(&b, mo + 88).max(0) as usize;
            let nnorms = i32_at(&b, mo + 92).max(0) as usize;
            let ninfo = mo + i32_at(&b, mo + 96).max(0) as usize;
            let ndata = mo + i32_at(&b, mo + 100).max(0) as usize;
            if vdata + nverts * 12 > b.len() || vinfo + nverts > b.len() || ndata + nnorms * 12 > b.len() || ninfo + nnorms > b.len() {
                continue;
            }
            let vpos = |i: usize| -> [f32; 3] {
                let o = vdata + i * 12;
                [f32_at(&b, o), f32_at(&b, o + 4), f32_at(&b, o + 8)]
            };
            let vnorm = |i: usize| -> [f32; 3] {
                let o = ndata + i * 12;
                [f32_at(&b, o), f32_at(&b, o + 4), f32_at(&b, o + 8)]
            };
            for m in 0..nmesh {
                let ms = meshindex + m * 20;
                if ms + 20 > b.len() {
                    break;
                }
                let trioff = ms + i32_at(&b, ms + 4).max(0) as usize;
                let skinref = i32_at(&b, ms + 8);
                // 皮肤族 0 的材质
                let mat = skins
                    .first()
                    .and_then(|row| row.get(skinref.clamp(0, i32::MAX) as usize))
                    .copied()
                    .unwrap_or(skinref);
                // tricmds：i16 计数；<0 扇形 / >0 条带 / 0 结束
                let mut cur = trioff;
                let mut dedup: std::collections::HashMap<(i32, i32, i32, i32), u32> = std::collections::HashMap::new();
                loop {
                    if cur + 2 > b.len() {
                        break;
                    }
                    let raw = u16_at(&b, cur) as i16;
                    cur += 2;
                    if raw == 0 {
                        break;
                    }
                    let (fan, count) = if raw < 0 { (true, (-raw as i32) as usize) } else { (false, raw as usize) };
                    if count == 0 || count > 1_000_000 {
                        break;
                    }
                    let mut ids: Vec<u32> = Vec::with_capacity(count);
                    let mut bad = false;
                    for _ in 0..count {
                        if cur + 8 > b.len() {
                            bad = true;
                            break;
                        }
                        let vi = i32_at(&b, cur) as i16 as i32;
                        let ni = i32_at(&b, cur + 2) as i16 as i32;
                        let s = i32_at(&b, cur + 4) as i16 as i32;
                        let t = i32_at(&b, cur + 6) as i16 as i32;
                        cur += 8;
                        if vi < 0 || (vi as usize) >= nverts || ni < 0 || (ni as usize) >= nnorms {
                            bad = true;
                            break;
                        }
                        let key = (vi, ni, s, t);
                        let id = match dedup.get(&key) {
                            Some(&id) => id,
                            None => {
                                let bone = b[vinfo + vi as usize] as i32;
                                let bp2 = bake(vpos(vi as usize), bone);
                                let n = vnorm(ni as usize);
                                let texw = mat as usize;
                                let (tw, th) = textures
                                    .get(texw)
                                    .map(|t| (t.width.max(1) as f32, t.height.max(1) as f32))
                                    .unwrap_or((1.0, 1.0));
                                positions.extend_from_slice(&[bp2[0], bp2[1], bp2[2]]);
                                normals.extend_from_slice(&[n[0], n[1], n[2]]);
                                uvs.push(s as f32 / tw);
                                uvs.push(t as f32 / th);
                                bone_indices.push([bone, -1, -1, -1]);
                                bone_weights.push([1.0, 0.0, 0.0, 0.0]);
                                mn[0] = mn[0].min(bp2[0]);
                                mn[1] = mn[1].min(bp2[1]);
                                mn[2] = mn[2].min(bp2[2]);
                                mx[0] = mx[0].max(bp2[0]);
                                mx[1] = mx[1].max(bp2[1]);
                                mx[2] = mx[2].max(bp2[2]);
                                let id = (positions.len() / 3 - 1) as u32;
                                dedup.insert(key, id);
                                id
                            }
                        };
                        ids.push(id);
                    }
                    if bad {
                        break;
                    }
                    // 生成三角形（winding 与 assimp 一致：v2,v1,v0）
                    if fan {
                        for k in 1..ids.len().saturating_sub(1) {
                            let (a, b2, c) = (ids[k + 1], ids[k], ids[0]);
                            indices.extend_from_slice(&[a, b2, c]);
                            mat_of_tri.push(mat);
                            tri_skinref.push(skinref);
                            tri_count += 1;
                        }
                    } else {
                        for k in 0..ids.len().saturating_sub(2) {
                            let (a, b2, c) = if k % 2 == 0 {
                                (ids[k + 2], ids[k + 1], ids[k])
                            } else {
                                (ids[k + 2], ids[k], ids[k + 1])
                            };
                            indices.extend_from_slice(&[a, b2, c]);
                            mat_of_tri.push(mat);
                            tri_skinref.push(skinref);
                            tri_count += 1;
                        }
                    }
                }
            }
        }
    }

    if positions.is_empty() || tri_count == 0 {
        return Err(lmsg(lang, "未能解析到任何 GoldSrc 网格几何", "No GoldSrc mesh geometry parsed"));
    }
    if mn[0] > mx[0] {
        mn = [0.0, 0.0, 0.0];
        mx = [1.0, 1.0, 1.0];
    }

    // ---- 命中框（模型空间 AABB）----
    let mut hitboxes: Vec<MdlHitboxView> = Vec::new();
    if nhit > 0 && hitindex + nhit * 32 <= b.len() {
        for i in 0..nhit {
            let h = hitindex + i * 32;
            hitboxes.push(MdlHitboxView {
                name: format!("hitbox{}", i),
                bone: i32_at(&b, h),
                group: i32_at(&b, h + 4),
                mins: [f32_at(&b, h + 8), f32_at(&b, h + 12), f32_at(&b, h + 16)],
                maxs: [f32_at(&b, h + 20), f32_at(&b, h + 24), f32_at(&b, h + 28)],
            });
        }
    }

    // ---- 附着点 ----
    let mut attachments: Vec<MdlAttachmentView> = Vec::new();
    if natt > 0 && attindex + natt * 88 <= b.len() {
        for i in 0..natt {
            let a = attindex + i * 88;
            attachments.push(MdlAttachmentView {
                name: cstr_at(&b, a, 32),
                bone: i32_at(&b, a + 36),
                pos: [f32_at(&b, a + 40), f32_at(&b, a + 44), f32_at(&b, a + 48)],
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
        bounds: [mn[0], mn[1], mn[2], mx[0], mx[1], mx[2]],
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

/// 解码指定序列的骨骼旋转动画：返回每帧每骨骼的欧拉旋转增量（度）。
/// 语义严格照抄 assimp `extract_anim_value`（已被真实模型验证的 walk + value 提取）；
/// 只解旋转通道（offset[3..6]），位置通道省略。越界安全降级 0。
pub fn sequence_anim(mdl_path: &Path, seq_index: i32, lang: &str) -> Result<Vec<Vec<[f32; 3]>>, String> {
    let b = std::fs::read(mdl_path).map_err(|e| e.to_string())?;
    if b.len() < 244 {
        return Err(lmsg(lang, "GoldSrc MDL 文件过短", "GoldSrc MDL file too short"));
    }
    let nseq = i32_at(&b, 164).max(0) as usize;
    if seq_index < 0 || (seq_index as usize) >= nseq {
        return Err(lmsg(lang, "序列序号越界", "Sequence index out of range"));
    }
    let seqindex = i32_at(&b, 168).max(0) as usize;
    let s = seqindex + (seq_index as usize) * 176;
    if s + 176 > b.len() {
        return Err(lmsg(lang, "序列描述越界", "Sequence descriptor out of range"));
    }
    let nframes = i32_at(&b, s + 56).clamp(0, 100000) as usize;
    let numbones = i32_at(&b, 140).max(0) as usize;
    let boneindex = i32_at(&b, 144).max(0) as usize;
    let seqgroup = i32_at(&b, s + 156);
    let numblends = i32_at(&b, s + 120).max(0) as usize;
    let animindex = i32_at(&b, s + 124).max(0) as usize;
    if nframes == 0 || numbones == 0 {
        return Ok(vec![vec![[0.0f32, 0.0, 0.0]; numbones]; nframes.max(1)]);
    }
    // 只支持内嵌组 0（外部 seqgroup 文件不加载）
    if seqgroup != 0 {
        return Ok(vec![vec![[0.0f32, 0.0, 0.0]; numbones]; nframes]);
    }
    let blend = numblends.min(1);
    let mut frames: Vec<Vec<[f32; 3]>> = vec![vec![[0.0f32, 0.0, 0.0]; numbones]; nframes];
    for bone in 0..numbones {
        // [blend][bone] 记录：12B = 6 × u16 偏移（相对记录自身）
        let rec = animindex + (blend * numbones + bone) * 12;
        if rec + 12 > b.len() {
            continue;
        }
        let mut chans: [Vec<f32>; 3] = [vec![0.0; nframes], vec![0.0; nframes], vec![0.0; nframes]];
        for c in 0..3 {
            let off = u16_at(&b, rec + (c + 3) * 2) as usize;
            if off == 0 {
                continue; // 该通道无动画
            }
            let start = rec + off;
            for f in 0..nframes {
                let mut ptr = start;
                let mut k = f;
                // assimp walk：while total <= k { k -= total; ptr += (valid+1) units }
                let mut found = false;
                loop {
                    if ptr + 2 > b.len() {
                        break;
                    }
                    let total = b[ptr + 1] as usize;
                    if total == 0 {
                        break;
                    }
                    if total > k {
                        found = true;
                        break;
                    }
                    k -= total;
                    let valid = b[ptr] as usize;
                    ptr += (valid + 1) * UNIT;
                }
                if !found {
                    continue;
                }
                let valid = b[ptr] as usize;
                let idx = if valid > k { k + 1 } else { valid };
                if ptr + (idx + 1) * 2 > b.len() {
                    continue;
                }
                let raw = u16_at(&b, ptr + idx * 2) as i16;
                let scale = if boneindex + bone * 112 + 92 <= b.len() {
                    f32_at(&b, boneindex + bone * 112 + 88 + c * 4)
                } else {
                    1.0
                };
                chans[c][f] = raw as f32 * scale;
            }
        }
        for f in 0..nframes {
            frames[f][bone] = [chans[0][f].to_degrees(), chans[1][f].to_degrees(), chans[2][f].to_degrees()];
        }
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_goldsrc_mdl() -> Vec<u8> {
        // 合成一个最小的 GoldSrc MDL：单骨骼、单身体组、单模型、单网格三角形、1 贴图、1 序列
        let mut b = vec![0u8; 244];
        b[0..4].copy_from_slice(b"IDST");
        b[4..8].copy_from_slice(&10i32.to_le_bytes());
        b[8..12].copy_from_slice(&244u32.to_le_bytes()); // name 占位
        // 骨骼 × 1 @ 244
        let bone_off = 244;
        let mut bone = vec![0u8; 112];
        bone[32..36].copy_from_slice(&(-1i32).to_le_bytes()); // parent
        bone[64..68].copy_from_slice(&0f32.to_le_bytes()); // pos
        bone[88..92].copy_from_slice(&1f32.to_le_bytes()); // scale XR = 1 弧度/单位
        // 贴图 × 1 @ bone_off+112
        let tex_off = bone_off + 112;
        let mut tex = vec![0u8; 80];
        tex[68..72].copy_from_slice(&2u32.to_le_bytes()); // w=2
        tex[72..76].copy_from_slice(&2u32.to_le_bytes()); // h=2
        // 纹理数据：4 像素索引 + mip(1×1) + palette(768)
        let mut tdata = vec![0u8; 4 + 1 + 768];
        tdata[0] = 0;
        tdata[1] = 1;
        tdata[2] = 2;
        tdata[3] = 3;
        tdata[4] = 0;
        // 调色板：前 4 色
        tdata[5..8].copy_from_slice(&[255, 0, 0]);
        tdata[8..11].copy_from_slice(&[0, 255, 0]);
        tdata[11..14].copy_from_slice(&[0, 0, 255]);
        tdata[14..17].copy_from_slice(&[255, 255, 255]);
        let tdata_off = tex_off + 80;
        tex[76..80].copy_from_slice(&(tdata_off as u32).to_le_bytes());
        // 皮肤表（1 族 × 1 ref）@ tdata 后
        let skin_off = tdata_off + tdata.len();
        // 序列 × 1 @ skin_off + 2
        let seq_off = skin_off + 2;
        let mut seq = vec![0u8; 176];
        seq[32..36].copy_from_slice(&30f32.to_le_bytes()); // fps
        seq[56..60].copy_from_slice(&1i32.to_le_bytes()); // numframes
        seq[120..124].copy_from_slice(&1i32.to_le_bytes()); // numblends
        // animindex 指向 anim 表：1 blend × 1 bone × 12B 记录
        let anim_off = seq_off + 176;
        seq[124..128].copy_from_slice(&((anim_off - seq_off) as u32).to_le_bytes());
        // anim 记录：1 根骨骼，3 个旋转通道 offset 指向各通道 animvalue
        let mut anim = vec![0u8; 12];
        // 通道 XR：offset=12（记录后紧跟 3 个通道的 animvalue）
        anim[6..8].copy_from_slice(&12u16.to_le_bytes());
        anim[8..10].copy_from_slice(&14u16.to_le_bytes());
        anim[10..12].copy_from_slice(&16u16.to_le_bytes());
        // animvalue：XR=[valid=1,total=1,short(128)] YR/ZR 同上（共 3×(2+2)）
        let mut av = Vec::new();
        for _ in 0..3 {
            av.extend_from_slice(&[1u8, 1u8]);
            av.extend_from_slice(&((128 as i16).to_le_bytes()));
        }
        let anim_end = anim_off + 12 + av.len();
        // 身体组 × 1 @ anim_end
        let bp_off = anim_end;
        let mut bp = vec![0u8; 76];
        bp[64..68].copy_from_slice(&1u32.to_le_bytes()); // nummodels
        // 模型 @ bp_off + 76
        bp[72..76].copy_from_slice(&(76i32).to_le_bytes()); // modelindex（相对身体组）
        let mut model = vec![0u8; 112];
        model[72..76].copy_from_slice(&1i32.to_le_bytes()); // nummesh
        let mesh_off_rel = 112;
        model[76..80].copy_from_slice(&(mesh_off_rel as i32).to_le_bytes()); // meshindex
        model[80..84].copy_from_slice(&3u32.to_le_bytes()); // numverts
        let vinfo_rel = 112 + 20;
        model[84..88].copy_from_slice(&(vinfo_rel as i32).to_le_bytes()); // vertinfoindex（相对模型）
        model[88..92].copy_from_slice(&(112i32 + 20 + 3).to_le_bytes()); // vertindex = 模型后 3 字节骨骼 + 3×12 顶点
        model[92..96].copy_from_slice(&3u32.to_le_bytes()); // numnorms
        model[96..100].copy_from_slice(&(112i32 + 20 + 3 + 36).to_le_bytes()); // norminfoindex
        model[100..104].copy_from_slice(&(112i32 + 20 + 3 + 36 + 3).to_le_bytes()); // normindex
        // 模型内数据：mesh(20) + vinfo(3) + vdata(3×12) + ninfo(3) + ndata(3×12)
        let mut mdata = vec![0u8; 0];
        // mesh：numtris=4 trivert 计数前缀（1 条带 4 点 → 2 三角形），triindex 相对 mesh
        let mut mesh = vec![0u8; 20];
        mesh[8..12].copy_from_slice(&0i32.to_le_bytes()); // skinref = 0
        let mut tricmds = Vec::new();
        tricmds.extend_from_slice(&4i16.to_le_bytes()); // 条带 4 点
        // 4 trivert：(vi,ni,s,t) 各 8B；vi 循环 0,1,2,0（只定义 3 个顶点）
        for k in 0..4u16 {
            let vi = k % 3;
            tricmds.extend_from_slice(&vi.to_le_bytes());
            tricmds.extend_from_slice(&vi.to_le_bytes());
            tricmds.extend_from_slice(&((k * 128) as i16).to_le_bytes()); // s
            tricmds.extend_from_slice(&0i16.to_le_bytes()); // t
        }
        tricmds.extend_from_slice(&0i16.to_le_bytes()); // 结束
        mesh[4..8].copy_from_slice(&(20 + tricmds.len() as i32).to_le_bytes()); // triindex（相对 mesh：mdata 内偏移）
        // 注意：triindex 相对 mesh 起点 → mesh 在 mdata 开头，故 triindex = 20 + tricmds.len() 之前的内容长度
        // 重新计算：mdata 布局 = mesh(20) + vinfo(3) + vdata(36) + ninfo(3) + ndata(36)
        // 但 tricmds 需要空间 —— 简化：tricmds 放在 mdata 末尾，triindex 指向它
        let mesh_len = 20;
        let vinfo_at = mesh_len;
        let vdata_at = vinfo_at + 3;
        let ninfo_at = vdata_at + 36;
        let ndata_at = ninfo_at + 3;
        let tricmds_at = ndata_at + 36;
        mesh[4..8].copy_from_slice(&(tricmds_at as i32).to_le_bytes());
        // meshindex 保持 =112（mdata 起点相对模型），不得覆盖
        // 顶点数据（vdata 36B）+ vinfo 3B（全 bone 0）
        let mut vinfo = vec![0u8; 3];
        let mut vdata = Vec::new();
        for k in 0..3 {
            let x = k as f32;
            vdata.extend_from_slice(&x.to_le_bytes());
            vdata.extend_from_slice(&0f32.to_le_bytes());
            vdata.extend_from_slice(&0f32.to_le_bytes());
        }
        let ninfo = vec![0u8; 3];
        let mut ndata = Vec::new();
        for _ in 0..3 {
            ndata.extend_from_slice(&0f32.to_le_bytes());
            ndata.extend_from_slice(&0f32.to_le_bytes());
            ndata.extend_from_slice(&1f32.to_le_bytes());
        }
        mdata.extend_from_slice(&mesh);
        mdata.extend_from_slice(&vinfo);
        mdata.extend_from_slice(&vdata);
        mdata.extend_from_slice(&ninfo);
        mdata.extend_from_slice(&ndata);
        mdata.extend_from_slice(&tricmds);
        // 组装
        b.extend_from_slice(&bone);
        b.extend_from_slice(&tex);
        b.extend_from_slice(&tdata);
        b.extend_from_slice(&0u16.to_le_bytes()); // skin 表：1 族 × 1 ref = 0
        b.extend_from_slice(&seq);
        b.extend_from_slice(&anim);
        b.extend_from_slice(&av);
        b.extend_from_slice(&bp);
        b.extend_from_slice(&model);
        b.extend_from_slice(&mdata);
        // 头指针
        b[140..144].copy_from_slice(&1i32.to_le_bytes());
        b[144..148].copy_from_slice(&(bone_off as i32).to_le_bytes());
        b[164..168].copy_from_slice(&1i32.to_le_bytes());
        b[168..172].copy_from_slice(&(seq_off as i32).to_le_bytes());
        b[180..184].copy_from_slice(&1u32.to_le_bytes());
        b[184..188].copy_from_slice(&(tex_off as i32).to_le_bytes());
        b[192..196].copy_from_slice(&1u32.to_le_bytes());
        b[196..200].copy_from_slice(&1u32.to_le_bytes());
        b[200..204].copy_from_slice(&(skin_off as i32).to_le_bytes());
        b[204..208].copy_from_slice(&1u32.to_le_bytes());
        b[208..212].copy_from_slice(&(bp_off as i32).to_le_bytes());
        let file_len = b.len() as u32;
        b[8..12].copy_from_slice(&file_len.to_le_bytes());
        b
    }

    #[test]
    fn goldsrc_mesh_basic() {
        let bytes = make_goldsrc_mdl();
        assert!(is_goldsrc(&bytes));
        let path = std::env::temp_dir().join("goldsrc_test.mdl");
        std::fs::write(&path, &bytes).unwrap();
        let m = mesh_data(&path, &[], "zh").unwrap();
        assert_eq!(m.bones.len(), 1);
        assert_eq!(m.bodygroups.len(), 1);
        assert_eq!(m.sequences.len(), 1);
        assert_eq!(m.sequences[0].fps, 30.0);
        assert_eq!(m.sequences[0].frames, 1);
        assert_eq!(m.textures.len(), 1);
        assert_eq!(m.textures[0].width, 2);
        assert_eq!(m.textures[0].height, 2);
        assert_eq!(m.textures[0].rgba.len(), 16);
        assert_eq!(m.skins.len(), 1);
        assert_eq!(m.skins[0], vec![0]);
        assert!(m.tri_count >= 2, "tri_count={}", m.tri_count);
        assert_eq!(m.positions.len() / 3, m.bone_indices.len());
        assert!(m.bounds[3] > m.bounds[0]);
        // 动画：1 帧，骨骼 0 旋转 = 128/…×scale → 非零增量
        let anim = sequence_anim(&path, 0, "zh").unwrap();
        assert_eq!(anim.len(), 1);
        assert_eq!(anim[0].len(), 1);
    }
}
