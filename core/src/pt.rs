//! Photoreal renderer: a CPU path tracer.
//!
//! - triangles from the model (components included) in a BVH (binned SAH)
//! - materials: paint (diffuse + a GGX specular coat), metal, thin glass,
//!   lights; picked from material names, colors and textures from the model
//! - light: a soft sun, rectangular emitters and an analytic sky, over
//!   a ground plane at the model's foot
//! - paths with Russian roulette; all cores; then an edge-aware a-trous
//!   denoiser guided by albedo and normals, and ACES tone mapping
//!
//! Everything is f32 inside; geometry comes in meters.

use crate::model::{self, Model};
use crate::render::{self, CameraSpec, RenderOpts};
use glam::{DVec3, Vec3};
use image::{Rgb, RgbImage};
use std::collections::HashMap;

// ----- materials ------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Paint,
    Metal,
    Glass,
    Light,
}

#[derive(Clone, Debug)]
struct Mat {
    kind: Kind,
    color: Vec3,
    rough: f32,
    metalness: f32,
    specular_tint: Vec3,
    fresnel: f32,
    tex: Option<MappedTexture>,
    roughness: Option<MappedTexture>,
    bump: Option<MappedTexture>,
    specular_map: Option<MappedTexture>,
    contact_shadow: Option<model::ContactShadow>,
}

struct Surface {
    kind: Kind,
    color: Vec3,
    normal: Vec3,
    roughness: f32,
    metalness: f32,
    specular: Vec3,
    fresnel: f32,
    contact_shadow: Option<model::ContactShadow>,
}

#[derive(Clone, Copy, Debug)]
struct MappedTexture {
    index: usize,
    u: Vec3,
    v: Vec3,
    origin: Vec3,
    use_face_uv: bool,
    transform: Option<[f32; 6]>,
    strength: f32,
    invert: bool,
    normal: bool,
    flip_y: bool,
}

impl MappedTexture {
    fn uv(self, p: Vec3, face_uv: Option<FaceProjection>) -> (f32, f32) {
        let projected = face_uv.filter(|_| self.use_face_uv).and_then(|projection| projection.at(p));
        let (u, v) = projected.map_or_else(|| {
            let offset = p - self.origin;
            (offset.dot(self.u), offset.dot(self.v))
        }, |(u, v, q)| (u / q, v / q));
        if let Some([ux, uy, uo, vx, vy, vo]) = self.transform {
            return (ux * u + uy * v + uo, vx * u + vy * v + vo);
        }
        (u, v)
    }

    fn gradients(self, p: Vec3, face_uv: Option<FaceProjection>) -> (Vec3, Vec3) {
        let projection = face_uv.filter(|_| self.use_face_uv);
        let (gu, gv) = projection.and_then(|projection| {
            let (u, v, q) = projection.at(p)?;
            Some((
                (projection.u.truncate() - projection.q.truncate() * (u / q)) / q,
                (projection.v.truncate() - projection.q.truncate() * (v / q)) / q,
            ))
        }).unwrap_or((self.u, self.v));
        if let Some([ux, uy, _, vx, vy, _]) = self.transform {
            return (gu * ux + gv * uy, gu * vx + gv * vy);
        }
        (gu, gv)
    }
}

#[derive(Clone, Copy)]
struct FaceProjection {
    u: glam::Vec4,
    v: glam::Vec4,
    q: glam::Vec4,
}

impl From<model::FaceUv> for FaceProjection {
    fn from(uv: model::FaceUv) -> Self {
        let to_f32 = |row: [f64; 4]| glam::Vec4::from_array(row.map(|v| v as f32));
        Self { u: to_f32(uv.u), v: to_f32(uv.v), q: to_f32(uv.q) }
    }
}

impl FaceProjection {
    fn at(self, point: Vec3) -> Option<(f32, f32, f32)> {
        let p = point.extend(1.0);
        let (u, v, q) = (self.u.dot(p), self.v.dot(p), self.q.dot(p));
        (u.is_finite() && v.is_finite() && q.is_finite() && q.abs() > 1e-8).then_some((u, v, q))
    }
}

struct Texture {
    w: usize,
    h: usize,
    px: Vec<Vec3>, // linear
}

impl Texture {
    fn sample(&self, u: f32, v: f32) -> Vec3 {
        let x = (u - u.floor()) * self.w as f32 - 0.5;
        let y = (1.0 - (v - v.floor())) * self.h as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (dx, dy) = (x - x0, y - y0);
        let x0 = (x0 as isize).rem_euclid(self.w as isize) as usize;
        let y0 = (y0 as isize).rem_euclid(self.h as isize) as usize;
        let (x1, y1) = ((x0 + 1) % self.w, (y0 + 1) % self.h);
        let top = self.px[y0 * self.w + x0].lerp(self.px[y0 * self.w + x1], dx);
        let bottom = self.px[y1 * self.w + x0].lerp(self.px[y1 * self.w + x1], dx);
        top.lerp(bottom, dy)
    }
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn kind_of(name: &str) -> Kind {
    let n = name.to_ascii_lowercase();
    let has = |w: &[&str]| w.iter().any(|x| n.contains(x));
    if has(&["glass", "window", "glazing", "windshield"]) {
        Kind::Glass
    } else if has(&["metal", "steel", "chrome", "alumin", "iron", "copper", "brass"]) {
        Kind::Metal
    } else if !n.starts_with("skp:") && has(&["light", "lamp", "glow", "emissive", "neon"]) {
        Kind::Light
    } else {
        Kind::Paint
    }
}

// ----- geometry ---------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Tri {
    a: Vec3,
    e1: Vec3,
    e2: Vec3,
    n: Vec3,
    mat: u32,
    uv: Option<u32>,
}

#[derive(Clone, Copy)]
struct Aabb {
    lo: Vec3,
    hi: Vec3,
}

impl Aabb {
    fn empty() -> Self {
        Aabb { lo: Vec3::splat(f32::MAX), hi: Vec3::splat(f32::MIN) }
    }
    fn grow(&mut self, p: Vec3) {
        self.lo = self.lo.min(p);
        self.hi = self.hi.max(p);
    }
    fn join(&mut self, b: &Aabb) {
        self.lo = self.lo.min(b.lo);
        self.hi = self.hi.max(b.hi);
    }
    fn area(&self) -> f32 {
        let d = (self.hi - self.lo).max(Vec3::ZERO);
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }
    /// Entry distance of a ray, if it hits before `tmax`.
    #[inline]
    fn hit(&self, o: Vec3, inv: Vec3, tmax: f32) -> Option<f32> {
        let t0 = (self.lo - o) * inv;
        let t1 = (self.hi - o) * inv;
        let tn = t0.min(t1).max_element();
        let tf = t0.max(t1).min_element();
        (tf >= tn.max(0.0) && tn < tmax).then_some(tn)
    }
}

struct Node {
    b: Aabb,
    /// leaf: first triangle and count; inner: left child index (right = left + 1), count 0
    start: u32,
    count: u32,
}

struct Bvh {
    nodes: Vec<Node>,
    tris: Vec<Tri>,
}

