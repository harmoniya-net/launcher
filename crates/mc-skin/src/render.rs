//! Software-rasterized Minecraft skin renderer.
//!
//! Builds the character out of axis-aligned boxes (head, body, arms, legs, hat
//! overlay, optional cape), rasterizes their faces with a depth buffer to an
//! [`RgbaImage`]. Pure: output depends only on (skin, cape, model, yaw, pitch,
//! t), so the result is easy to cache. UI/animation/drag concerns live in the
//! caller.

use image::{Rgba, RgbaImage};

/// Output image width in pixels (the rasterizer always renders at this size).
pub const WIDTH: u32 = 168;
/// Output image height in pixels.
pub const HEIGHT: u32 = 290;
/// Logical pixels per 1 model unit. Decoupled from `HEIGHT` so the buffer can
/// grow for rotation padding without zooming the character.
const SCALE: f32 = 7.5;
/// SSAA factor. We rasterize at `SS × WIDTH × SS × HEIGHT` and downsample with a
/// coverage-only filter: interior pixels copy directly (texels stay crisp),
/// silhouette pixels average their alpha (smooth outline). Higher = smoother
/// silhouette but cost grows quadratically; the extra smoothness is mostly
/// supplied cheaply by `smooth_alpha` below.
const SS: u32 = 2;

/// Arm width model. "Slim" (Alex) arms are 3px wide; "Classic" (Steve) 4px.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Model {
    Classic,
    Slim,
}

#[derive(Clone, Copy)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

fn rotate_y(p: Vec3, s: f32, c: f32) -> Vec3 {
    Vec3::new(c * p.x + s * p.z, p.y, -s * p.x + c * p.z)
}
fn rotate_x(p: Vec3, s: f32, c: f32) -> Vec3 {
    Vec3::new(p.x, c * p.y - s * p.z, s * p.y + c * p.z)
}
fn rotate_z(p: Vec3, s: f32, c: f32) -> Vec3 {
    Vec3::new(c * p.x - s * p.y, s * p.x + c * p.y, p.z)
}

/// One axis-aligned box of the character.
/// `origin` is the box center in model pixels; size + texture origin describe
/// the source rect on the skin.
struct Part {
    origin: Vec3,
    w: f32,
    h: f32,
    d: f32,
    /// Top-left of the texture region for this part, in skin pixel coords.
    u: u32,
    v: u32,
    /// How much to expand the geometry outward per face (0.0 for base layer,
    /// 0.5 for overlays like the hat).
    expand: f32,
    /// Local rotation in radians applied to vertices around the part pivot:
    /// `(pitch, yaw, roll)` — roll around Z, then pitch around X, then yaw
    /// around Y. (Z first so a rolled arm still pitches naturally.)
    rotate: (f32, f32, f32),
    /// Offset of the rotation pivot relative to the geometric centre, in
    /// model units. e.g. `(0, hy, 0)` pivots from the top of the box (used by
    /// the cape so it swings from the neck, not its middle).
    pivot: Vec3,
}

