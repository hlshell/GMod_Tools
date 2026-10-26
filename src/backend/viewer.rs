//! 模型查看页：Slint 没有 WebGL，这里用**纯软件光栅化**把 .mdl 网格
//! 投影渲染成一张 RGBA 位图（着色 / 线框 / 贴图 + 骨骼 / 命中框 / 附着点 / 网格轴叠加），
//! 再交给 Slint 显示。旋转 / 缩放 / 平移由主界面交互触发重渲。

use super::MdlMeshView;

/// 一次渲染结果：宽、高、RGBA8 像素（透明背景）。
pub type RenderedImage = (u32, u32, Vec<u8>);

/// 相机。
#[derive(Clone, Copy)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub center: [f32; 3],
    /// 相机位移（屏幕空间平移，叠加在 center 上）
    pub pan: [f32; 3],
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: 0.7,
            pitch: -0.35,
            dist: 4.0,
            center: [0.0, 0.0, 0.0],
            pan: [0.0, 0.0, 0.0],
        }
    }
}

/// 渲染模式。
#[derive(Clone, Copy, PartialEq)]
pub enum RenderMode {
    /// 平滑着色（顶点法线）。
    Shaded,
    /// 线框。
    Wire,
    /// 贴图 + 光照。
    Textured,
    /// 全亮贴图（无光照调制，等价 HLMV 的 Fullbright）。
    Fullbright,
    /// 平面着色（面法线，等价 HLMV 的 Flat Shaded）。
    Flat,
    /// UV 展开视图：三角形在 UV 空间绘制并按贴图采样，叠加网格线。
    UV,
}

/// 渲染参数。
#[derive(Clone, Copy)]
pub struct RenderParams {
    pub mode: RenderMode,
    pub show_bones: bool,
    pub show_hitboxes: bool,
    pub show_attachments: bool,
    pub show_grid: bool,
    pub show_axes: bool,
    /// 叠加顶点/面法线短线（HLMV 的 Show Normals）。
    pub show_normals: bool,
    /// 叠加模型包围盒（HLMV 的 Show Bounding Box）。
    pub show_bounds: bool,
    /// 当前皮肤族（0..skins.len()）。
    pub skin_family: u32,
    /// 透视 FOV（弧度）；正交模式忽略。
    pub fov: f32,
    pub ortho: bool,
    pub bg: [u8; 3],
    pub w: u32,
    pub h: u32,
}

fn normalize3(a: [f32; 3]) -> [f32; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if l < 1e-9 {
        [0.0, 0.0, 0.0]
    } else {
        [a[0] / l, a[1] / l, a[2] / l]
    }
}
fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// 4x4 矩阵（列主序，与 WebGL 一致），辅助投影。
struct Mat4([f32; 16]);

impl Mat4 {
    fn perspective(fovy: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let f = 1.0 / (fovy / 2.0).tan();
        let nf = 1.0 / (near - far);
        Mat4([
            f / aspect,
            0.0,
            0.0,
            0.0,
            0.0,
            f,
            0.0,
            0.0,
            0.0,
            0.0,
            (far + near) * nf,
            -1.0,
            0.0,
            0.0,
            2.0 * far * near * nf,
            0.0,
        ])
    }

    /// 正交投影：把视空间 [l,r]×[b,t]×[n,f] 映射到 NDC。
    fn ortho(l: f32, r: f32, b: f32, t: f32, n: f32, f: f32) -> Mat4 {
        let rl = 1.0 / (r - l);
        let tb = 1.0 / (t - b);
        let nf = 1.0 / (n - f);
        Mat4([
            2.0 * rl,
            0.0,
            0.0,
            0.0,
            0.0,
            2.0 * tb,
            0.0,
            0.0,
            0.0,
            0.0,
            2.0 * nf,
            0.0,
            -(r + l) * rl,
            -(t + b) * tb,
            (f + n) * nf,
            1.0,
        ])
    }

    fn look_at(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> Mat4 {
        let z = normalize3([center[0] - eye[0], center[1] - eye[1], center[2] - eye[2]]);
        let x = normalize3(cross3(up, z));
        let y = cross3(z, x);
        Mat4([
            x[0],
            y[0],
            z[0],
            0.0,
            x[1],
            y[1],
            z[1],
            0.0,
            x[2],
            y[2],
            z[2],
            0.0,
            -dot3(x, eye),
            -dot3(y, eye),
            -dot3(z, eye),
            1.0,
        ])
    }

    /// 变换点 (x,y,z,1) → (x',y',z',w')
    fn transform(&self, p: [f32; 4]) -> [f32; 4] {
        let m = &self.0;
        [
            m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12] * p[3],
            m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13] * p[3],
            m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14] * p[3],
            m[3] * p[0] + m[7] * p[1] + m[11] * p[2] + m[15] * p[3],
        ]
    }
}

/// 屏幕投影点；带贴图插值辅助量（invw = 1/clip.w，u/v 为未除 w 的 UV）。
#[derive(Clone, Copy)]
struct ScreenPt {
    x: f32,
    y: f32,
    depth: f32, // [0,1]，0=近 1=远
    invw: f32,
    uw: f32,
    vw: f32,
    ok: bool,
}