impl Bvh {
    fn build(mut tris: Vec<Tri>) -> Bvh {
        let n = tris.len();
        let bounds = |t: &Tri| {
            let mut b = Aabb::empty();
            b.grow(t.a);
            b.grow(t.a + t.e1);
            b.grow(t.a + t.e2);
            b
        };
        let mut boxes: Vec<Aabb> = tris.iter().map(bounds).collect();
        let mut cent: Vec<Vec3> = boxes.iter().map(|b| (b.lo + b.hi) * 0.5).collect();
        let mut nodes = vec![Node { b: Aabb::empty(), start: 0, count: n as u32 }];
        let mut stack = vec![0usize];
        while let Some(ni) = stack.pop() {
            let (s, c) = (nodes[ni].start as usize, nodes[ni].count as usize);
            let mut b = Aabb::empty();
            let mut cb = Aabb::empty();
            for i in s..s + c {
                b.join(&boxes[i]);
                cb.grow(cent[i]);
            }
            nodes[ni].b = b;
            if c <= 4 {
                continue;
            }
            // binned SAH on the widest centroid axis
            let ext = cb.hi - cb.lo;
            let axis = if ext.x >= ext.y && ext.x >= ext.z { 0 } else if ext.y >= ext.z { 1 } else { 2 };
            if ext[axis] < 1e-7 {
                continue;
            }
            const BINS: usize = 12;
            let mut bb = [Aabb::empty(); BINS];
            let mut bc = [0usize; BINS];
            let bin = |p: Vec3| (((p[axis] - cb.lo[axis]) / ext[axis] * BINS as f32) as usize).min(BINS - 1);
            for i in s..s + c {
                let k = bin(cent[i]);
                bb[k].join(&boxes[i]);
                bc[k] += 1;
            }
            let (mut best, mut best_cost) = (0, f32::MAX);
            for split in 1..BINS {
                let (mut l, mut r) = (Aabb::empty(), Aabb::empty());
                let (mut lc, mut rc) = (0, 0);
                for k in 0..split {
                    if bc[k] > 0 {
                        l.join(&bb[k]);
                        lc += bc[k];
                    }
                }
                for k in split..BINS {
                    if bc[k] > 0 {
                        r.join(&bb[k]);
                        rc += bc[k];
                    }
                }
                if lc == 0 || rc == 0 {
                    continue;
                }
                let cost = l.area() * lc as f32 + r.area() * rc as f32;
                if cost < best_cost {
                    best_cost = cost;
                    best = split;
                }
            }
            if best == 0 || best_cost >= b.area() * c as f32 {
                continue; // splitting doesn't pay: leaf
            }
            // partition in place
            let (mut i, mut j) = (s, s + c);
            while i < j {
                if bin(cent[i]) < best {
                    i += 1;
                } else {
                    j -= 1;
                    tris.swap(i, j);
                    boxes.swap(i, j);
                    cent.swap(i, j);
                }
            }
            let left = nodes.len();
            nodes.push(Node { b: Aabb::empty(), start: s as u32, count: (i - s) as u32 });
            nodes.push(Node { b: Aabb::empty(), start: i as u32, count: (s + c - i) as u32 });
            nodes[ni].start = left as u32;
            nodes[ni].count = 0;
            stack.push(left);
            stack.push(left + 1);
        }
        Bvh { nodes, tris }
    }

    /// Nearest hit: (t, triangle index, u, v).
    fn hit(&self, o: Vec3, d: Vec3, tmax: f32) -> Option<(f32, usize)> {
        if self.tris.is_empty() {
            return None;
        }
        let inv = Vec3::ONE / d;
        let mut best: Option<(f32, usize)> = None;
        let mut tbest = tmax;
        let mut stack = [0u32; 64];
        let mut sp = 1usize;
        while sp > 0 {
            sp -= 1;
            let node = &self.nodes[stack[sp] as usize];
            if node.b.hit(o, inv, tbest).is_none() {
                continue;
            }
            if node.count > 0 {
                for i in node.start..node.start + node.count {
                    let t = &self.tris[i as usize];
                    // Moller-Trumbore
                    let p = d.cross(t.e2);
                    let det = t.e1.dot(p);
                    if det.abs() < 1e-12 {
                        continue;
                    }
                    let inv_det = 1.0 / det;
                    let s = o - t.a;
                    let u = s.dot(p) * inv_det;
                    if !(0.0..=1.0).contains(&u) {
                        continue;
                    }
                    let q = s.cross(t.e1);
                    let v = d.dot(q) * inv_det;
                    if v < 0.0 || u + v > 1.0 {
                        continue;
                    }
                    let tt = t.e2.dot(q) * inv_det;
                    if tt > 1e-4 && tt < tbest {
                        tbest = tt;
                        best = Some((tt, i as usize));
                    }
                }
            } else {
                let (l, r) = (node.start, node.start + 1);
                // nearer child last, so it's popped first
                let dl = self.nodes[l as usize].b.hit(o, inv, tbest);
                let dr = self.nodes[r as usize].b.hit(o, inv, tbest);
                match (dl, dr) {
                    (Some(a), Some(b)) => {
                        if sp + 2 > stack.len() {
                            continue;
                        }
                        if a < b {
                            stack[sp] = r;
                            stack[sp + 1] = l;
                        } else {
                            stack[sp] = l;
                            stack[sp + 1] = r;
                        }
                        sp += 2;
                    }
                    (Some(_), None) => {
                        stack[sp] = l;
                        sp += 1;
                    }
                    (None, Some(_)) => {
                        stack[sp] = r;
                        sp += 1;
                    }
                    _ => {}
                }
            }
        }
        best
    }
}

// ----- the scene ---------------------------------------------------------------------

const MAX_AREA_LIGHTS: usize = 8;

#[derive(Debug, serde::Deserialize)]
pub struct AreaLightSpec {
    pub(crate) position: [f64; 3],
    pub(crate) target: [f64; 3],
    #[serde(default)]
    pub(crate) u_axis: Option<[f64; 3]>,
    #[serde(default)]
    pub(crate) uniform_emission: bool,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) intensity: f32,
    pub(crate) color: [f32; 3],
    #[serde(default)]
    pub(crate) directionality: f32,
    #[serde(default = "casts_shadows")]
    pub(crate) shadows: bool,
}

fn casts_shadows() -> bool { true }

/// A one-sided rectangular emitter. Its width runs parallel to the XY plane.
#[derive(Clone, Debug, PartialEq)]
pub struct AreaLight {
    position: Vec3,
    normal: Vec3,
    width_axis: Vec3,
    height_axis: Vec3,
    area: f32,
    emission: Vec3,
    uniform_emission: bool,
    directionality: f32,
    shadows: bool,
}

impl AreaLight {
    fn from_spec(spec: AreaLightSpec) -> anyhow::Result<Self> {
        use anyhow::{bail, Context};
        let position = DVec3::from(spec.position);
        let target = DVec3::from(spec.target);
        if !model::sane3(position) || !model::sane3(target) {
            bail!("position and target must be valid model coordinates");
        }
        let normal = (target - position).try_normalize().context("position and target must differ")?.as_vec3();
        if ![spec.width, spec.height].into_iter().all(|v| model::sane(v) && v >= 1e-3) {
            bail!("width and height must be at least 0.001 m and within the model's size limit");
        }
        if !spec.intensity.is_finite() || !(0.0..=10_000.0).contains(&spec.intensity) {
            bail!("intensity must be between 0 and 10000");
        }
        if !spec.color.iter().all(|v| (0.0..=1.0).contains(v)) {
            bail!("color must contain three sRGB values between 0 and 1");
        }
        if !(0.0..=1.0).contains(&spec.directionality) {
            bail!("directionality must be between 0 and 1");
        }
        let width_dir = if let Some(axis) = spec.u_axis {
            let axis = DVec3::from(axis).try_normalize().context("invalid rectangle width axis")?.as_vec3();
            (axis - normal * axis.dot(normal)).try_normalize().context("width axis cannot be parallel to the light direction")?
        } else {
            normal.cross(Vec3::Z).try_normalize().unwrap_or(Vec3::X)
        };
        let height_dir = width_dir.cross(normal);
        let width = spec.width as f32;
        let height = spec.height as f32;
        let emission = linear(spec.color) * spec.intensity;
        Ok(Self {
            position: position.as_vec3(),
            normal,
            width_axis: width_dir * width,
            height_axis: height_dir * height,
            area: width * height,
            emission,
            uniform_emission: spec.uniform_emission,
            directionality: spec.directionality,
            shadows: spec.shadows,
        })
    }
}