/// Build the part list for a given arm model.
/// Slim ("Alex"): arms are 3 wide; their inner edge still rests at x=±4
/// so the body silhouette stays consistent.
/// `legacy` skins (64×32) lack dedicated left arm/leg regions, so the left
/// limbs reuse the right ones — visually OK for symmetric skins.
fn parts_for(model: Model, legacy: bool, t: f32) -> Vec<Part> {
    let (arm_w, arm_cx) = match model {
        Model::Slim => (3.0_f32, 5.5_f32),
        Model::Classic => (4.0_f32, 6.0_f32),
    };
    let (l_arm_u, l_arm_v) = if legacy { (40, 16) } else { (32, 48) };
    let (l_leg_u, l_leg_v) = if legacy { (0, 16) } else { (16, 48) };
    // Idle motion: arms swing outward only (never into the body). sin² is
    // smooth, always non-negative, and naturally eases at both ends — the
    // arms rest at the body, drift out, then drift back.
    let arm_sway = (t * 0.45).sin().powi(2) * 0.07; // 0..~4°
    let shoulder_pivot = Vec3::new(0., 6., 0.); // top of arm box (shoulder)
    vec![
        // body
        Part {
            origin: Vec3::new(0., 18., 0.),
            w: 8.,
            h: 12.,
            d: 4.,
            u: 16,
            v: 16,
            expand: 0.0,
            rotate: (0.0, 0.0, 0.0),
            pivot: Vec3::new(0., 0., 0.),
        },
        // head
        Part {
            origin: Vec3::new(0., 28., 0.),
            w: 8.,
            h: 8.,
            d: 8.,
            u: 0,
            v: 0,
            expand: 0.0,
            rotate: (0.0, 0.0, 0.0),
            pivot: Vec3::new(0., 0., 0.),
        },
        // right arm (character's right = model -X). Negative roll swings
        // the hand outward (away from body, in -X direction).
        Part {
            origin: Vec3::new(-arm_cx, 18., 0.),
            w: arm_w,
            h: 12.,
            d: 4.,
            u: 40,
            v: 16,
            expand: 0.0,
            rotate: (0.0, 0.0, -arm_sway),
            pivot: shoulder_pivot,
        },
        // left arm — mirrored roll so both arms swing outward together.
        Part {
            origin: Vec3::new(arm_cx, 18., 0.),
            w: arm_w,
            h: 12.,
            d: 4.,
            u: l_arm_u,
            v: l_arm_v,
            expand: 0.0,
            rotate: (0.0, 0.0, arm_sway),
            pivot: shoulder_pivot,
        },
        // right leg
        Part {
            origin: Vec3::new(-2., 6., 0.),
            w: 4.,
            h: 12.,
            d: 4.,
            u: 0,
            v: 16,
            expand: 0.0,
            rotate: (0.0, 0.0, 0.0),
            pivot: Vec3::new(0., 0., 0.),
        },
        // left leg
        Part {
            origin: Vec3::new(2., 6., 0.),
            w: 4.,
            h: 12.,
            d: 4.,
            u: l_leg_u,
            v: l_leg_v,
            expand: 0.0,
            rotate: (0.0, 0.0, 0.0),
            pivot: Vec3::new(0., 0., 0.),
        },
        // hat (head overlay)
        Part {
            origin: Vec3::new(0., 28., 0.),
            w: 8.,
            h: 8.,
            d: 8.,
            u: 32,
            v: 0,
            expand: 0.5,
            rotate: (0.0, 0.0, 0.0),
            pivot: Vec3::new(0., 0., 0.),
        },
    ]
}

#[derive(Clone, Copy)]
struct Vertex {
    pos: Vec3,
    uv: (f32, f32),
}
struct Tri([Vertex; 3]);

fn part_triangles(p: &Part, tex_scale: f32) -> Vec<Tri> {
    let hx = p.w / 2.0 + p.expand;
    let hy = p.h / 2.0 + p.expand;
    let hz = p.d / 2.0 + p.expand;
    let c = p.origin;
    // Geometry stays in 1-unit-per-texel model space; UV coordinates scale by
    // the skin's actual resolution / 64 so HD skins (128, 256, ...) map to the
    // correct texel rectangles.
    let (u, v) = (p.u as f32 * tex_scale, p.v as f32 * tex_scale);
    let (w, h, d) = (p.w * tex_scale, p.h * tex_scale, p.d * tex_scale);

    // Vertices in local space (centred on the part origin), rotated around
    // the part's pivot offset, then translated by the part's world origin.
    let (pitch, yaw, roll) = p.rotate;
    let (ps, pc) = pitch.sin_cos();
    let (ys, yc) = yaw.sin_cos();
    let (rs, rc) = roll.sin_cos();
    let piv = p.pivot;
    let place = |lx: f32, ly: f32, lz: f32| -> Vec3 {
        let r = Vec3::new(lx - piv.x, ly - piv.y, lz - piv.z);
        let r = rotate_z(r, rs, rc);
        let r = rotate_x(r, ps, pc);
        let r = rotate_y(r, ys, yc);
        Vec3::new(c.x + piv.x + r.x, c.y + piv.y + r.y, c.z + piv.z + r.z)
    };
    let v000 = place(-hx, -hy, -hz);
    let v100 = place(hx, -hy, -hz);
    let v010 = place(-hx, hy, -hz);
    let v110 = place(hx, hy, -hz);
    let v001 = place(-hx, -hy, hz);
    let v101 = place(hx, -hy, hz);
    let v011 = place(-hx, hy, hz);
    let v111 = place(hx, hy, hz);

    let mut out = Vec::with_capacity(12);

    // Bias UVs inward by a tiny epsilon — just enough to stop `floor()` from
    // spilling into the next texture region (a transparent reserved area on
    // overlay/slim layouts). A larger bias (e.g. 0.5) would map texel centres
    // to face corners, which compresses the edge texels to half the screen
    // area of the interior texels.
    const E: f32 = 0.001;
    let mut face =
        |p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, u0: f32, v0: f32, u1: f32, v1: f32, v_flip: bool| {
            let (lo_u, hi_u) = (u0.min(u1) + E, u0.max(u1) - E);
            let (lo_v, hi_v) = (v0.min(v1) + E, v0.max(v1) - E);
            // p0=BL of face, p1=BR, p2=TR, p3=TL.
            // Normal case: BL of face ↔ BL of texture region.
            // V-flipped: BL of face ↔ TL of texture region (bottom face per MC).
            let (uv0, uv1, uv2, uv3) = if v_flip {
                ((lo_u, lo_v), (hi_u, lo_v), (hi_u, hi_v), (lo_u, hi_v))
            } else {
                ((lo_u, hi_v), (hi_u, hi_v), (hi_u, lo_v), (lo_u, lo_v))
            };
            out.push(Tri([
                Vertex { pos: p0, uv: uv0 },
                Vertex { pos: p1, uv: uv1 },
                Vertex { pos: p2, uv: uv2 },
            ]));
            out.push(Tri([
                Vertex { pos: p0, uv: uv0 },
                Vertex { pos: p2, uv: uv2 },
                Vertex { pos: p3, uv: uv3 },
            ]));
        };

    // FRONT (+Z local)
    face(
        v001,
        v101,
        v111,
        v011,
        u + d,
        v + d,
        u + d + w,
        v + d + h,
        false,
    );
    // BACK (-Z local)
    face(
        v100,
        v000,
        v010,
        v110,
        u + 2.0 * d + w,
        v + d,
        u + 2.0 * d + w + w,
        v + d + h,
        false,
    );
    // LEFT (+X local): texture region (u+d+w, v+d) size (d, h)
    face(
        v101,
        v100,
        v110,
        v111,
        u + d + w,
        v + d,
        u + d + w + d,
        v + d + h,
        false,
    );
    // RIGHT (-X local): texture region (u, v+d) size (d, h)
    face(v000, v001, v011, v010, u, v + d, u + d, v + d + h, false);
    // TOP (+Y): texture region (u+d, v) size (w, d)
    face(v011, v111, v110, v010, u + d, v, u + d + w, v + d, false);
    // BOTTOM (-Y): texture region (u+d+w, v) size (w, d), V-flipped per MC convention
    face(
        v000,
        v100,
        v101,
        v001,
        u + d + w,
        v,
        u + d + w + w,
        v + d,
        true,
    );

    out
}