fn project(v: [f32; 3], view: &Mat4, proj: &Mat4, w: f32, h: f32) -> ScreenPt {
    let vw = view.transform([v[0], v[1], v[2], 1.0]);
    let p = proj.transform(vw);
    if p[3].abs() < 1e-6 {
        return ScreenPt { x: 0.0, y: 0.0, depth: 1.0, invw: 0.0, uw: 0.0, vw: 0.0, ok: false };
    }
    let ndc_x = p[0] / p[3];
    let ndc_y = p[1] / p[3];
    let ndc_z = p[2] / p[3];
    // 裁剪：超出 [-1.3,1.3] 就不画（简化处理，给边缘留一点余量避免正对镜头面被裁掉）
    let ok = ndc_x >= -1.3 && ndc_x <= 1.3 && ndc_y >= -1.3 && ndc_y <= 1.3 && ndc_z >= -1.3 && ndc_z <= 1.3;
    let invw = 1.0 / p[3];
    ScreenPt {
        x: (ndc_x + 1.0) * 0.5 * w,
        y: (1.0 - ndc_y) * 0.5 * h,
        depth: (ndc_z + 1.0) * 0.5,
        invw,
        uw: 0.0,
        vw: 0.0,
        ok,
    }
}

/// 带 UV 的投影（贴图模式使用）。
fn project_uv(v: [f32; 3], u: f32, vt: f32, view: &Mat4, proj: &Mat4, w: f32, h: f32) -> ScreenPt {
    let mut s = project(v, view, proj, w, h);
    if s.ok {
        s.uw = u * s.invw;
        s.vw = vt * s.invw;
    }
    s
}

fn put(px: &mut [u8], w: usize, x: i32, y: i32, r: u8, g: u8, b: u8, a: u8) {
    if x < 0 || y < 0 || x as usize >= w {
        return;
    }
    let i = (y as usize) * w + x as usize;
    let o = i * 4;
    px[o] = r;
    px[o + 1] = g;
    px[o + 2] = b;
    px[o + 3] = a;
}