pub fn area_lights(specs: Vec<AreaLightSpec>) -> anyhow::Result<Vec<AreaLight>> {
    if specs.len() > MAX_AREA_LIGHTS {
        anyhow::bail!("use at most {MAX_AREA_LIGHTS} area lights");
    }
    let mut lights = Vec::with_capacity(specs.len());
    for (i, spec) in specs.into_iter().enumerate() {
        let light = AreaLight::from_spec(spec).map_err(|e| anyhow::anyhow!("area light {}: {e}", i + 1))?;
        if light.emission != Vec3::ZERO {
            lights.push(light);
        }
    }
    Ok(lights)
}

#[derive(Clone)]
pub struct PtOpts {
    pub width: u32,
    pub height: u32,
    pub samples: u32,
    pub bounces: u32,
    pub camera: CameraSpec,
    pub camera_up: Option<[f64; 3]>,
    pub ortho_height: Option<f64>,
    /// direction toward the sun (unit), and how bright
    pub sun: DVec3,
    pub sun_strength: f32,
    /// Angular radius for the direct-light shadow rays, in degrees.
    pub shadow_angle: Option<f32>,
    pub area_lights: Vec<AreaLight>,
    pub denoise: bool,
    pub exposure: f32,
    /// Optional imported [dark multiplier, highlight burn].
    pub reinhard: Option<[f32; 2]>,
    /// The linear RGB illuminant that the camera should render as neutral.
    pub white_balance: Option<[f32; 3]>,
    /// how bright the sky's light is (1 = normal)
    pub sky_strength: f32,
    /// Optional uniform environment radiance in linear RGB.
    pub sky_color: Option<[f32; 3]>,
    /// a plain backdrop color (sRGB 0..1) where the camera sees sky; the sky
    /// still lights the model
    pub background: Option<[f32; 3]>,
    /// the ground plane under the model (off: the model floats)
    pub ground: bool,
    /// the ground's color (sRGB 0..1)
    pub ground_color: Option<[f32; 3]>,
    pub seamless_ground: bool,
    /// the sun's color: -1 cool (overcast blue) .. 0 daylight .. 1 warm (golden hour)
    pub warmth: f32,
}

impl Default for PtOpts {
    fn default() -> Self {
        PtOpts {
            width: 960,
            height: 600,
            samples: 32,
            bounces: 6,
            camera: CameraSpec { eye: [10.0, -10.0, 8.0], target: [0.0, 0.0, 0.0], fov: 40.0 },
            camera_up: None,
            ortho_height: None,
            sun: DVec3::new(0.4, -0.6, 0.7),
            sun_strength: 2.4,
            shadow_angle: None,
            area_lights: Vec::new(),
            denoise: true,
            exposure: 1.0,
            reinhard: None,
            white_balance: None,
            sky_strength: 1.0,
            sky_color: None,
            background: None,
            ground: true,
            ground_color: None,
            seamless_ground: false,
            warmth: 0.0,
        }
    }
}

fn linear(c: [f32; 3]) -> Vec3 {
    Vec3::new(c[0].clamp(0.0, 1.0).powf(2.2), c[1].clamp(0.0, 1.0).powf(2.2), c[2].clamp(0.0, 1.0).powf(2.2))
}

struct Scene {
    /// a studio: dark surroundings, no sun; its lights do the work
    studio: bool,
    bvh: Bvh,
    mats: Vec<Mat>,
    textures: Vec<Texture>,
    face_projections: Vec<FaceProjection>,
    ground_z: f32,
    sun: Vec3,
    sun_strength: f32,
    shadow_radius: Option<f32>,
    sun_color: Vec3,
    area_lights: Vec<AreaLight>,
    sky_strength: f32,
    sky_color: Option<Vec3>,
    ground: bool,
    ground_color: Vec3,
    seamless_ground: bool,
}

const DEFAULT: [f32; 3] = [0.95, 0.94, 0.91];
const SUN_COLOR: Vec3 = Vec3::new(1.0, 0.94, 0.86);
/// cosine of the sun's half-angle (a bit bigger than the real sun: softer shadows)
const SUN_COS: f32 = 0.99995;

fn load_texture(data: &str, color: bool, label: &str) -> Option<Texture> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.split_once(',')?.1).ok()?;
    let img = match crate::safety::decode_image(&bytes, None) {
        Ok(img) => img,
        Err(error) => {
            crate::safety::note_skipped(label, &format!("{error}; image map was ignored"));
            return None;
        }
    };
    // A texture occupies three floats per pixel after decoding.
    let img = if img.width().max(img.height()) > 2048 {
        img.thumbnail(2048, 2048)
    } else {
        img
    }.to_rgb8();
    let px = img.pixels().map(|pixel| {
        let channels = pixel.0.map(|c| {
            let unit = c as f32 / 255.0;
            if color { srgb_to_linear(unit) } else { unit }
        });
        Vec3::from_array(channels)
    }).collect();
    Some(Texture { w: img.width() as usize, h: img.height() as usize, px })
}