/// Cape geometry: 10×16×1 box mounted just behind the upper body.
/// Rotated 180° on Y so the cape sheet's front region (the exterior) ends up
/// on the face that points away from the body, plus a small forward pitch so
/// the bottom hangs slightly outward — the natural worn pose.
fn cape_part(t: f32) -> Part {
    // Subtle cape sway at a different cadence than the arms so they don't
    // look synchronised.
    let cape_sway = (t * 0.8 + 0.6).sin() * 0.06;
    Part {
        // Origin places the cape's pivot (top of geometry, at y = origin + hy)
        // right against the back of the upper body.
        origin: Vec3::new(0., 16., -2.2),
        w: 10.,
        h: 16.,
        d: 1.,
        u: 0,
        v: 0,
        expand: 0.0,
        rotate: (-0.15 + cape_sway, std::f32::consts::PI, 0.0),
        pivot: Vec3::new(0., 8., 0.),
    }
}

/// Rasterize the character to an `RgbaImage` of [`WIDTH`]×[`HEIGHT`]. `t` is the
/// idle-animation clock in seconds; `yaw`/`pitch` are the view rotation in
/// radians.
pub fn rasterize(
    skin: &RgbaImage,
    cape: Option<&RgbaImage>,
    model: Model,
    yaw: f32,
    pitch: f32,
    t: f32,
) -> RgbaImage {
    let bw = WIDTH * SS;
    let bh = HEIGHT * SS;
    let mut buf = RgbaImage::from_pixel(bw, bh, Rgba([0, 0, 0, 0]));
    let mut depth = vec![f32::INFINITY; (bw * bh) as usize];

    let (ys, yc) = yaw.sin_cos();
    let (ps, pc) = pitch.sin_cos();

    let scale = SCALE * SS as f32;
    let cx = bw as f32 * 0.5;
    let cy = bh as f32 * 0.5;
    let project = |p: Vec3| -> (f32, f32, f32) {
        let q = Vec3::new(p.x, p.y - 16.0, p.z);
        let q = rotate_y(q, ys, yc);
        let q = rotate_x(q, ps, pc);
        let sx = cx + q.x * scale;
        let sy = cy - q.y * scale;
        // Camera looks toward -Z; larger model z = closer. Negate so depth
        // ordering is "smaller = closer" for a simple `<` test.
        (sx, sy, -q.z)
    };

    let mut rasterize = |part: &Part, texture: &RgbaImage| {
        let tex_scale = texture.width() as f32 / 64.0;
        let sw = texture.width() as i32;
        let sh = texture.height() as i32;
        for Tri([a, b, c]) in part_triangles(part, tex_scale) {
            let (ax, ay, az) = project(a.pos);
            let (bx, by, bz) = project(b.pos);
            let (cx2, cy2, cz) = project(c.pos);

            // Backface cull. Front-facing triangles wind CCW in screen space
            // after the Y flip (since we negate Y in projection, the sign also
            // flips — net result is back-faces have a positive cross-product).
            let area = (bx - ax) * (cy2 - ay) - (by - ay) * (cx2 - ax);
            if area >= 0.0 {
                continue;
            }
            let inv_area = 1.0 / area;

            let min_x = ax.min(bx).min(cx2).floor().max(0.0) as i32;
            let min_y = ay.min(by).min(cy2).floor().max(0.0) as i32;
            let max_x = ax.max(bx).max(cx2).ceil().min(bw as f32 - 1.0) as i32;
            let max_y = ay.max(by).max(cy2).ceil().min(bh as f32 - 1.0) as i32;

            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    let px = x as f32 + 0.5;
                    let py = y as f32 + 0.5;
                    let w0 = ((bx - px) * (cy2 - py) - (by - py) * (cx2 - px)) * inv_area;
                    let w1 = ((cx2 - px) * (ay - py) - (cy2 - py) * (ax - px)) * inv_area;
                    let w2 = ((ax - px) * (by - py) - (ay - py) * (bx - px)) * inv_area;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let z = w0 * az + w1 * bz + w2 * cz;
                    let idx = (y as u32 * bw + x as u32) as usize;
                    if z >= depth[idx] {
                        continue;
                    }

                    let tu = w0 * a.uv.0 + w1 * b.uv.0 + w2 * c.uv.0;
                    let tv = w0 * a.uv.1 + w1 * b.uv.1 + w2 * c.uv.1;
                    let ix = (tu.floor() as i32).clamp(0, sw - 1) as u32;
                    let iy = (tv.floor() as i32).clamp(0, sh - 1) as u32;
                    let px_col = *texture.get_pixel(ix, iy);
                    if px_col[3] < 8 {
                        continue;
                    }

                    depth[idx] = z;
                    buf.put_pixel(x as u32, y as u32, px_col);
                }
            }
        }
    };

    let legacy = skin.height() * 2 == skin.width();
    for part in &parts_for(model, legacy, t) {
        rasterize(part, skin);
    }
    if let Some(cape_tex) = cape {
        rasterize(&cape_part(t), cape_tex);
    }

    let mut out = downsample_aa(&buf);
    smooth_alpha(&mut out);
    out
}