fn line(px: &mut [u8], w: usize, h: usize, a0: ScreenPt, a1: ScreenPt, r: u8, g: u8, b: u8, a: u8) {
    if !a0.ok || !a1.ok {
        return;
    }
    let x0 = a0.x.round() as i32;
    let y0 = a0.y.round() as i32;
    let x1 = a1.x.round() as i32;
    let y1 = a1.y.round() as i32;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        if y >= 0 && y < h as i32 {
            put(px, w, x, y, r, g, b, a);
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// 填充三角形（z-buffer）。
fn fill_tri(
    px: &mut [u8],
    zbuf: &mut [f32],
    w: usize,
    h: usize,
    a: ScreenPt,
    b: ScreenPt,
    c: ScreenPt,
    cr: u8,
    cg: u8,
    cb: u8,
) {
    if !a.ok || !b.ok || !c.ok {
        return;
    }
    let minx = (a.x.min(b.x).min(c.x)).max(0.0) as i32;
    let maxx = (a.x.max(b.x).max(c.x)).min((w - 1) as f32) as i32;
    let miny = (a.y.min(b.y).min(c.y)).max(0.0) as i32;
    let maxy = (a.y.max(b.y).max(c.y)).min((h - 1) as f32) as i32;
    if minx > maxx || miny > maxy {
        return;
    }
    let area = (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
    if area.abs() < 1e-9 {
        return;
    }
    for y in miny..=maxy {
        for x in minx..=maxx {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let w0 = ((b.x - a.x) * (fy - a.y) - (b.y - a.y) * (fx - a.x)) / area;
            let w1 = ((c.x - a.x) * (fy - a.y) - (c.y - a.y) * (fx - a.x)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                let depth = w0 * a.depth + w1 * b.depth + w2 * c.depth;
                let i = y as usize * w + x as usize;
                if depth < zbuf[i] {
                    zbuf[i] = depth;
                    let o = i * 4;
                    px[o] = cr;
                    px[o + 1] = cg;
                    px[o + 2] = cb;
                    px[o + 3] = 255;
                }
            }
        }
    }
}

/// 双线性采样贴图（RGB 输出，alpha 用于全透明丢弃）。
fn sample_tex(tex: &[u8], tw: u32, th: u32, u: f32, v: f32) -> (u8, u8, u8, u8) {
    let mut u = u - u.floor();
    let mut v = v - v.floor();
    if u < 0.0 {
        u += 1.0;
    }
    if v < 0.0 {
        v += 1.0;
    }
    let fx = u * (tw as f32);
    let fy = v * (th as f32);
    let x0 = (fx as usize).min((tw - 1) as usize);
    let y0 = (fy as usize).min((th - 1) as usize);
    let x1 = if x0 + 1 < tw as usize { x0 + 1 } else { x0 };
    let y1 = if y0 + 1 < th as usize { y0 + 1 } else { y0 };
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;
    let px = |x: usize, y: usize| -> (u8, u8, u8, u8) {
        let o = (y * tw as usize + x) * 4;
        (tex[o], tex[o + 1], tex[o + 2], tex[o + 3])
    };
    let (r00, g00, b00, a00) = px(x0, y0);
    let (r10, g10, b10, a10) = px(x1, y0);
    let (r01, g01, b01, a01) = px(x0, y1);
    let (r11, g11, b11, a11) = px(x1, y1);
    let lerp = |a: u8, b: u8, c: u8, d: u8| -> u8 {
        let a0 = a as f32 + (b as f32 - a as f32) * tx;
        let a1 = c as f32 + (d as f32 - c as f32) * tx;
        (a0 + (a1 - a0) * ty).round().clamp(0.0, 255.0) as u8
    };
    (
        lerp(r00, r10, r01, r11),
        lerp(g00, g10, g01, g11),
        lerp(b00, b10, b01, b11),
        lerp(a00, a10, a01, a11),
    )
}

/// 透视校正填充三角形（带贴图采样）。
fn fill_tri_tex(
    px: &mut [u8],
    zbuf: &mut [f32],
    w: usize,
    h: usize,
    a: ScreenPt,
    b: ScreenPt,
    c: ScreenPt,
    light: f32,
    tex: &[u8],
    tw: u32,
    th: u32,
) {
    if !a.ok || !b.ok || !c.ok {
        return;
    }
    let minx = (a.x.min(b.x).min(c.x)).max(0.0) as i32;
    let maxx = (a.x.max(b.x).max(c.x)).min((w - 1) as f32) as i32;
    let miny = (a.y.min(b.y).min(c.y)).max(0.0) as i32;
    let maxy = (a.y.max(b.y).max(c.y)).min((h - 1) as f32) as i32;
    if minx > maxx || miny > maxy {
        return;
    }
    let area = (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
    if area.abs() < 1e-9 {
        return;
    }
    for y in miny..=maxy {
        for x in minx..=maxx {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let w0 = ((b.x - a.x) * (fy - a.y) - (b.y - a.y) * (fx - a.x)) / area;
            let w1 = ((c.x - a.x) * (fy - a.y) - (c.y - a.y) * (fx - a.x)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                let depth = w0 * a.depth + w1 * b.depth + w2 * c.depth;
                let i = y as usize * w + x as usize;
                if depth < zbuf[i] {
                    let invw = w0 * a.invw + w1 * b.invw + w2 * c.invw;
                    if invw.abs() > 1e-9 {
                        let u = (w0 * a.uw + w1 * b.uw + w2 * c.uw) / invw;
                        let vt = (w0 * a.vw + w1 * b.vw + w2 * c.vw) / invw;
                        let (r, g, bl, al) = sample_tex(tex, tw, th, u, vt);
                        if al > 8 {
                            zbuf[i] = depth;
                            let o = i * 4;
                            let k = (light * 255.0).clamp(0.0, 255.0);
                            px[o] = ((r as f32 * k) / 255.0) as u8;
                            px[o + 1] = ((g as f32 * k) / 255.0) as u8;
                            px[o + 2] = ((bl as f32 * k) / 255.0) as u8;
                            px[o + 3] = 255;
                        }
                    }
                }
            }
        }
    }
}

/// 面法线（由三角形三顶点计算，用于平面光照）。
fn face_light(p: &[f32], a: u32, b: u32, c: u32) -> f32 {
    let ax = p[a as usize * 3];
    let ay = p[a as usize * 3 + 1];
    let az = p[a as usize * 3 + 2];
    let bx = p[b as usize * 3];
    let by = p[b as usize * 3 + 1];
    let bz = p[b as usize * 3 + 2];
    let cx = p[c as usize * 3];
    let cy = p[c as usize * 3 + 1];
    let cz = p[c as usize * 3 + 2];
    let e1x = bx - ax;
    let e1y = by - ay;
    let e1z = bz - az;
    let e2x = cx - ax;
    let e2y = cy - ay;
    let e2z = cz - az;
    let nx = e1y * e2z - e1z * e2y;
    let ny = e1z * e2x - e1x * e2z;
    let nz = e1x * e2y - e1y * e2x;
    let n = normalize3([nx, ny, nz]);
    let light = normalize3([0.35, 0.75, 0.5]);
    dot3(n, light).abs()
}

/// 欧拉旋转矩阵（度 → 弧度；顺序 XYZ 应用到向量）。
fn euler_rotate(v: [f32; 3], deg: [f32; 3]) -> [f32; 3] {
    let (rx, ry, rz) = (
        deg[0].to_radians(),
        deg[1].to_radians(),
        deg[2].to_radians(),
    );
    // Rx
    let (sx, cx) = rx.sin_cos();
    let vx = [
        v[0],
        cx * v[1] - sx * v[2],
        sx * v[1] + cx * v[2],
    ];
    // Ry
    let (sy, cy) = ry.sin_cos();
    let vy = [
        cy * vx[0] + sy * vx[2],
        vx[1],
        -sy * vx[0] + cy * vx[2],
    ];
    // Rz
    let (sz, cz) = rz.sin_cos();
    [
        cz * vy[0] - sz * vy[1],
        sz * vy[0] + cz * vy[1],
        vy[2],
    ]
}

/// 欧拉角(度)→3x3 旋转矩阵（行主序，作用于列向量；顺序 Rz*Ry*Rx，与 euler_rotate 一致）。
fn euler_mat(deg: [f32; 3]) -> [f32; 9] {
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

/// 行主序 4x4 矩阵乘法。
fn mat4_mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut o = [0.0f32; 16];
    for r in 0..4 {
        for c in 0..4 {
            o[r * 4 + c] = a[r * 4] * b[c] + a[r * 4 + 1] * b[4 + c] + a[r * 4 + 2] * b[8 + c] + a[r * 4 + 3] * b[12 + c];
        }
    }
    o
}

/// 行主序 4x4 变换点（仿射，w=1）。
fn mat4_transform(m: &[f32; 16], v: [f32; 3]) -> [f32; 3] {
    [
        m[0] * v[0] + m[1] * v[1] + m[2] * v[2] + m[3],
        m[4] * v[0] + m[5] * v[1] + m[6] * v[2] + m[7],
        m[8] * v[0] + m[9] * v[1] + m[10] * v[2] + m[11],
    ]
}

/// 刚性变换逆（上 3x3 转置 + 平移取负）。
fn mat4_inv_rigid(m: &[f32; 16]) -> [f32; 16] {
    let r = [m[0], m[4], m[8], m[1], m[5], m[9], m[2], m[6], m[10]];
    let t = [m[3], m[7], m[11]];
    let tn = [
        -(r[0] * t[0] + r[3] * t[1] + r[6] * t[2]),
        -(r[1] * t[0] + r[4] * t[1] + r[7] * t[2]),
        -(r[2] * t[0] + r[5] * t[1] + r[8] * t[2]),
    ];
    [
        r[0], r[3], r[6], tn[0],
        r[1], r[4], r[7], tn[1],
        r[2], r[5], r[8], tn[2],
        0.0, 0.0, 0.0, 1.0,
    ]
}

/// 构建骨骼装配：返回 (每骨骼皮肤变换 skin[i]=pose_world[i]*inv(bind_world[i]),
/// 每骨骼姿态世界矩阵平移、供骨骼线绘制)。
fn build_bone_rig(mesh: &MdlMeshView, pose: Option<&[[f32; 3]]>) -> (Vec<[f32; 16]>, Vec<[f32; 3]>) {
    let nb = mesh.bones.len();
    let mut bind_world = vec![[0.0f32; 16]; nb];
    let mut pose_world = vec![[0.0f32; 16]; nb];
    for i in 0..nb {
        let m = euler_mat(mesh.bones[i].rot);
        let d = pose.and_then(|p| p.get(i)).copied().unwrap_or([0.0; 3]);
        let pm = euler_mat([
            mesh.bones[i].rot[0] + d[0],
            mesh.bones[i].rot[1] + d[1],
            mesh.bones[i].rot[2] + d[2],
        ]);
        let t = mesh.bones[i].pos;
        let local = [m[0], m[1], m[2], t[0], m[3], m[4], m[5], t[1], m[6], m[7], m[8], t[2], 0.0, 0.0, 0.0, 1.0];
        let plocal = [pm[0], pm[1], pm[2], t[0], pm[3], pm[4], pm[5], t[1], pm[6], pm[7], pm[8], t[2], 0.0, 0.0, 0.0, 1.0];
        let p = mesh.bones[i].parent;
        if p >= 0 && (p as usize) < nb {
            bind_world[i] = mat4_mul(&bind_world[p as usize], &local);
            pose_world[i] = mat4_mul(&pose_world[p as usize], &plocal);
        } else {
            bind_world[i] = local;
            pose_world[i] = plocal;
        }
    }
    let mut skin = vec![[0.0f32; 16]; nb];
    for i in 0..nb {
        skin[i] = mat4_mul(&pose_world[i], &mat4_inv_rigid(&bind_world[i]));
    }
    let mut trans = vec![[0.0f32; 3]; nb];
    for i in 0..nb {
        trans[i] = [pose_world[i][3], pose_world[i][7], pose_world[i][11]];
    }
    (skin, trans)
}

/// 顶点蒙皮：按每顶点骨骼权重把网格形变到当前姿势。
fn skin_mesh(mesh: &MdlMeshView, pose: Option<&[[f32; 3]]>) -> Vec<f32> {
    let (skin, _) = build_bone_rig(mesh, pose);
    let vc = mesh.positions.len() / 3;
    let mut out = vec![0.0f32; mesh.positions.len()];
    for vi in 0..vc {
        let v = [mesh.positions[vi * 3], mesh.positions[vi * 3 + 1], mesh.positions[vi * 3 + 2]];
        let bix = mesh.bone_indices.get(vi).copied().unwrap_or([-1, -1, -1, -1]);
        let bwt = mesh.bone_weights.get(vi).copied().unwrap_or([0.0; 4]);
        let sum = bwt[0] + bwt[1] + bwt[2] + bwt[3];
        let mut acc = [0.0f32; 3];
        for k in 0..4 {
            if bwt[k] <= 0.0 || sum <= 0.0 {
                continue;
            }
            let b = bix[k];
            if b < 0 || (b as usize) >= skin.len() {
                continue;
            }
            let w = bwt[k] / sum;
            let pv = mat4_transform(&skin[b as usize], v);
            acc[0] += w * pv[0];
            acc[1] += w * pv[1];
            acc[2] += w * pv[2];
        }
        out[vi * 3] = acc[0];
        out[vi * 3 + 1] = acc[1];
        out[vi * 3 + 2] = acc[2];
    }
    out
}

/// 由姿势覆盖计算骨骼世界位置（绑定姿势 + 局部旋转增量），供骨骼线绘制。
fn posed_bone_pos(mesh: &MdlMeshView, pose: Option<&[[f32; 3]]>) -> Vec<[f32; 3]> {
    let (_, trans) = build_bone_rig(mesh, pose);
    trans
}

/// UV 展开视图：把每个三角形按 UV 坐标绘制到 2D 画布（贴图采样 + 青色边线 + 网格线）。
fn render_uv(mesh: &MdlMeshView, px: &mut [u8], zbuf: &mut [f32], w: usize, h: usize, family: u32) {
    let wf = w as f32;
    let hf = h as f32;
    let uv_pt = |u: f32, v: f32| -> ScreenPt {
        let uu = u - u.floor();
        let vv = v - v.floor();
        ScreenPt {
            x: uu * wf,
            y: (1.0 - vv) * hf,
            depth: 0.0,
            invw: 1.0,
            uw: uu,
            vw: vv,
            ok: true,
        }
    };
    for t in 0..mesh.tri_count as usize {
        let i0 = mesh.indices[t * 3] as usize;
        let i1 = mesh.indices[t * 3 + 1] as usize;
        let i2 = mesh.indices[t * 3 + 2] as usize;
        let a = uv_pt(mesh.uvs[i0 * 2], mesh.uvs[i0 * 2 + 1]);
        let b = uv_pt(mesh.uvs[i1 * 2], mesh.uvs[i1 * 2 + 1]);
        let c = uv_pt(mesh.uvs[i2 * 2], mesh.uvs[i2 * 2 + 1]);
        let mat = if mesh.skins.is_empty() {
            mesh.mat_of_tri.get(t).copied().unwrap_or(-1)
        } else {
            super::skin_resolve(&mesh.skins, mesh.tri_skinref.get(t).copied().unwrap_or(-1), family)
        };
        if mat >= 0 && (mat as usize) < mesh.textures.len() && mesh.textures[mat as usize].width > 0 && mesh.textures[mat as usize].rgba.len() >= 4 {
            let tex = &mesh.textures[mat as usize];
            fill_tri_tex(px, zbuf, w, h, a, b, c, 1.0, &tex.rgba, tex.width, tex.height);
        } else {
            fill_tri(px, zbuf, w, h, a, b, c, 118, 122, 132);
        }
        line(px, w, h, a, b, 20, 190, 230, 255);
        line(px, w, h, b, c, 20, 190, 230, 255);
        line(px, w, h, c, a, 20, 190, 230, 255);
    }
    // 网格线（每 1/4）与边框
    for i in 0..=4 {
        let f = i as f32 * 0.25;
        let g = uv_pt(f, 0.0);
        let g2 = uv_pt(f, 1.0);
        let gv = uv_pt(0.0, f);
        let gv2 = uv_pt(1.0, f);
        line(px, w, h, g, g2, 70, 82, 96, 90);
        line(px, w, h, gv, gv2, 70, 82, 96, 90);
    }
    // 边框
    let tl = uv_pt(0.0, 0.0);
    let tr = uv_pt(1.0, 0.0);
    let bl = uv_pt(0.0, 1.0);
    let br = uv_pt(1.0, 1.0);
    line(px, w, h, tl, tr, 190, 205, 222, 255);
    line(px, w, h, tr, br, 190, 205, 222, 255);
    line(px, w, h, br, bl, 190, 205, 222, 255);
    line(px, w, h, bl, tl, 190, 205, 222, 255);
}

/// 渲染一张模型预览图。
pub fn render_preview(mesh: &MdlMeshView, cam: &Camera, p: &RenderParams, pose: Option<&[[f32; 3]]>) -> RenderedImage {
    let w = p.w.max(32);
    let h = p.h.max(32);
    let wu = w as usize;
    let hu = h as usize;
    let mut px = vec![0u8; wu * hu * 4];
    let mut zbuf = vec![1.0f32; wu * hu];

    // 背景
    for i in 0..wu * hu {
        let o = i * 4;
        px[o] = p.bg[0];
        px[o + 1] = p.bg[1];
        px[o + 2] = p.bg[2];
        px[o + 3] = 255;
    }

    // UV 展开视图：直接按 UV 空间绘制，不依赖相机。
    if p.mode == RenderMode::UV {
        render_uv(mesh, &mut px, &mut zbuf, wu, hu, p.skin_family);
        return (w, h, px);
    }

    // 蒙皮：有姿势（姿势编辑器/序列动画）时按骨骼权重形变网格
    let skinned = if pose.is_some() { Some(skin_mesh(mesh, pose)) } else { None };
    let posref: &[f32] = if let Some(s) = &skinned { s.as_slice() } else { &mesh.positions };

    let b = mesh.bounds;
    let mr = ((b[3] - b[0]).max(b[4] - b[1]).max(b[5] - b[2])).max(0.001) / 2.0;
    let dist = cam.dist.max(mr * 0.5);
    let near = (dist * 0.01).max(0.05);
    let far = ((dist + mr) * 3.0).max(200.0);
    let aspect = wu as f32 / hu as f32;
    // 相机中心 = 模型中心 + 屏幕空间平移
    let center = [cam.center[0] + cam.pan[0], cam.center[1] + cam.pan[1], cam.center[2] + cam.pan[2]];

    let proj = if p.ortho {
        // 正交：取视锥高度约为 dist 的两倍，保持视口大小稳定
        let vh = dist.max(mr * 1.6);
        let vw = vh * aspect;
        Mat4::ortho(-vw, vw, -vh, vh, near, far)
    } else {
        Mat4::perspective(p.fov.max(0.05), aspect, near, far)
    };

    let sp = cam.pitch.sin();
    let cp = cam.pitch.cos();
    let eye = [
        center[0] + cp * cam.yaw.sin() * dist,
        center[1] + sp * dist,
        center[2] + cp * cam.yaw.cos() * dist,
    ];
    let view = Mat4::look_at(eye, center, [0.0, 1.0, 0.0]);

    let wf = wu as f32;
    let hf = hu as f32;

    // 顶点 → 屏幕
    let vcount = posref.len() / 3;
    let mut scr: Vec<ScreenPt> = Vec::with_capacity(vcount);
    for i in 0..vcount {
        let v = [posref[i * 3], posref[i * 3 + 1], posref[i * 3 + 2]];
        scr.push(project(v, &view, &proj, wf, hf));
    }

    let shaded = p.mode != RenderMode::Wire;
    let textured = matches!(p.mode, RenderMode::Textured | RenderMode::Fullbright);
    let fullbright = p.mode == RenderMode::Fullbright;
    let flat = p.mode == RenderMode::Flat;

    // 三角形 → 贴图（皮肤族重映射）
    let tri_mat = |t: usize| -> i32 {
        if mesh.skins.is_empty() {
            mesh.mat_of_tri.get(t).copied().unwrap_or(-1)
        } else {
            super::skin_resolve(&mesh.skins, mesh.tri_skinref.get(t).copied().unwrap_or(-1), p.skin_family)
        }
    };

    if shaded {
        if textured {
            // 贴图模式：带透视校正 UV 的填充（Fullbright 不做光照调制）
            for t in 0..mesh.tri_count as usize {
                let i0 = mesh.indices[t * 3] as usize;
                let i1 = mesh.indices[t * 3 + 1] as usize;
                let i2 = mesh.indices[t * 3 + 2] as usize;
                let light = if fullbright {
                    1.0
                } else {
                    face_light(posref, i0 as u32, i1 as u32, i2 as u32)
                };
                let mat = tri_mat(t);
                if mat >= 0 && (mat as usize) < mesh.textures.len() && mesh.textures[mat as usize].width > 0 && mesh.textures[mat as usize].rgba.len() >= 4 {
                    let tex = &mesh.textures[mat as usize];
                    let a = project_uv(
                        [posref[i0 * 3], posref[i0 * 3 + 1], posref[i0 * 3 + 2]],
                        mesh.uvs[i0 * 2],
                        mesh.uvs[i0 * 2 + 1],
                        &view, &proj, wf, hf,
                    );
                    let bb = project_uv(
                        [posref[i1 * 3], posref[i1 * 3 + 1], posref[i1 * 3 + 2]],
                        mesh.uvs[i1 * 2],
                        mesh.uvs[i1 * 2 + 1],
                        &view, &proj, wf, hf,
                    );
                    let cc = project_uv(
                        [posref[i2 * 3], posref[i2 * 3 + 1], posref[i2 * 3 + 2]],
                        mesh.uvs[i2 * 2],
                        mesh.uvs[i2 * 2 + 1],
                        &view, &proj, wf, hf,
                    );
                    // 光照调制：0.55 亮度下限，避免过暗
                    let k = 0.55 + 0.45 * light;
                    fill_tri_tex(&mut px, &mut zbuf, wu, hu, a, bb, cc, k, &tex.rgba, tex.width, tex.height);
                } else {
                    // 无贴图：平面着色
                    let ci = 0.25 + 0.75 * light;
                    let (r, g, bl) = ((0.30 * 255.0 * ci) as u8, (0.62 * 255.0 * ci) as u8, (0.98 * 255.0 * ci) as u8);
                    fill_tri(&mut px, &mut zbuf, wu, hu, scr[i0], scr[i1], scr[i2], r, g, bl);
                }
            }
        } else if flat {
            // 平面着色：面法线光照（HLMV Flat Shaded）
            for t in 0..mesh.tri_count as usize {
                let i0 = mesh.indices[t * 3] as usize;
                let i1 = mesh.indices[t * 3 + 1] as usize;
                let i2 = mesh.indices[t * 3 + 2] as usize;
                let light = face_light(posref, i0 as u32, i1 as u32, i2 as u32);
                let ci = (0.22 + 0.78 * light).clamp(0.0, 1.0);
                let (r, g, b2) = (
                    (0.32 * 255.0 * ci) as u8,
                    (0.60 * 255.0 * ci) as u8,
                    (0.95 * 255.0 * ci) as u8,
                );
                fill_tri(&mut px, &mut zbuf, wu, hu, scr[i0], scr[i1], scr[i2], r, g, b2);
            }
        } else {
            // 着色：按顶点法线光照（平滑着色）
            let light = normalize3([0.35, 0.75, 0.5]);
            for t in 0..mesh.tri_count as usize {
                let i0 = mesh.indices[t * 3] as usize;
                let i1 = mesh.indices[t * 3 + 1] as usize;
                let i2 = mesh.indices[t * 3 + 2] as usize;
                let n = normalize3([
                    mesh.normals[i0 * 3],
                    mesh.normals[i0 * 3 + 1],
                    mesh.normals[i0 * 3 + 2],
                ]);
                let d = dot3(n, light).abs();
                let ci = (0.22 + 0.78 * d).clamp(0.0, 1.0);
                let (r, g, b2) = (
                    (0.30 * 255.0 * ci) as u8,
                    (0.62 * 255.0 * ci) as u8,
                    (0.98 * 255.0 * ci) as u8,
                );
                fill_tri(&mut px, &mut zbuf, wu, hu, scr[i0], scr[i1], scr[i2], r, g, b2);
            }
        }
    }

    // 边线
    let (er, eg, eb) = if shaded {
        if textured {
            (16u8, 26u8, 40u8)
        } else {
            (26u8, 41u8, 61u8)
        }
    } else {
        (46u8, 77u8, 112u8)
    };
    for t in 0..mesh.tri_count as usize {
        let i0 = mesh.indices[t * 3] as usize;
        let i1 = mesh.indices[t * 3 + 1] as usize;
        let i2 = mesh.indices[t * 3 + 2] as usize;
        line(&mut px, wu, hu, scr[i0], scr[i1], er, eg, eb, 255);
        line(&mut px, wu, hu, scr[i1], scr[i2], er, eg, eb, 255);
        line(&mut px, wu, hu, scr[i2], scr[i0], er, eg, eb, 255);
    }

    // 面法线叠加（HLMV Show Normals）：从三角形重心沿法线画短线
    if p.show_normals {
        let nlen = mr.max(0.1) * 0.1;
        for t in 0..mesh.tri_count as usize {
            let i0 = mesh.indices[t * 3] as usize;
            let i1 = mesh.indices[t * 3 + 1] as usize;
            let i2 = mesh.indices[t * 3 + 2] as usize;
            let a = [posref[i0 * 3], posref[i0 * 3 + 1], posref[i0 * 3 + 2]];
            let b2 = [posref[i1 * 3], posref[i1 * 3 + 1], posref[i1 * 3 + 2]];
            let c = [posref[i2 * 3], posref[i2 * 3 + 1], posref[i2 * 3 + 2]];
            let n = normalize3(cross3(
                [b2[0] - a[0], b2[1] - a[1], b2[2] - a[2]],
                [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
            ));
            let cen = [(a[0] + b2[0] + c[0]) / 3.0, (a[1] + b2[1] + c[1]) / 3.0, (a[2] + b2[2] + c[2]) / 3.0];
            let tip = [cen[0] + n[0] * nlen, cen[1] + n[1] * nlen, cen[2] + n[2] * nlen];
            let pa = project(cen, &view, &proj, wf, hf);
            let pb = project(tip, &view, &proj, wf, hf);
            line(&mut px, wu, hu, pa, pb, 120, 255, 230, 200);
        }
    }

    // 模型包围盒叠加（HLMV Show Bounding Box）
    if p.show_bounds {
        let (x0, y0, z0) = (b[0], b[1], b[2]);
        let (x1, y1, z1) = (b[3], b[4], b[5]);
        let corners = [
            [x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0],
            [x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1],
        ];
        let edges = [
            (0, 1), (1, 2), (2, 3), (3, 0),
            (4, 5), (5, 6), (6, 7), (7, 4),
            (0, 4), (1, 5), (2, 6), (3, 7),
        ];
        for (a, b) in edges {
            let pa = project(corners[a], &view, &proj, wf, hf);
            let pb = project(corners[b], &view, &proj, wf, hf);
            line(&mut px, wu, hu, pa, pb, 170, 170, 230, 220);
        }
    }

    // 网格地面（在模型后绘制：先画网格再画模型会覆盖，这里先于叠加层画在模型之上会更醒目；
    // 简单做法：作为底层辅助线，半透明画在背景上）
    if p.show_grid {
        let half = mr.max(1.0) * 1.4;
        let step = (half / 8.0).max(0.05);
        let y0 = center[1] - mr * 0.5; // 网格放在模型底部附近
        let mut n = 0i32;
        while n as f32 * step <= half {
            let off = n as f32 * step;
            let a = project([center[0] - half, y0, center[2] - off], &view, &proj, wf, hf);
            let b2 = project([center[0] + half, y0, center[2] - off], &view, &proj, wf, hf);
            let c = project([center[0] - off, y0, center[2] - half], &view, &proj, wf, hf);
            let d = project([center[0] - off, y0, center[2] + half], &view, &proj, wf, hf);
            line(&mut px, wu, hu, a, b2, 60, 66, 78, 120);
            line(&mut px, wu, hu, c, d, 60, 66, 78, 120);
            n += 1;
        }
    }

    // 坐标轴（X 红 / Y 绿 / Z 蓝）
    if p.show_axes {
        let alen = mr.max(0.5) * 0.6;
        let o = project(center, &view, &proj, wf, hf);
        let ax = project([center[0] + alen, center[1], center[2]], &view, &proj, wf, hf);
        let ay = project([center[0], center[1] + alen, center[2]], &view, &proj, wf, hf);
        let az = project([center[0], center[1], center[2] + alen], &view, &proj, wf, hf);
        line(&mut px, wu, hu, o, ax, 240, 90, 90, 255);
        line(&mut px, wu, hu, o, ay, 110, 230, 110, 255);
        line(&mut px, wu, hu, o, az, 100, 150, 250, 255);
    }

    // 骨骼线（应用姿势覆盖后的世界位置）
    if p.show_bones {
        let bonepos = posed_bone_pos(mesh, pose);
        for (i, bn) in mesh.bones.iter().enumerate() {
            if bn.parent < 0 {
                continue;
            }
            let pi = bn.parent as usize;
            let pa = project(bonepos[pi], &view, &proj, wf, hf);
            let ca = project(bonepos[i], &view, &proj, wf, hf);
            line(&mut px, wu, hu, pa, ca, 242, 89, 38, 255);
        }
        // 骨关节点
        for i in 0..mesh.bones.len() {
            let pt = project(bonepos[i], &view, &proj, wf, hf);
            if pt.ok {
                let x = pt.x.round() as i32;
                let y = pt.y.round() as i32;
                put(&mut px, wu, x, y, 255, 200, 60, 255);
                put(&mut px, wu, x - 1, y, 255, 200, 60, 255);
                put(&mut px, wu, x + 1, y, 255, 200, 60, 255);
                put(&mut px, wu, x, y - 1, 255, 200, 60, 255);
                put(&mut px, wu, x, y + 1, 255, 200, 60, 255);
            }
        }
    }

    // 命中框（AABB，8 条边）
    if p.show_hitboxes {
        for hb in &mesh.hitboxes {
            let (x0, y0, z0) = (hb.mins[0], hb.mins[1], hb.mins[2]);
            let (x1, y1, z1) = (hb.maxs[0], hb.maxs[1], hb.maxs[2]);
            let corners = [
                [x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0],
                [x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1],
            ];
            let edges = [
                (0, 1), (1, 2), (2, 3), (3, 0),
                (4, 5), (5, 6), (6, 7), (7, 4),
                (0, 4), (1, 5), (2, 6), (3, 7),
            ];
            for (a, b) in edges {
                let pa = project(corners[a], &view, &proj, wf, hf);
                let pb = project(corners[b], &view, &proj, wf, hf);
                line(&mut px, wu, hu, pa, pb, 250, 60, 60, 255);
            }
        }
    }

    // 附着点（十字标记）
    if p.show_attachments {
        for at in &mesh.attachments {
            let pt = project(at.pos, &view, &proj, wf, hf);
            if pt.ok {
                let x = pt.x.round() as i32;
                let y = pt.y.round() as i32;
                for i in -3..=3 {
                    put(&mut px, wu, x + i, y, 80, 255, 160, 255);
                    put(&mut px, wu, x, y + i, 80, 255, 160, 255);
                }
            }
        }
    }

    (w, h, px)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{MdlAttachmentView, MdlBoneView, MdlHitboxView, MdlTextureView};

    fn synth_mesh() -> MdlMeshView {
        MdlMeshView {
            bones: vec![
                MdlBoneView { name: "root".into(), parent: -1, pos: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], children: vec![1] },
                MdlBoneView { name: "child".into(), parent: 0, pos: [1.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], children: vec![] },
            ],
            positions: vec![
                -1.0, -1.0, 0.0, 1.0, -1.0, 0.0, 1.0, 1.0, 0.0,
                -1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, 1.0, 0.0,
            ],
            normals: [0.0f32, 0.0, 1.0].repeat(6),
            uvs: vec![0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0],
            indices: vec![0, 1, 2, 3, 4, 5],
            tri_count: 2,
            bounds: [-1.0, -1.0, -1.0, 1.0, 1.0, 1.0],
            sequences: vec![],
            bodygroups: vec![],
            hitboxes: vec![MdlHitboxView { name: "hb".into(), bone: 0, group: 0, mins: [-1.0, -1.0, -1.0], maxs: [1.0, 1.0, 1.0] }],
            attachments: vec![MdlAttachmentView { name: "at".into(), bone: 0, pos: [0.0, 0.0, 0.0] }],
            textures: vec![MdlTextureView { name: "t".into(), path: Some("x".into()), width: 2, height: 2, rgba: vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255] }],
            mat_of_tri: vec![0, 0],
            skins: vec![],
            tri_skinref: vec![0, 0],
            bone_indices: vec![[0, -1, -1, -1], [0, -1, -1, -1], [0, -1, -1, -1], [0, -1, -1, -1], [0, -1, -1, -1], [0, -1, -1, -1]],
            bone_weights: vec![[1.0, 0.0, 0.0, 0.0]; 6],
        }
    }

    #[test]
    fn render_all_modes() {
        let mesh = synth_mesh();
        let cam = Camera::default();
        let modes = [RenderMode::Shaded, RenderMode::Wire, RenderMode::Textured, RenderMode::Fullbright, RenderMode::Flat, RenderMode::UV];
        for mode in modes {
            for ortho in [false, true] {
                let p = RenderParams {
                    mode,
                    show_bones: true,
                    show_hitboxes: true,
                    show_attachments: true,
                    show_grid: true,
                    show_axes: true,
                    show_normals: true,
                    show_bounds: true,
                    skin_family: 0,
                    fov: 0.8,
                    ortho,
                    bg: [16, 20, 26],
                    w: 320,
                    h: 180,
                };
                let (w, h, px) = render_preview(&mesh, &cam, &p, None);
                assert_eq!(w, 320);
                assert_eq!(h, 180);
                assert_eq!(px.len(), 320 * 180 * 4);
            }
        }
        // 姿势覆盖不崩溃
        let p = RenderParams { mode: RenderMode::Shaded, show_bones: true, show_hitboxes: false, show_attachments: false, show_grid: false, show_axes: false, show_normals: false, show_bounds: false, skin_family: 0, fov: 0.8, ortho: false, bg: [0, 0, 0], w: 160, h: 90 };
        let pose = Some(vec![[0.0f32, 45.0, 0.0]]);
        let (_, _, px) = render_preview(&mesh, &cam, &p, pose.as_deref());
        assert_eq!(px.len(), 160 * 90 * 4);
    }

    #[test]
    fn skin_bind_identity() {
        // 绑定姿势（无覆盖）下蒙皮应还原原始顶点位置
        let mesh = synth_mesh();
        let sk = skin_mesh(&mesh, None);
        for i in 0..mesh.positions.len() {
            assert!((sk[i] - mesh.positions[i]).abs() < 1e-3, "bind skin mismatch at {i}");
        }
        // 有覆盖时长度一致且不崩
        let pose = vec![[0.0f32, 90.0, 0.0]];
        let sk2 = skin_mesh(&mesh, Some(&pose));
        assert_eq!(sk2.len(), mesh.positions.len());
    }
}