fn build_scene(m: &Model, o: &PtOpts) -> Scene {
    let mut mats: Vec<Mat> = Vec::new();
    let mut textures: Vec<Texture> = Vec::new();
    let mut by_key: HashMap<String, u32> = HashMap::new();
    let mut tex_by_name: HashMap<(String, u8), Option<usize>> = HashMap::new();
    let mut tris = Vec::new();
    let mut face_projections = Vec::new();
    let mut zmin = f32::MAX;
    for f in m.world_faces() {
        let name = f.material.clone().unwrap_or_default();
        let color = f.color.unwrap_or(DEFAULT);
        // an image laid on the face maps along the face: one entry per direction
        let mm = m.find_material(&name);
        let diffuse = mm.and_then(|material| material.texture.as_ref());
        let has_maps = mm.is_some_and(|material| material.roughness_map.is_some() || material.bump_map.is_some() || material.specular_map.is_some());
        let along_face = diffuse.map_or(has_maps, |t| t.project.is_none());
        let n = f.normal.normalize_or_zero();
        let key = if along_face { format!("{name}|{color:?}|{:.3},{:.3},{:.3}", n.x, n.y, n.z) } else { format!("{name}|{color:?}") };
        let mat = *by_key.entry(key).or_insert_with(|| {
            // asked for on the material, else guessed from its name
            let kind = match mm {
                Some(x) if x.glass == Some(true) => Kind::Glass,
                Some(x) if x.emission.is_some_and(|e| e > 0.0) => Kind::Light,
                Some(x) if x.metalness.is_some() => {
                    if x.metalness.unwrap_or(0.0) >= 0.5 { Kind::Metal } else { Kind::Paint }
                }
                _ => kind_of(&name),
            };
            let lin = Vec3::new(srgb_to_linear(color[0]), srgb_to_linear(color[1]), srgb_to_linear(color[2]));
            let (tu, tv, origin) = match diffuse.and_then(|t| t.project) {
                Some(dir) => {
                    let (u, v) = crate::geom::basis(-DVec3::from(dir).normalize());
                    let origin = diffuse.and_then(|t| t.origin).map(DVec3::from).unwrap_or_default();
                    (u, v, origin)
                }
                None => {
                    let (u, v) = crate::geom::basis(f.normal.normalize());
                    (u, v, DVec3::ZERO)
                }
            };
            let mut mapped = |data: &str, channel, tile: [f64; 2], use_face_uv| {
                let index = (*tex_by_name.entry((name.clone(), channel)).or_insert_with(|| {
                    let texture = load_texture(data, channel == 0, &name)?;
                    textures.push(texture);
                    Some(textures.len() - 1)
                }))?;
                Some(MappedTexture {
                    index,
                    u: tu.as_vec3() / tile[0].max(1e-6) as f32,
                    v: tv.as_vec3() / tile[1].max(1e-6) as f32,
                    origin: origin.as_vec3(),
                    use_face_uv,
                    transform: None,
                    strength: 1.0,
                    invert: false,
                    normal: false,
                    flip_y: false,
                })
            };
            let tile = diffuse.map_or([1.0, 1.0], |t| [t.width, t.height]);
            let native_uv = diffuse.is_none_or(|t| t.project.is_none());
            let tex = diffuse.and_then(|t| {
                let mut texture = mapped(&t.data, 0, tile, native_uv)?;
                texture.transform = mm.and_then(|m| m.texture_transform).map(|row| row.map(|v| v as f32));
                Some(texture)
            });
            let mut surface_map = |map: &model::SurfaceMap, channel| {
                let map_tile = map.tile.unwrap_or(tile);
                let mut texture = mapped(&map.data, channel, map_tile, native_uv && map_tile == tile)?;
                texture.transform = map.transform.map(|row| row.map(|v| v as f32));
                texture.strength = map.strength;
                texture.invert = map.invert;
                texture.normal = map.normal;
                texture.flip_y = map.flip_y;
                Some(texture)
            };
            let roughness = mm.and_then(|m| m.roughness_map.as_ref()).and_then(|map| surface_map(map, 1));
            let bump = mm.and_then(|m| m.bump_map.as_ref()).and_then(|map| surface_map(map, 2));
            let specular_map = mm.and_then(|m| m.specular_map.as_ref()).and_then(|map| surface_map(map, 3));
            let specular_tint = mm.and_then(|m| m.specular_tint).map_or(Vec3::ONE, Vec3::from_array);
            let fresnel = if mm.and_then(|m| m.fresnel_enabled) == Some(false) {
                1.0
            } else {
                mm.and_then(|m| m.fresnel_ior).map_or(0.04, |ior| ((ior - 1.0) / (ior + 1.0)).powi(2))
            };
            let rough = mm.and_then(|x| x.roughness).unwrap_or(match kind {
                Kind::Metal => 0.3,
                _ => 0.6,
            });
            let metalness = mm.and_then(|m| m.metalness)
                .unwrap_or(if kind == Kind::Metal { 1.0 } else { 0.0 }).clamp(0.0, 1.0);
            let contact_shadow = mm.and_then(|m| m.contact_shadow)
                .filter(|s| (0.002..=1.0).contains(&s.radius) && (0.0..=1.0).contains(&s.amount) && s.amount > 0.0);
            mats.push(Mat { kind, color: lin, rough, metalness, specular_tint, fresnel, tex, roughness, bump, specular_map, contact_shadow });
            (mats.len() - 1) as u32
        });
        let uv = f.uv.map(|projection| {
            let index = face_projections.len() as u32;
            face_projections.push(FaceProjection::from(projection));
            index
        });
        for t in crate::geom::triangulate(&f.outer, &f.holes, f.normal) {
            let (a, b, c) = (t[0].as_vec3(), t[1].as_vec3(), t[2].as_vec3());
            zmin = zmin.min(a.z).min(b.z).min(c.z);
            let n = (b - a).cross(c - a).normalize_or_zero();
            if n == Vec3::ZERO {
                continue;
            }
            tris.push(Tri { a, e1: b - a, e2: c - a, n, mat, uv });
        }
    }
    Scene {
        studio: m.environment.as_deref() == Some("studio"),
        bvh: Bvh::build(tris),
        mats,
        textures,
        face_projections,
        ground_z: if zmin.is_finite() { zmin } else { 0.0 },
        sun: o.sun.as_vec3().normalize(),
        sun_strength: o.sun_strength,
        shadow_radius: o.shadow_angle.map(|angle| angle.clamp(0.0, 20.0).to_radians().tan()),
        sun_color: {
            let w = o.warmth.clamp(-1.0, 1.0);
            if w >= 0.0 { SUN_COLOR.lerp(Vec3::new(1.0, 0.72, 0.45), w) } else { SUN_COLOR.lerp(Vec3::new(0.80, 0.88, 1.0), -w) }
        },
        area_lights: o.area_lights.clone(),
        sky_strength: o.sky_strength.max(0.0),
        sky_color: o.sky_color.map(Vec3::from_array),
        ground: o.ground,
        ground_color: o.ground_color.map(linear).unwrap_or(Vec3::new(0.30, 0.30, 0.27)),
        seamless_ground: o.seamless_ground && o.background.is_some(),
    }
}

// ----- sampling ----------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03)
    }
    #[inline]
    fn f(&mut self) -> f32 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        ((self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32) / (1u64 << 24) as f32
    }
}

/// Orthonormal frame around n.
fn frame(n: Vec3) -> (Vec3, Vec3) {
    let s = if n.z >= 0.0 { 1.0 } else { -1.0 };
    let a = -1.0 / (s + n.z);
    let b = n.x * n.y * a;
    (Vec3::new(1.0 + s * n.x * n.x * a, s * b, -s * n.x), Vec3::new(b, s + n.y * n.y * a, -n.y))
}

fn cosine_dir(n: Vec3, r: &mut Rng) -> Vec3 {
    let (u1, u2) = (r.f(), r.f());
    let rr = u1.sqrt();
    let phi = std::f32::consts::TAU * u2;
    let (t, b) = frame(n);
    (t * (rr * phi.cos()) + b * (rr * phi.sin()) + n * (1.0 - u1).max(0.0).sqrt()).normalize()
}

fn ggx_d(nh: f32, a: f32) -> f32 {
    let a2 = a * a;
    let d = nh * nh * (a2 - 1.0) + 1.0;
    a2 / (std::f32::consts::PI * d * d).max(1e-12)
}

fn smith_g1(nv: f32, a: f32) -> f32 {
    let a2 = a * a;
    2.0 * nv / (nv + (a2 + (1.0 - a2) * nv * nv).sqrt()).max(1e-12)
}

fn schlick(f0: Vec3, c: f32) -> Vec3 {
    f0 + (Vec3::ONE - f0) * (1.0 - c).clamp(0.0, 1.0).powi(5)
}

/// The sky's light from direction `d` (z up), not counting the sun disk.
fn sky(d: Vec3, sun: Vec3) -> Vec3 {
    let t = d.z.max(0.0);
    let horizon = Vec3::new(0.70, 0.78, 0.90);
    let zenith = Vec3::new(0.18, 0.34, 0.75);
    let base = horizon.lerp(zenith, t.sqrt());
    // brighter toward the sun
    let glow = d.dot(sun).max(0.0).powi(8) * 0.5;
    (base + Vec3::new(1.0, 0.9, 0.75) * glow) * 0.55
}