/// 3×3 box blur on the alpha channel. Cheap silhouette feathering without
/// re-rasterizing: opaque interior stays opaque (all 9 neighbours = 255),
/// but pixels next to the silhouette pick up a partial alpha that softens
/// the edge by ~1 pixel. Colours are untouched, so texel boundaries inside
/// the model don't blur.
fn smooth_alpha(buf: &mut RgbaImage) {
    let w = buf.width() as usize;
    let h = buf.height() as usize;
    let alphas: Vec<u8> = buf.pixels().map(|p| p[3]).collect();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let mut sum = 0u32;
            for dy in 0..3 {
                let row = (y + dy - 1) * w;
                sum += alphas[row + x - 1] as u32;
                sum += alphas[row + x] as u32;
                sum += alphas[row + x + 1] as u32;
            }
            let avg = (sum / 9) as u8;
            let mut p = *buf.get_pixel(x as u32, y as u32);
            p[3] = avg;
            buf.put_pixel(x as u32, y as u32, p);
        }
    }
}

/// Box-downsample from `SS×` resolution to native, but only blend the alpha
/// channel. Colour is taken from one source pixel so internal texel boundaries
/// stay crisp; only the silhouette gets anti-aliased.
fn downsample_aa(src: &RgbaImage) -> RgbaImage {
    let mut out = RgbaImage::from_pixel(WIDTH, HEIGHT, Rgba([0, 0, 0, 0]));
    let n = SS * SS;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let mut alpha_sum = 0u32;
            let mut color = [0u8; 3];
            let mut found = false;
            for dy in 0..SS {
                for dx in 0..SS {
                    let p = src.get_pixel(x * SS + dx, y * SS + dy);
                    alpha_sum += p[3] as u32;
                    if !found && p[3] > 0 {
                        color = [p[0], p[1], p[2]];
                        found = true;
                    }
                }
            }
            let alpha = (alpha_sum / n) as u8;
            out.put_pixel(x, y, Rgba([color[0], color[1], color[2], alpha]));
        }
    }
    out
}
