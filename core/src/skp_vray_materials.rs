//! Read the small bitmap subgraphs stored in V-Ray SketchUp material metadata.
//! External files are loaded only below local directories approved by the importer.

use crate::model::{Material, SurfaceMap, Texture};
use base64::Engine as _;
use image::{DynamicImage, GrayImage, RgbImage, RgbaImage};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Read as _};
use std::path::{Component, Path, PathBuf};

const MAX_NODES: usize = 256;
const MAX_CHAIN: usize = 12;
const MAX_ASSETS: usize = 40;
const MAX_MAP_SIDE: u32 = 1536;
const MAX_ASSET_BYTES: u64 = 180 << 20;
const MAX_RESIDENT_BYTES: usize = 24 << 20;
const IDENTITY_UV: [f64; 6] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

#[derive(Clone, Debug)]
struct Bitmap {
    file: String,
    embedded: Option<String>,
    invert: bool,
    normal: bool,
    flip_y: bool,
    strength: f32,
    transform: Option<[f64; 6]>,
    transfer: Option<Transfer>,
    color_adjustment: Option<RgbAdjustment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct RgbAdjustment {
    multiplier: [u32; 3],
    offset: [u32; 3],
}

impl RgbAdjustment {
    fn from_bitmap(node: &Value) -> Option<Self> {
        let scalar = |value: &str| {
            let value: f32 = value.parse().ok()?;
            (value.is_finite() && value.abs() <= 16.0).then_some(value)
        };
        let rgb = |key| params(node, key).and_then(|value| rgb(value, scalar));
        let multiplier = rgb("color_mult").unwrap_or([1.0; 3]);
        let offset = rgb("color_offset").unwrap_or([0.0; 3]);
        (multiplier != [1.0; 3] || offset != [0.0; 3]).then_some(Self {
            multiplier: multiplier.map(f32::to_bits),
            offset: offset.map(f32::to_bits),
        })
    }