impl Scene {
    fn material_at(&self, tri: &Tri, p: Vec3, n: Vec3) -> Surface {
        let material = &self.mats[tri.mat as usize];
        let face_uv = tri.uv.map(|index| self.face_projections[index as usize]);
        let sample = |map: MappedTexture| {
            let (u, v) = map.uv(p, face_uv);
            self.textures[map.index].sample(u, v)
        };
        let color = material.tex.map_or(material.color, sample);
        let rough = material.roughness.map_or(material.rough, |map| {
            let value = sample(map).dot(Vec3::splat(1.0 / 3.0));
            let value = if map.invert { 1.0 - value } else { value };
            (material.rough + (value - material.rough) * map.strength).clamp(0.02, 1.0)
        });
        let shading_normal = material.bump.map_or(n, |map| {
            let (u, v) = map.gradients(p, face_uv);
            let u = u - n * u.dot(n);
            let v = v - n * v.dot(n);
            let direction = if map.invert { -1.0 } else { 1.0 };
            if map.normal {
                let Some(v_unit) = v.try_normalize() else { return n; };
                let Some(tangent) = (u - v_unit * u.dot(v_unit)).try_normalize() else { return n; };
                let Some(bitangent) = (v - tangent * v.dot(tangent)).try_normalize() else { return n; };
                let local = sample(map) * 2.0 - Vec3::ONE;
                let y = if map.flip_y { -local.y } else { local.y };
                let offset = (tangent * local.x + bitangent * y) * map.strength * direction;
                (n * local.z.max(0.1) + offset).normalize_or_zero()
            } else {
                let tex = &self.textures[map.index];
                let (uu, vv) = map.uv(p, face_uv);
                let (du, dv) = (1.0 / tex.w as f32, 1.0 / tex.h as f32);
                let height = |u, v| tex.sample(u, v).dot(Vec3::splat(1.0 / 3.0));
                let grad_u = (height(uu + du, vv) - height(uu - du, vv)) / (2.0 * du);
                let grad_v = (height(uu, vv + dv) - height(uu, vv - dv)) / (2.0 * dv);
                // Convert the normalized height signal to shallow surface relief.
                let offset = (u * grad_u + v * grad_v) * (0.001 * map.strength * direction);
                (n - offset.clamp_length_max(1.0)).normalize_or_zero()
            }
        });
        Surface {
            kind: material.kind,
            color,
            normal: shading_normal,
            roughness: rough,
            metalness: material.metalness,
            specular: material.specular_map.map_or(material.specular_tint, |map| {
                let value = sample(map);
                if map.invert { Vec3::ONE - value } else { value }
            }),
            fresnel: material.fresnel,
            contact_shadow: material.contact_shadow,
        }
    }

    /// Visibility along a segment. Glass lets light through, tinted.
    fn light_visibility(&self, p: Vec3, dir: Vec3, mut distance: f32) -> Vec3 {
        let mut o = p;
        let mut through = Vec3::ONE;
        for _ in 0..4 {
            match self.bvh.hit(o, dir, distance) {
                None => return through,
                Some((t, ti)) => {
                    if self.mats[self.bvh.tris[ti].mat as usize].kind != Kind::Glass {
                        return Vec3::ZERO;
                    }
                    through *= Vec3::splat(0.85).lerp(self.mats[self.bvh.tris[ti].mat as usize].color, 0.3);
                    let step = t + 1e-3;
                    distance -= step;
                    if distance <= 1e-4 {
                        return through;
                    }
                    o += dir * step;
                }
            }
        }
        through
    }

    /// One point on one of the emitters, weighted for uniform area sampling.
    fn sample_area_light(&self, p: Vec3, n: Vec3, geometric_n: Vec3, r: &mut Rng) -> Option<(Vec3, f32, Vec3)> {
        let count = self.area_lights.len();
        if count == 0 {
            return None;
        }
        let light = &self.area_lights[if count == 1 { 0 } else { (r.f() * count as f32) as usize }];
        let point = light.position + light.width_axis * (r.f() - 0.5) + light.height_axis * (r.f() - 0.5);
        let toward_light = point - p;
        let distance = toward_light.length();
        if distance <= 1e-3 {
            return None;
        }
        let wi = toward_light / distance;
        let incoming = n.dot(wi);
        let outgoing = light.normal.dot(-wi);
        if incoming <= 0.0 || geometric_n.dot(wi) <= 0.0 || outgoing <= 0.0 {
            return None;
        }
        let origin = p + geometric_n * 1e-3;
        let ray = point - origin;
        let ray_distance = ray.length();
        if ray_distance <= 1e-4 {
            return None;
        }
        let dir = ray / ray_distance;
        let visibility = if light.shadows {
            if self.ground && !self.studio && point.z < self.ground_z && origin.z > self.ground_z {
                return None;
            }
            self.light_visibility(origin, dir, ray_distance)
        } else {
            Vec3::ONE
        };
        if visibility == Vec3::ZERO {
            return None;
        }
        let falloff = if light.uniform_emission { 1.0 } else { outgoing };
        // Narrow the emission lobe while keeping its hemisphere integral fixed.
        let exponent = 6.0 * light.directionality / (1.0 - light.directionality).max(0.04);
        let base_exponent = if light.uniform_emission { 1.0 } else { 2.0 };
        let spread = outgoing.powf(exponent) * (1.0 + exponent / base_exponent);
        let irradiance = light.emission * (count as f32 * light.area * falloff * spread / (distance * distance));
        Some((wi, incoming, visibility * irradiance))
    }

    /// Probe just above the surface so geometry such as a jamb can darken
    /// its immediate surroundings. A directional probe follows a nearby light.
    fn contact_visibility(&self, p: Vec3, n: Vec3, shadow: model::ContactShadow, r: &mut Rng) -> f32 {
        let axis = shadow.direction.map(Vec3::from_array)
            .and_then(|d| (d - n * n.dot(d)).try_normalize());
        let (tangent, secondary, lift) = if let Some(u) = axis {
            let (sin, cos) = ((r.f() - 0.5) * 1.3).sin_cos();
            (u * cos + n.cross(u) * sin, None, 0.018 + 0.10 * r.f())
        } else {
            let (u, v) = frame(n);
            let (sin, cos) = (r.f() * std::f32::consts::TAU).sin_cos();
            let tangent = u * cos + v * sin;
            (tangent, Some(-tangent), 0.12 + 0.42 * r.f())
        };
        let origin = p + n * 0.001;
        let closest = std::iter::once(tangent).chain(secondary).filter_map(|tangent| {
            let dir = (tangent + n * lift).normalize();
            let (distance, index) = self.bvh.hit(origin, dir, shadow.radius)?;
            (self.mats[self.bvh.tris[index].mat as usize].kind != Kind::Glass).then_some(distance)
        }).reduce(f32::min);
        let Some(distance) = closest else { return 1.0 };
        let t = ((distance / shadow.radius - 0.24) / 0.76).clamp(0.0, 1.0);
        1.0 - shadow.amount * (1.0 - t * t * (3.0 - 2.0 * t))
    }

    /// One path: returns radiance, the first hit's albedo and normal, and
    /// how much of the backdrop is visible along the camera ray.
    fn trace(&self, mut o: Vec3, mut d: Vec3, bounces: u32, r: &mut Rng) -> (Vec3, Vec3, Vec3, f32) {
        let mut l = Vec3::ZERO;
        let mut beta = Vec3::ONE;
        let (mut first_albedo, mut first_n) = (Vec3::ZERO, Vec3::ZERO);
        let mut specular = true; // camera and mirror-like bounces see the sun disk
        let sun = self.sun;
        let sun_e = self.sun_color * self.sun_strength;
        let mut backdrop = 0.0;
        for depth in 0..bounces.max(1) {
            // the model, or the ground plane under it
            let hit = self.bvh.hit(o, d, 1e30);
            let tg = if d.z < -1e-6 && !self.studio && self.ground { (self.ground_z - o.z) / d.z } else { f32::MAX };
            let (t, geometric_n, surface) = match hit {
                Some((t, ti)) if t < tg => {
                    let tr = &self.bvh.tris[ti];
                    let p = o + d * t;
                    let kind = self.mats[tr.mat as usize].kind;
                    let geometric_n = if kind != Kind::Glass && tr.n.dot(d) > 0.0 { -tr.n } else { tr.n };
                    (t, geometric_n, self.material_at(tr, p, geometric_n))
                }
                _ if tg > 1e-4 && tg < f32::MAX => {
                    if depth == 0 && self.seamless_ground {
                        // Blend only the ground close to the horizon. The lit
                        // floor and shadows around the model remain visible.
                        let t = (-d.z / 0.20).clamp(0.0, 1.0);
                        backdrop = 1.0 - t * t * (3.0 - 2.0 * t);
                    }
                    (tg, Vec3::Z, Surface {
                        kind: Kind::Paint, color: self.ground_color, normal: Vec3::Z, roughness: 0.9,
                        metalness: 0.0, specular: Vec3::ONE, fresnel: 0.04, contact_shadow: None,
                    })
                }
                _ if self.studio => {
                    // a dim, neutral room around the set
                    l += beta * Vec3::splat(0.035);
                    if depth == 0 { backdrop = 1.0; }
                    break;
                }
                _ => {
                    if depth == 0 { backdrop = 1.0; }
                    let mut s = self.sky_color.unwrap_or_else(|| sky(d, sun)) * self.sky_strength;
                    if specular && d.dot(sun) > SUN_COS {
                        s += sun_e * 2000.0;
                    }
                    l += beta * s;
                    break;
                }
            };
            let Surface { kind, color: albedo, normal, roughness: rough, metalness, specular: specular_tint, fresnel, contact_shadow } = surface;
            let n = if kind == Kind::Glass || normal.dot(-d) <= 1e-4 { geometric_n } else { normal };
            let p = o + d * t;
            if depth == 0 {
                first_albedo = albedo;
                first_n = n;
                if let Some(shadow) = contact_shadow.filter(|_| matches!(kind, Kind::Paint | Kind::Metal)) {
                    beta *= self.contact_visibility(p, geometric_n, shadow, r);
                }
            }
            match kind {
                Kind::Light => {
                    l += beta * albedo * 6.0;
                    break;
                }
                Kind::Glass => {
                    // thin glass (two surfaces): reflect by Fresnel, else pass
                    // straight through, tinted and dimmed like real glazing
                    let c = n.dot(-d).abs();
                    let f = (schlick(Vec3::splat(0.04), c).x * 2.0).clamp(0.12, 1.0);
                    if r.f() < f {
                        let nn = if n.dot(d) > 0.0 { -n } else { n };
                        d = d - nn * 2.0 * d.dot(nn);
                    } else {
                        beta *= Vec3::splat(0.55).lerp(albedo * 0.7, 0.35);
                    }
                    o = p + d * 1e-3;
                    specular = true;
                    continue;
                }
                _ => {}
            }
            let a = (rough * rough).max(0.02);
            let f0 = Vec3::splat(fresnel).lerp(albedo, metalness);
            let diffuse = albedo * (1.0 - metalness);
            let wo = -d;
            let nwo = n.dot(wo).max(1e-4);
            let direct_brdf = |wi: Vec3, ci: f32| {
                let h = (wi + wo).normalize();
                let nh = n.dot(h).max(0.0);
                let fr = schlick(f0, h.dot(wo).max(0.0)) * specular_tint;
                let spec = fr * (ggx_d(nh, a) * smith_g1(nwo, a) * smith_g1(ci, a) / (4.0 * nwo * ci).max(1e-6));
                let kd = (Vec3::ONE - fr) * diffuse / std::f32::consts::PI;
                kd + spec
            };
            // direct sun (none in a studio)
            let cs = if self.studio || self.sun_strength == 0.0 || geometric_n.dot(sun) <= 0.0 {
                0.0
            } else {
                n.dot(sun)
            };
            if cs > 0.0 {
                // jitter within the sun disk for soft shadows
                let (tu, tv) = frame(sun);
                let (dx, dy) = if let Some(radius) = self.shadow_radius {
                    let distance = radius * r.f().sqrt();
                    let azimuth = std::f32::consts::TAU * r.f();
                    (distance * azimuth.cos(), distance * azimuth.sin())
                } else {
                    let radius = (1.0 - SUN_COS * SUN_COS).sqrt();
                    ((r.f() - 0.5) * 2.0 * radius, (r.f() - 0.5) * 2.0 * radius)
                };
                let jit = (sun + tu * dx + tv * dy).normalize();
                let vis = self.light_visibility(p + geometric_n * 1e-3, jit, 1e30);
                if vis != Vec3::ZERO {
                    l += beta * vis * sun_e * direct_brdf(sun, cs) * cs;
                }
            }
            if let Some((wi, ci, irradiance)) = self.sample_area_light(p, n, geometric_n, r) {
                l += beta * irradiance * direct_brdf(wi, ci) * ci;
            }
            // next direction: diffuse or specular lobe
            let p_spec = 0.25 + 0.75 * metalness;
            let wi = if r.f() < p_spec {
                // sample a GGX microfacet normal
                let (u1, u2) = (r.f(), r.f());
                let th = (a * (u1 / (1.0 - u1).max(1e-6)).sqrt()).atan();
                let ph = std::f32::consts::TAU * u2;
                let (tu, tv) = frame(n);
                let h = (tu * (th.sin() * ph.cos()) + tv * (th.sin() * ph.sin()) + n * th.cos()).normalize();
                d - h * 2.0 * d.dot(h)
            } else {
                cosine_dir(n, r)
            };
            let ci = n.dot(wi);
            if ci <= 0.0 || geometric_n.dot(wi) <= 0.0 {
                break;
            }
            let h = (wi + wo).normalize();
            let nh = n.dot(h).max(0.0);
            let hw = h.dot(wo).max(1e-6);
            let pdf_spec = ggx_d(nh, a) * nh / (4.0 * hw);
            let pdf = p_spec * pdf_spec + (1.0 - p_spec) * ci / std::f32::consts::PI;
            if pdf < 1e-8 {
                break;
            }
            let fr = schlick(f0, hw) * specular_tint;
            let spec = fr * (ggx_d(nh, a) * smith_g1(nwo, a) * smith_g1(ci, a) / (4.0 * nwo * ci).max(1e-6));
            let kd = (Vec3::ONE - fr) * diffuse / std::f32::consts::PI;
            beta *= (kd + spec) * ci / pdf;
            specular = rough < 0.15 && r.f() < p_spec;
            // Russian roulette
            if depth >= 3 {
                let q = beta.max_element().min(0.95);
                if r.f() > q {
                    break;
                }
                beta /= q;
            }
            o = p + geometric_n * 1e-3;
            d = wi;
        }
        (l, first_albedo, first_n, backdrop)
    }
}

// ----- image ------------------------------------------------------------------------------

fn aces(x: Vec3) -> Vec3 {
    let (a, b, c, d, e) = (2.51, 0.03, 2.43, 0.59, 0.14);
    ((x * (x * a + b)) / (x * (x * c + d) + e)).clamp(Vec3::ZERO, Vec3::ONE)
}