    fn apply(self, channel: usize, value: f32) -> f32 {
        value * f32::from_bits(self.multiplier[channel]) + f32::from_bits(self.offset[channel])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Transfer {
    Linear,
    Gamma(u32),
    Srgb,
}

impl Transfer {
    fn from_bitmap(node: &Value) -> Option<Self> {
        match params(node, "transfer_function")? {
            "0" => Some(Self::Linear),
            "1" => {
                let gamma = params(node, "gamma").unwrap_or("1").parse::<f32>().ok()?;
                if !(0.05..=10.0).contains(&gamma) {
                    return None;
                }
                Some(if gamma == 1.0 { Self::Linear } else { Self::Gamma(gamma.to_bits()) })
            }
            "2" => Some(Self::Srgb),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Correction {
    from: Transfer,
    to_srgb: bool,
    adjust: Option<RgbAdjustment>,
}

impl Bitmap {
    fn correction(&self, to_srgb: bool) -> Option<Correction> {
        let from = self.transfer.unwrap_or(if to_srgb { Transfer::Srgb } else { Transfer::Linear });
        if self.color_adjustment.is_none() && matches!((from, to_srgb), (Transfer::Linear, false) | (Transfer::Srgb, true)) {
            return None;
        }
        Some(Correction { from, to_srgb, adjust: self.color_adjustment })
    }
}

#[derive(Debug, Default)]
pub(crate) struct VrayMaterial {
    diffuse: Option<Bitmap>,
    color: Option<[f32; 3]>,
    roughness: Option<f32>,
    metalness: Option<f32>,
    roughness_map: Option<Bitmap>,
    specular_tint: Option<[f32; 3]>,
    specular_map: Option<Bitmap>,
    fresnel_enabled: Option<bool>,
    fresnel_ior: Option<f32>,
    bump: Option<Bitmap>,
    cellular_bump: Option<(f32, f64)>,
}

struct Graph {
    nodes: BTreeMap<String, Value>,
}

fn params<'a>(node: &'a Value, key: &str) -> Option<&'a str> {
    node.get("params")?.get(key)?.as_str().filter(|v| !v.is_empty())
}

fn first<'a>(node: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| params(node, key))
}

fn unit(value: &str) -> Option<f32> {
    let value: f32 = value.parse().ok()?;
    value.is_finite().then(|| value.clamp(0.0, 1.0))
}

fn rgb(value: &str, scalar: impl Fn(&str) -> Option<f32>) -> Option<[f32; 3]> {
    let mut c = value.strip_prefix("Color(")?.strip_suffix(')')?.split(',').map(|v| scalar(v.trim()));
    Some([c.next()??, c.next()??, c.next()??])
}

fn color(value: &str) -> Option<[f32; 3]> {
    rgb(value, unit)
}

fn valid_ior(value: &str) -> Option<f32> {
    let ior = value.parse().ok()?;
    (0.01..=100.0).contains(&ior).then_some(ior)
}

fn class(node: &Value) -> &str {
    node.get("class").and_then(Value::as_str).unwrap_or_default()
}

fn uv_affine(value: &str) -> Option<[f64; 6]> {
    fn vector(value: &str) -> Option<[f64; 3]> {
        let mut scalars = value.trim().strip_prefix("Vector(")?.split(',');
        let mut result = [0.0_f64; 3];
        for slot in &mut result {
            *slot = scalars.next()?.trim().parse().ok()?;
        }
        (scalars.next().is_none() && result.iter().all(|v| v.is_finite() && v.abs() <= 1e6)).then_some(result)
    }

    let value = value.trim().strip_prefix("Transform(Matrix(")?.strip_suffix(')')?;
    let (columns, offset) = value.split_once(")),")?;
    let mut columns = columns.split("),");
    let x = vector(columns.next()?)?;
    let y = vector(columns.next()?)?;
    let z = vector(columns.next()?)?;
    let offset = vector(offset.trim().strip_suffix(')')?)?;
    // V-Ray serializes the columns. Only planar transforms fit normalized image UVs.
    if columns.next().is_some() || [x[2], y[2], z[0], z[1]].into_iter().any(|v| v.abs() > 1e-8) {
        return None;
    }
    Some([x[0], y[0], offset[0], x[1], y[1], offset[1]])
}

fn compose_uv([a, b, c, d, e, f]: [f64; 6], [g, h, i, j, k, l]: [f64; 6]) -> [f64; 6] {
    [a * g + b * j, a * h + b * k, a * i + b * l + c,
     d * g + e * j, d * h + e * k, d * i + e * l + f]
}

impl Graph {
    fn uv_transform(&self, name: &str, depth: usize) -> Option<[f64; 6]> {
        if depth >= MAX_CHAIN {
            return None;
        }
        let node = self.nodes.get(name)?;
        match class(node) {
            "UVWGenRandomizer" => self.uv_transform(params(node, "input")?, depth + 1),
            "UVWGenChannel" => {
                if !matches!(params(node, "uvw_channel"), None | Some("1" | "-1")) {
                    return None;
                }
                let transform = |key| match params(node, key) {
                    Some(value) => uv_affine(value),
                    None => Some(IDENTITY_UV),
                };
                let input = match params(node, "uvwgen") {
                    Some(input) => self.uv_transform(input, depth + 1)?,
                    None => IDENTITY_UV,
                };
                let initial = transform("uvw_transform")?;
                let final_uv = transform("tex_transform")?;
                // The native image sampler repeats the affine result; nonlinear placement modes are outside this mapping.
                Some(compose_uv(final_uv, compose_uv(initial, input)))
            }
            _ => None,
        }
    }

    fn bitmap(&self, start: &str) -> Option<Bitmap> {
        let mut name = start;
        let mut invert = false;
        let mut normal = false;
        let mut flip_y = false;
        let mut transform = None;
        let mut color_adjustment = None;
        for _ in 0..MAX_CHAIN {
            let node = self.nodes.get(name)?;
            match class(node) {
                "BitmapBuffer" => {
                    let file = params(node, "file")?;
                    return Some(Bitmap {
                        file: file.into(), embedded: None, invert, normal, flip_y, strength: 1.0,
                        transform, transfer: Transfer::from_bitmap(node), color_adjustment,
                    });
                }
                "TexBitmap" => {
                    invert ^= params(node, "invert") == Some("1");
                    color_adjustment = RgbAdjustment::from_bitmap(node);
                    transform = params(node, "uvwgen").and_then(|name| self.uv_transform(name, 0))
                        .filter(|matrix| matrix.iter().zip(IDENTITY_UV).any(|(a, b)| (a - b).abs() > 1e-8));
                    name = params(node, "bitmap")?;
                }
                "TexNormalMapFlip" => {
                    normal = true;
                    flip_y ^= params(node, "flip_green") == Some("1");
                    name = params(node, "texmap")?;
                }
                "ColorCorrection" | "TexColorCorrection" | "TexCombineFloat" | "TexOutput" => {
                    name = first(node, &["texture_map", "texture", "texmap", "input"])?;
                }
                _ => return None,
            }
        }
        None
    }

    fn material(&self) -> Option<VrayMaterial> {
        let main = self.nodes.values().find(|node| class(node) == "MtlSingleBRDF")?;
        let mut current = params(main, "brdf")?;
        let mut layers = Vec::new();
        let mut out = VrayMaterial::default();
        for _ in 0..MAX_CHAIN {
            let node = self.nodes.get(current)?;
            match class(node) {
                "MtlMaterialID" | "MtlRenderStats" => current = params(node, "base_mtl")?,
                "BRDFBump" => {
                    let strength = params(node, "bump_tex_mult_tex").and_then(unit).unwrap_or(1.0);
                    out.bump = first(node, &["bump_tex_color", "bump_tex"])
                        .and_then(|name| self.bitmap(name))
                        .map(|mut map| {
                            map.strength = strength;
                            map.normal |= params(node, "map_type") == Some("1");
                            map
                        });
                    current = params(node, "base_brdf")?;
                }
                "BRDFLayered" => {
                    let refs = params(node, "brdfs")?.strip_prefix("List(")?.strip_suffix(')')?;
                    layers.extend(refs.split(',').take(MAX_CHAIN).filter_map(|name| self.nodes.get(name.trim())));
                    break;
                }
                "BRDFVRayMtl" => {
                    layers.push(node);
                    break;
                }
                _ => return None,
            }
        }
        let base = *layers.first()?;
        let finish = layers.last().copied().unwrap_or(base);
        let diffuse = params(base, "diffuse");
        out.color = diffuse.and_then(color);
        out.diffuse = diffuse.and_then(|name| self.bitmap(name));
        out.metalness = params(finish, "metalness").and_then(unit);
        let is_rough = params(finish, "option_use_roughness") == Some("1");
        let reflection = params(finish, "reflect");
        out.specular_tint = reflection.and_then(color);
        out.specular_map = reflection.and_then(|name| self.bitmap(name));
        out.fresnel_enabled = match params(finish, "fresnel") {
            Some("0") if layers.len() == 1 => Some(false),
            Some("1") => Some(true),
            _ => None,
        };
        if out.fresnel_enabled == Some(true) {
            let key = if params(finish, "fresnel_ior_lock") != Some("0") { "refract_ior" } else { "fresnel_ior" };
            out.fresnel_ior = params(finish, key).and_then(valid_ior);
        } else if layers.len() > 1 {
            let opacity_ior = (|| {
                let mut opacity = self.nodes.get(params(finish, "opacity")?)?;
                if class(opacity) == "TexCombineFloat" {
                    opacity = self.nodes.get(params(opacity, "texture")?)?;
                }
                if class(opacity) != "TexFresnel" {
                    return None;
                }
                params(opacity, "fresnel_ior").and_then(valid_ior)
            })();
            if let Some(ior) = opacity_ior {
                // A layered coat can obtain its angle dependence from its opacity.
                out.fresnel_enabled = Some(true);
                out.fresnel_ior = Some(ior);
            }
        }
        if out.specular_tint != Some([0.0; 3]) {
            let gloss = params(finish, "reflect_glossiness");
            out.roughness = gloss.and_then(unit).map(|value| if is_rough { value } else { 1.0 - value });
            out.roughness_map = gloss.and_then(|name| self.bitmap(name)).map(|mut map| {
                map.invert ^= !is_rough;
                map
            });
        }
        if out.bump.is_none() {
            out.bump = params(base, "bump_map").and_then(|name| self.bitmap(name)).map(|mut map| {
                map.normal |= params(base, "bump_type") == Some("1");
                map.strength = params(base, "bump_amount").and_then(unit).unwrap_or(1.0);
                map
            });
            out.cellular_bump = (|| {
                let map = self.nodes.get(params(base, "bump_map")?)?;
                if class(map) != "TexCellular" {
                    return None;
                }
                let wavelength = params(map, "size")?.parse::<f64>().ok()?;
                if !wavelength.is_finite() || !(0.0001..=1.0).contains(&wavelength) {
                    return None;
                }
                let strength = params(base, "bump_amount").and_then(unit)?;
                Some((strength, wavelength))
            })();
        }
        Some(out)
    }
}

/// SketchUp keeps each plugin as a JSON CDATA value in the material document.
pub(crate) fn parse(xml: &str) -> Option<VrayMaterial> {
    if !xml.contains("VRayPlugins") {
        return None;
    }
    let nodes = xml.split("<![CDATA[").skip(1).take(MAX_NODES).filter_map(|chunk| {
        let json = chunk.split_once("]]>")?.0;
        let value: Value = serde_json::from_str(json).ok()?;
        let name = value.get("name")?.as_str()?.to_owned();
        Some((name, value))
    }).collect();
    Graph { nodes }.material()
}

impl VrayMaterial {
    pub(crate) fn load_embedded(&mut self, mut read: impl FnMut(&str) -> Option<String>) {
        for bitmap in [&mut self.roughness_map, &mut self.bump, &mut self.specular_map].into_iter().flatten() {
            let Some(leaf) = bitmap.file.rsplit(['/', '\\']).next().filter(|name| !name.is_empty()) else { continue };
            let Some((stem, ext)) = leaf.rsplit_once('.') else { continue };
            let names = match ext.to_ascii_lowercase().as_str() {
                "png" | "jpg" | "jpeg" => vec![leaf.to_owned()],
                "tx" | "tif" | "tiff" => vec![format!("{stem}.png"), format!("{stem}.jpg")],
                _ => continue,
            };
            bitmap.embedded = names.iter().find_map(|name| read(name));
        }
    }
}

#[derive(Default)]
pub(crate) struct AssetResolver {
    roots: Vec<PathBuf>,
    cache: HashMap<(PathBuf, Option<Correction>), Option<String>>,
    loaded_bytes: usize,
    cellular: Option<String>,
}

fn normalized(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase().trim_end_matches('/').to_owned()
}

fn local(path: &Path) -> bool {
    let text = path.to_string_lossy();
    !text.starts_with("//") && !text.starts_with("\\\\")
}

impl AssetResolver {
    pub(crate) fn new(source: Option<&Path>, allowed: &[String]) -> Self {
        let mut resolver = Self::default();
        let cosmos_base = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .map(PathBuf::from)
            .map(|root| root.join("Documents").join("Chaos Cosmos").join("Packages"));
        let configured = std::env::var_os("SKETCHCRAFT_VRAY_ASSET_ROOTS").unwrap_or_default();
        let roots = source.and_then(Path::parent).into_iter()
            .chain(cosmos_base.as_deref())
            .chain(allowed.iter().map(Path::new))
            .map(Path::to_path_buf)
            .chain(std::env::split_paths(&configured).filter(|path| !path.as_os_str().is_empty()))
            .filter(|path| local(path))
            .take(24);
        for root in roots.filter_map(|root| root.canonicalize().ok()).filter(|root| root.is_dir()) {
            if !resolver.roots.contains(&root) {
                resolver.roots.push(root);
            }
        }
        resolver
    }

    fn find(&self, reference: &str) -> Option<PathBuf> {
        let slashes = reference.replace('\\', "/");
        if slashes.starts_with("//") || slashes.contains("://") || slashes.len() > 4096 {
            return None;
        }
        let input = Path::new(&slashes);
        let leaf = input.file_name()?;
        if !matches!(Path::new(leaf).extension()?.to_str()?.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "tx" | "tif" | "tiff") {
            return None;
        }
        let normalized_ref = normalized(&slashes);
        let approved = |root: &Path, relative: &Path| {
            // Check canonical containment as well as lexical containment (junctions/symlinks).
            let path = root.join(relative).canonicalize().ok()?;
            (path.starts_with(root) && path.is_file()).then_some(path)
        };
        for root in &self.roots {
            // Don't probe an arbitrary absolute path supplied by the model.
            let display_root = normalized(root.to_string_lossy().trim_start_matches(r"\\?\"));
            let at_root = normalized_ref.strip_prefix(&display_root).and_then(|tail| tail.strip_prefix('/'));
            let cosmos = (display_root.ends_with("/chaos cosmos/packages"))
                .then(|| normalized_ref.find("/chaos cosmos/packages/"))
                .flatten()
                .map(|index| index + "/chaos cosmos/packages/".len());
            let relative = at_root.map(|tail| &slashes[slashes.len() - tail.len()..])
                .or_else(|| cosmos.map(|index| &slashes[index..]))
                .or_else(|| (!input.is_absolute()).then_some(slashes.as_str()))
                .map(Path::new)
                .filter(|path| path.components().all(|part| matches!(part, Component::Normal(_))));
            if let Some(path) = relative.and_then(|path| approved(root, path)) {
                return Some(path);
            }
        }
        self.roots.iter().find_map(|root| approved(root, Path::new(leaf)))
    }

    fn image(&mut self, map: &Bitmap, to_srgb: bool) -> Option<String> {
        let correction = map.correction(to_srgb);
        let key = (self.find(&map.file)?, correction);
        if let Some(data) = self.cache.get(&key) {
            return data.clone();
        }
        if self.cache.len() >= MAX_ASSETS || self.loaded_bytes >= MAX_RESIDENT_BYTES {
            return None;
        }
        let value = decode_asset(&key.0).map(|image| correct(image, correction)).and_then(encode)
            .filter(|data| self.loaded_bytes + data.len() <= MAX_RESIDENT_BYTES);
        self.loaded_bytes += value.as_ref().map_or(0, String::len);
        self.cache.insert(key, value.clone());
        value
    }
}

fn decode_asset(path: &Path) -> Option<DynamicImage> {
    let file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    if size == 0 || size > MAX_ASSET_BYTES {
        return None;
    }
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "tx" | "tif" | "tiff" => decode_tiff(file),
        _ if size <= (40 << 20) => {
            let mut bytes = Vec::new();
            file.take((40 << 20) + 1).read_to_end(&mut bytes).ok()?;
            crate::safety::decode_image(&bytes, None).ok()
        }
        _ => None,
    }
}

fn decode_tiff(file: std::fs::File) -> Option<DynamicImage> {
    use tiff::decoder::{Decoder, DecodingResult};
    use tiff::ColorType;
    let mut decoder = Decoder::new(std::io::BufReader::new(file)).ok()?;
    // Cosmos .tx is a pyramid of TIFF images; take the highest mip up to our cap.
    for _ in 0..16 {
        let (w, h) = decoder.dimensions().ok()?;
        if w.max(h) <= MAX_MAP_SIDE || !decoder.more_images() {
            if w.max(h) > 8192 || (w as u64) * (h as u64) > 20_000_000 {
                return None;
            }
            let channels = decoder.colortype().ok()?;
            let data = match decoder.read_image().ok()? {
                DecodingResult::U8(data) => data,
                DecodingResult::U16(data) => data.into_iter().map(|v| (v >> 8) as u8).collect(),
                _ => return None,
            };
            return match channels {
                ColorType::Gray(_) => GrayImage::from_raw(w, h, data).map(DynamicImage::ImageLuma8),
                ColorType::RGB(_) => RgbImage::from_raw(w, h, data).map(DynamicImage::ImageRgb8),
                ColorType::RGBA(_) => RgbaImage::from_raw(w, h, data).map(DynamicImage::ImageRgba8),
                ColorType::YCbCr(8) => {
                    let mut rgb = data;
                    let quantize = |value: f32| value.round().clamp(0.0, 255.0) as u8;
                    // The TIFF decoder leaves JPEG samples in their original color space.
                    for pixel in rgb.chunks_exact_mut(3) {
                        let y = f32::from(pixel[0]);
                        let cb = f32::from(pixel[1]) - 128.0;
                        let cr = f32::from(pixel[2]) - 128.0;
                        pixel[0] = quantize(y + 1.402 * cr);
                        pixel[1] = quantize(y - 0.344136 * cb - 0.714136 * cr);
                        pixel[2] = quantize(y + 1.772 * cb);
                    }
                    RgbImage::from_raw(w, h, rgb).map(DynamicImage::ImageRgb8)
                }
                _ => None,
            };
        }
        decoder.next_image().ok()?;
    }
    None
}

fn encode(img: DynamicImage) -> Option<String> {
    let img = if img.width().max(img.height()) > MAX_MAP_SIDE {
        img.thumbnail(MAX_MAP_SIDE, MAX_MAP_SIDE)
    } else {
        img
    };
    let mut bytes = Cursor::new(Vec::new());
    img.write_to(&mut bytes, image::ImageFormat::Png).ok()?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())))
}

fn srgb(v: f32) -> f32 {
    if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

fn correct(image: DynamicImage, correction: Option<Correction>) -> DynamicImage {
    let Some(Correction { from, to_srgb, adjust }) = correction else { return image; };
    let table: [[u8; 256]; 3] = std::array::from_fn(|channel| std::array::from_fn(|i| {
        let sample = i as f32 / 255.0;
        let linear = match from {
            Transfer::Linear => sample,
            Transfer::Gamma(bits) => sample.powf(f32::from_bits(bits).recip()),
            Transfer::Srgb if sample <= 0.04045 => sample / 12.92,
            Transfer::Srgb => ((sample + 0.055) / 1.055).powf(2.4),
        };
        let linear = adjust.map_or(linear, |rgb| rgb.apply(channel, linear));
        let value = if to_srgb { srgb(linear) } else { linear };
        (value * 255.0).round() as u8
    }));
    if image.color().has_alpha() {
        let mut rgba = image.into_rgba8();
        for pixel in rgba.pixels_mut() {
            for (index, channel) in pixel.0[..3].iter_mut().enumerate() {
                *channel = table[index][*channel as usize];
            }
        }
        DynamicImage::ImageRgba8(rgba)
    } else if table[0] == table[1] && table[1] == table[2] && matches!(image.color(), image::ColorType::L8 | image::ColorType::L16) {
        let mut gray = image.into_luma8();
        for pixel in gray.pixels_mut() {
            pixel[0] = table[0][pixel[0] as usize];
        }
        DynamicImage::ImageLuma8(gray)
    } else {
        let mut rgb = image.into_rgb8();
        for pixel in rgb.pixels_mut() {
            for (index, channel) in pixel.0.iter_mut().enumerate() {
                *channel = table[index][*channel as usize];
            }
        }
        DynamicImage::ImageRgb8(rgb)
    }
}

fn embedded_image(map: &Bitmap) -> Option<String> {
    let data = map.embedded.as_ref()?;
    let Some(correction) = map.correction(false) else { return Some(data.clone()); };
    let (_, base64) = data.split_once(',')?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(base64).ok()?;
    let image = crate::safety::decode_image(&bytes, None).ok()?;
    encode(correct(image, Some(correction)))
}

/// A small tileable height approximation of the original cellular edge pattern.
/// The measured cell spacing is carried separately in meters on the map.
fn cellular_height() -> Option<String> {
    const SIDE: u32 = 128;
    const CELLS: i32 = 8;
    let cell = |x: i32, y: i32| {
        let key = (x.rem_euclid(CELLS) + y.rem_euclid(CELLS) * CELLS) as u32;
        let n = key.wrapping_mul(0x85eb_ca6b).wrapping_add(0x79ac_1e35);
        let n = (n ^ (n >> 15)).wrapping_mul(0xc2b2_ae35);
        (0.2 + (n & 255) as f32 * (0.6 / 255.0), 0.2 + ((n >> 12) & 255) as f32 * (0.6 / 255.0))
    };
    let img = GrayImage::from_fn(SIDE, SIDE, |x, y| {
        let u = (x as f32 + 0.5) * CELLS as f32 / SIDE as f32;
        let v = (y as f32 + 0.5) * CELLS as f32 / SIDE as f32;
        let mut nearest = f32::MAX;
        let mut second = f32::MAX;
        for j in (v as i32 - 1)..=(v as i32 + 1) {
            for i in (u as i32 - 1)..=(u as i32 + 1) {
                let (dx, dy) = cell(i, j);
                let d = (u - i as f32 - dx).powi(2) + (v - j as f32 - dy).powi(2);
                if d < nearest {
                    second = nearest;
                    nearest = d;
                } else {
                    second = second.min(d);
                }
            }
        }
        let edge = (1.0 - (second.sqrt() - nearest.sqrt()) * 2.4).clamp(0.0, 1.0);
        image::Luma([(edge * edge * (3.0 - 2.0 * edge) * 128.0).round() as u8])
    });
    encode(DynamicImage::ImageLuma8(img))
}

pub(crate) fn apply(material: &mut Material, vray: &VrayMaterial, assets: &mut AssetResolver) {
    if let Some(rgb) = vray.color {
        material.color = rgb.map(srgb);
    }
    if let Some(map) = &vray.diffuse {
        if let Some(data) = assets.image(map, true) {
            if let Some(texture) = &mut material.texture {
                texture.data = data;
            } else {
                material.texture = Some(Texture { data, width: 1.0, height: 1.0, project: None, origin: None });
            }
        }
        if material.texture.is_some() {
            material.texture_transform = map.transform;
        }
    }
    material.roughness = vray.roughness.or(material.roughness);
    material.metalness = vray.metalness.or(material.metalness);
    material.specular_tint = vray.specular_tint;
    material.fresnel_enabled = vray.fresnel_enabled;
    material.fresnel_ior = vray.fresnel_ior;
    let tile = material.texture.as_ref().map(|tex| [tex.width, tex.height]);
    let mut load = |map: &Bitmap| Some(SurfaceMap {
        data: assets.image(map, false).or_else(|| embedded_image(map))?,
        strength: map.strength,
        invert: map.invert,
        normal: map.normal,
        flip_y: map.flip_y,
        tile,
        transform: map.transform,
    });
    material.roughness_map = vray.roughness_map.as_ref().and_then(&mut load);
    material.bump_map = vray.bump.as_ref().and_then(&mut load);
    material.specular_map = vray.specular_map.as_ref().and_then(&mut load);
    if material.bump_map.is_none() {
        if let Some((strength, size)) = vray.cellular_bump {
            if assets.cellular.is_none() {
                assets.cellular = cellular_height();
            }
            material.bump_map = assets.cellular.clone().map(|data| SurfaceMap {
                data, strength, invert: false, normal: false, flip_y: false, tile: Some([size * 8.0; 2]), transform: None,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bitmap_preserves_static_column_major_uv_transform() {
        let graph = Graph { nodes: BTreeMap::from([
            ("bitmap".into(), json!({"class": "TexBitmap", "params": {"bitmap": "file", "uvwgen": "random"}})),
            ("file".into(), json!({"class": "BitmapBuffer", "params": {"file": "wood.tx"}})),
            ("random".into(), json!({"class": "UVWGenRandomizer", "params": {"input": "channel"}})),
            ("channel".into(), json!({"class": "UVWGenChannel", "params": {
                "uvw_channel": "1",
                "uvw_transform": "Transform(Matrix(Vector(0, 1, 0), Vector(-1, 0, 0), Vector(0, 0, 1)), Vector(0.25, 0.5, 0))",
                "tex_transform": "Transform(Matrix(Vector(2, 0, 0), Vector(0, 3, 0), Vector(0, 0, 1)), Vector(0.75, -0.25, 0))"
            }})),
        ]) };
        assert_eq!(graph.bitmap("bitmap").unwrap().transform, Some([0.0, -2.0, 1.25, 3.0, 0.0, 1.25]));
    }

    #[test]
    fn bitmap_transfer_decodes_reciprocal_gamma_and_preserves_alpha() {
        let pixels = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, image::Rgba([64, 64, 64, 17])));
        let from = Transfer::Gamma(2.0_f32.to_bits());
        let data = correct(pixels.clone(), Some(Correction { from, to_srgb: false, adjust: None })).into_rgba8();
        let color = correct(pixels, Some(Correction { from, to_srgb: true, adjust: None })).into_rgba8();
        assert_eq!(data.get_pixel(0, 0).0, [128, 128, 128, 17]);
        assert_eq!(color.get_pixel(0, 0).0, [188, 188, 188, 17]);
    }
}