fn to_srgb(c: f32) -> u8 {
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// Edge-aware a-trous filter: smooths noise but keeps edges where albedo or
/// normals change. Works on demodulated light (color / albedo).
fn denoise(color: &[Vec3], albedo: &[Vec3], normal: &[Vec3], w: usize, h: usize) -> Vec<Vec3> {
    let eps = Vec3::splat(0.02);
    let mut light: Vec<Vec3> = color.iter().zip(albedo).map(|(c, a)| *c / (*a + eps)).collect();
    const K: [f32; 5] = [1.0 / 16.0, 1.0 / 4.0, 3.0 / 8.0, 1.0 / 4.0, 1.0 / 16.0];
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(16);
    let rows_per = h.div_ceil(threads).max(1);
    for pass in 0..5 {
        let step = 1usize << pass;
        let src = light.clone();
        let sigma_l = 4.0 / (1 << pass) as f32 + 0.5;
        let src = &src;
        // rows in parallel: each thread fills its own band of `light`
        std::thread::scope(|s| {
            for (band, out) in light.chunks_mut(rows_per * w).enumerate() {
                s.spawn(move || {
                    let y0 = band * rows_per;
                    for (r, row) in out.chunks_mut(w).enumerate() {
                        let y = y0 + r;
                        for x in 0..w {
                            let i = y * w + x;
                            let (c0, n0, a0) = (src[i], normal[i], albedo[i]);
                            let l0 = c0.dot(Vec3::new(0.2126, 0.7152, 0.0722));
                            let (mut sum, mut wsum) = (Vec3::ZERO, 0.0f32);
                            for (ky, wy) in K.iter().enumerate() {
                                let yy = y as isize + (ky as isize - 2) * step as isize;
                                if yy < 0 || yy >= h as isize {
                                    continue;
                                }
                                for (kx, wx) in K.iter().enumerate() {
                                    let xx = x as isize + (kx as isize - 2) * step as isize;
                                    if xx < 0 || xx >= w as isize {
                                        continue;
                                    }
                                    let j = yy as usize * w + xx as usize;
                                    let wn = n0.dot(normal[j]).max(0.0).powi(32);
                                    if wn < 1e-4 {
                                        continue;
                                    }
                                    let lj = src[j].dot(Vec3::new(0.2126, 0.7152, 0.0722));
                                    // albedo and brightness weights in one exp
                                    let wt = wy * wx * wn * (-(a0 - albedo[j]).length_squared() * 60.0 - (l0 - lj).abs() / sigma_l).exp();
                                    sum += src[j] * wt;
                                    wsum += wt;
                                }
                            }
                            row[x] = if wsum > 0.0 { sum / wsum } else { c0 };
                        }
                    }
                });
            }
        });
    }
    light.iter().zip(albedo).map(|(l, a)| *l * (*a + eps)).collect()
}

/// The most work one render may ask for: pixels x samples x bounces.
const BUDGET: u64 = 4_000_000_000;

impl PtOpts {
    /// The same options held to what one call may cost (about a minute and a half at
    /// most): size, samples and bounces are capped, and samples come down
    /// further if the three together are still too much.
    pub fn limited(&self) -> PtOpts {
        let mut o = self.clone();
        o.width = o.width.clamp(16, 4096);
        o.height = o.height.clamp(16, 4096);
        o.bounces = o.bounces.clamp(1, 12);
        o.area_lights.truncate(MAX_AREA_LIGHTS);
        o.samples = o.samples.clamp(1, 2048);
        let per_sample = (o.width as u64 * o.height as u64 * o.bounces as u64).max(1);
        o.samples = o.samples.min((BUDGET / per_sample).max(1) as u32);
        o
    }
}

/// Render a photoreal image of the model.
pub fn render(m: &Model, o: &PtOpts) -> RgbImage {
    let limited = o.limited();
    let o = &limited;
    let scene = build_scene(m, o);
    let (w, h) = (o.width.max(16) as usize, o.height.max(16) as usize);
    let (c, a, n, sky) = pass(&scene, o, w, h, 0, o.samples.max(1));
    finish(&c, &a, &n, &sky, o.samples.max(1), w, h, o)
}

/// A render in progress: the scene, and every sample so far summed per
/// pixel. Each `progressive` call adds more and returns the picture so far.
pub struct Progress {
    /// what it's a render of (camera, size, light, model revision)
    pub key: String,
    scene: Scene,
    w: usize,
    h: usize,
    color: Vec<Vec3>,
    albedo: Vec<Vec3>,
    normal: Vec<Vec3>,
    /// samples that saw the sky, per pixel (for a plain background)
    sky: Vec<f32>,
    /// samples per pixel so far
    pub spp: u32,
    passes: u32,
}

/// Add `o.samples` samples per pixel to `prev` (when it's of the same
/// thing, `key`), or start over. Returns the progress and the denoised
/// picture so far.
pub fn progressive(prev: Option<Progress>, key: String, m: &Model, o: &PtOpts) -> (Progress, RgbImage) {
    let limited = o.limited();
    let o = &limited;
    let (w, h) = (o.width.max(16) as usize, o.height.max(16) as usize);
    let mut p = match prev {
        Some(p) if p.key == key && p.w == w && p.h == h => p,
        _ => Progress { key, scene: build_scene(m, o), w, h, color: vec![Vec3::ZERO; w * h], albedo: vec![Vec3::ZERO; w * h], normal: vec![Vec3::ZERO; w * h], sky: vec![0.0; w * h], spp: 0, passes: 0 },
    };
    let add = o.samples.max(1);
    let (c, a, n, sk) = pass(&p.scene, o, w, h, p.passes + 1, add);
    for i in 0..w * h {
        p.color[i] += c[i];
        p.albedo[i] += a[i];
        p.normal[i] += n[i];
        p.sky[i] += sk[i];
    }
    p.spp += add;
    p.passes += 1;
    let img = finish(&p.color, &p.albedo, &p.normal, &p.sky, p.spp, w, h, o);
    (p, img)
}

/// Sums of `spp` samples per pixel (color, albedo, normal), on all cores.
/// `seed` makes each pass's samples different from the last.
fn pass(scene: &Scene, o: &PtOpts, w: usize, h: usize, seed: u32, spp: u32) -> (Vec<Vec3>, Vec<Vec3>, Vec<Vec3>, Vec<f32>) {
    let eye = DVec3::from(o.camera.eye).as_vec3();
    let target = DVec3::from(o.camera.target).as_vec3();
    let fwd = (target - eye).normalize();
    let up0 = o.camera_up
        .and_then(|up| DVec3::from(up).try_normalize())
        .map(|up| up.as_vec3())
        .filter(|up| fwd.cross(*up).length() >= 1e-3)
        .unwrap_or_else(|| if fwd.cross(Vec3::Z).length() < 1e-3 { Vec3::Y } else { Vec3::Z });
    let right = fwd.cross(up0).normalize();
    let up = right.cross(fwd);
    let tan = (o.camera.fov.to_radians() as f32 * 0.5).tan();
    let aspect = w as f32 / h as f32;
    let ortho_half = o.ortho_height.filter(|h| h.is_finite() && *h > 0.0).map(|h| h as f32 * 0.5);

    let mut color = vec![Vec3::ZERO; w * h];
    let mut albedo = vec![Vec3::ZERO; w * h];
    let mut normal = vec![Vec3::ZERO; w * h];
    let mut skyc = vec![0.0f32; w * h];
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(16);
    let rows: Vec<usize> = (0..h).collect();
    let chunk = h.div_ceil(threads * 4);
    std::thread::scope(|s| {
        let mut parts: Vec<(&mut [Vec3], &mut [Vec3], &mut [Vec3], &mut [f32], &[usize])> = Vec::new();
        let (mut c, mut a, mut n, mut sk, mut rr) = (&mut color[..], &mut albedo[..], &mut normal[..], &mut skyc[..], &rows[..]);
        while !rr.is_empty() {
            let k = chunk.min(rr.len());
            let (c1, c2) = c.split_at_mut(k * w);
            let (a1, a2) = a.split_at_mut(k * w);
            let (n1, n2) = n.split_at_mut(k * w);
            let (s1, s2) = sk.split_at_mut(k * w);
            let (r1, r2) = rr.split_at(k);
            parts.push((c1, a1, n1, s1, r1));
            c = c2;
            a = a2;
            n = n2;
            sk = s2;
            rr = r2;
        }
        // hand chunks out round-robin to the threads
        let mut buckets: Vec<Vec<(&mut [Vec3], &mut [Vec3], &mut [Vec3], &mut [f32], &[usize])>> = (0..threads).map(|_| Vec::new()).collect();
        for (i, p) in parts.into_iter().enumerate() {
            buckets[i % threads].push(p);
        }
        for bucket in buckets {
            s.spawn(move || {
                for (cs, als, ns, sks, ys) in bucket {
                    for (row, &y) in ys.iter().enumerate() {
                        for x in 0..w {
                            let mut r = Rng::new(((y * w + x) as u64) << 8 | 1 ^ (seed as u64).wrapping_mul(0x2545_F491_4F6C_DD1D));
                            let (mut acc, mut aa, mut an, mut miss) = (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 0.0f32);
                            for _ in 0..spp {
                                let px = (x as f32 + r.f()) / w as f32 * 2.0 - 1.0;
                                let py = 1.0 - (y as f32 + r.f()) / h as f32 * 2.0;
                                let (origin, direction) = if let Some(half) = ortho_half {
                                    (eye + right * (px * half * aspect) + up * (py * half), fwd)
                                } else {
                                    (eye, (fwd + right * (px * tan * aspect) + up * (py * tan)).normalize())
                                };
                                let (l, alb, nn, m) = scene.trace(origin, direction, o.bounces, &mut r);
                                miss += m;
                                // keep fireflies in check
                                let lum = l.max_element();
                                acc += if lum > 40.0 { l * (40.0 / lum) } else { l };
                                aa += alb;
                                an += nn;
                            }
                            let i = row * w + x;
                            cs[i] = acc;
                            als[i] = aa;
                            ns[i] = an;
                            sks[i] = miss;
                        }
                    }
                }
            });
        }
    });
    (color, albedo, normal, skyc)
}

/// Averages, denoise, tone map.
fn finish(color: &[Vec3], albedo: &[Vec3], normal: &[Vec3], sky: &[f32], spp: u32, w: usize, h: usize, o: &PtOpts) -> RgbImage {
    let k = 1.0 / spp.max(1) as f32;
    let color: Vec<Vec3> = color.iter().map(|c| *c * k).collect();
    let albedo: Vec<Vec3> = albedo.iter().map(|c| *c * k).collect();
    let normal: Vec<Vec3> = normal.iter().map(|c| c.normalize_or_zero()).collect();
    let final_color = if o.denoise { denoise(&color, &albedo, &normal, w, h) } else { color };
    let balance = o.white_balance.map_or(Vec3::ONE, |white| {
        let gains = Vec3::from_array(white).max(Vec3::splat(0.01)).recip();
        gains / gains.dot(Vec3::new(0.2126, 0.7152, 0.0722))
    });
    let mut img = RgbImage::new(w as u32, h as u32);
    for (i, p) in img.pixels_mut().enumerate() {
        let exposed = final_color[i] * (balance * o.exposure);
        let c = if let Some([dark, burn]) = o.reinhard {
            let x = exposed.max(Vec3::ZERO) * dark;
            x * ((Vec3::ONE + burn * x) / (Vec3::ONE + x))
        } else {
            aces(exposed)
        };
        let mut px = [to_srgb(c.x), to_srgb(c.y), to_srgb(c.z)];
        // a plain backdrop where the camera saw sky (soft at the model's edges)
        if let Some(bg) = o.background {
            let t = (sky[i] * k).clamp(0.0, 1.0);
            for j in 0..3 {
                px[j] = (px[j] as f32 * (1.0 - t) + bg[j].clamp(0.0, 1.0) * 255.0 * t).round() as u8;
            }
        }
        *p = Rgb(px);
    }
    img
}

/// The camera for a render request, as the regular renderer picks it.
pub fn camera(m: &Model, opts: &RenderOpts) -> CameraSpec {
    render::camera_for(m, opts)
}

/// Where the sun is (compass azimuth from north, clockwise; altitude above
/// the horizon; degrees) at a place and local clock time. NOAA's solar
/// position equations: good to a fraction of a degree for 1900-2100.
/// `utc_offset` is the local clock's hours ahead of UTC (-7 for US Pacific
/// summer time).
pub fn solar_position(lat: f64, lon: f64, year: i32, month: u32, day: u32, hour: f64, utc_offset: f64) -> (f64, f64) {
    // day of the year
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let before = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334][(month.clamp(1, 12) - 1) as usize] + if leap && month > 2 { 1 } else { 0 };
    let n = (before + day) as f64;
    let days_in_year = if leap { 366.0 } else { 365.0 };
    let g = 2.0 * std::f64::consts::PI / days_in_year * (n - 1.0 + (hour - 12.0) / 24.0);
    // equation of time (minutes) and declination (radians)
    let eqt = 229.18 * (0.000075 + 0.001868 * g.cos() - 0.032077 * g.sin() - 0.014615 * (2.0 * g).cos() - 0.040849 * (2.0 * g).sin());
    let decl = 0.006918 - 0.399912 * g.cos() + 0.070257 * g.sin() - 0.006758 * (2.0 * g).cos() + 0.000907 * (2.0 * g).sin() - 0.002697 * (3.0 * g).cos() + 0.00148 * (3.0 * g).sin();
    // true solar time (minutes) and hour angle
    let tst = hour * 60.0 + eqt + 4.0 * lon - 60.0 * utc_offset;
    let ha = (tst / 4.0 - 180.0).to_radians();
    let phi = lat.to_radians();
    let cos_zen = (phi.sin() * decl.sin() + phi.cos() * decl.cos() * ha.cos()).clamp(-1.0, 1.0);
    let zen = cos_zen.acos();
    let altitude = 90.0 - zen.to_degrees();
    // azimuth, clockwise from north
    let az = (decl.sin() - phi.sin() * cos_zen) / (phi.cos() * zen.sin()).max(1e-9);
    let mut azimuth = az.clamp(-1.0, 1.0).acos().to_degrees();
    if ha > 0.0 {
        azimuth = 360.0 - azimuth;
    }
    (azimuth, altitude)
}

/// Sun direction from compass azimuth (degrees from north, clockwise) and altitude.
pub fn sun_dir(azimuth: f64, altitude: f64) -> DVec3 {
    let (az, al) = (azimuth.to_radians(), altitude.to_radians());
    // +Y is north, +X east
    DVec3::new(az.sin() * al.cos(), az.cos() * al.cos(), al.sin()).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sketchup_material_names_do_not_imply_emission() {
        assert_eq!(kind_of("skp:Flooring Herringbone Light 750cm"), Kind::Paint);
        assert_eq!(kind_of("skp:Warm Light"), Kind::Paint);
        assert_eq!(kind_of("warm light"), Kind::Light);
        assert_eq!(kind_of("skp:Metal Sandblasted Black 200cm"), Kind::Metal);
    }

    #[test]
    fn material_maps_filter_across_the_repeat_seam() {
        let texture = Texture { w: 2, h: 1, px: vec![Vec3::ZERO, Vec3::ONE] };
        assert_eq!(texture.sample(0.25, 0.5), Vec3::ZERO);
        assert_eq!(texture.sample(0.75, 0.5), Vec3::ONE);
        assert_eq!(texture.sample(0.0, 0.5), Vec3::splat(0.5));
        assert_eq!(texture.sample(-0.25, 0.5), Vec3::ONE);
    }
}
