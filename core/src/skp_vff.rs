//! Modern SketchUp files (2013 and later, checked against SketchUp 26.2).
//!
//! Clean-room reader, worked out from sample files for interoperability.
//! After the usual UTF-16 "SketchUp Model" + version header comes a short
//! "VFF" marker and a ZIP archive holding:
//!
//! - `model.dat`: a tree of records, each `[tag u16][length u32][body]`. A
//!   body is either raw bytes or more records. Integers (ids, counts) are
//!   little-endian of whatever length the record gives.
//! - `materials/<name>/material.xml`: color, and the image file of textured
//!   materials.
//!
//! Inside `model.dat` (tag paths from the root record 0x1f4):
//!
//! ```text
//! 0x1f7 .. 0x32c8          material: 0x5dc/0x5de id, 0x32cc name
//! 0x1f9/0x1770/0x1771/0x157c   component definition
//!     0x157e               older GUID (text), or display name when 0x157d contains the binary GUID
//!     0x1388               its entities:
//!         0x1389/0x9c4     vertex: 0x5dc/0x5de id, 0x9c5 x,y,z (f64 inches)
//!         0x138a/0xbb8     edge: 0x7d0 header (0x5dc/0x5de id, 0x7d3 flags),
//!                          0xbb9 start vertex id, 0xbba end vertex id
//!         0x138b/0xdac     face: 0x7d0 header (id; 0x7d1 material id),
//!                          0xdad plane, 0xdae/0x1194 loops (first is outer),
//!                          each 0x1195/0xfa0 edge use: 0xfa1 edge id,
//!                          0xfa2 reversed
//!         0x138d/0x1d4c/0x1964  instance: 0x1965 definition GUID,
//!                          0x1966 13 f64 (x axis, y axis, z axis, origin, scale)
//!         0x1390/0x1f40/0x1964  a picture, placed like an instance
//!         0x138c/0x1964    instance (SketchUp 27): 0x1965 is empty and
//!                          0x1967 names the definition by the id of its
//!                          0x1388 entities record; 0x1968 is its own GUID
//! 0x1f6/0x1388             the model's own entities (same layout; 0x1391/0x4269
//!                          and 0x1397 hold guide points and curves, not drawn)
//! 0x207/0x6d60/0x6d61/0x7148   saved scene
//!     0x6f54/0x6f55            name; 0x7149 bit 0 saves the camera
//!     0x714a/0x34bc            camera: 0x34bd eye, 0x34be target, 0x34bf up,
//!                              0x34c2 perspective, 0x34c3 height, 0x34c4 FOV,
//!                              0x34c5 aspect, 0x34c6 measures FOV vertically
//! ```
//!
//! The front texture placement is in the face header's
//! 0x5dc/0x5dd/0x36b1/0x36b2/0x2710/0x2711/0x2713 record. Its 0x2715
//! matrix maps texture inches onto face-plane inches. Tile size normally
//! comes from material XML. Edge flag bit 0x10 marks soft edges.

use crate::model::{self, View};
use crate::skp::{Entities, SkpFace, SkpFile, SkpPlacement, INCH};
use anyhow::{bail, Context, Result};
use glam::{DMat3, DMat4, DVec3, DVec4};
use std::collections::HashMap;
use std::io::{Cursor, Read};

#[derive(Clone, Copy)]
pub(crate) struct Node<'a> {
    pub(crate) d: &'a [u8],
    tag: u16,
}

impl<'a> Node<'a> {
    /// The records inside this one, or none if its body isn't records.
    pub(crate) fn kids(&self) -> Vec<Node<'a>> {
        let (d, mut p, mut out) = (self.d, 0usize, Vec::new());
        while p < d.len() {
            if p + 6 > d.len() {
                return Vec::new();
            }
            let tag = u16::from_le_bytes([d[p], d[p + 1]]);
            let n = u32::from_le_bytes([d[p + 2], d[p + 3], d[p + 4], d[p + 5]]) as usize;
            if n > d.len() - p - 6 {
                return Vec::new();
            }
            out.push(Node { d: &d[p + 6..p + 6 + n], tag });
            p += 6 + n;
        }
        out
    }
    pub(crate) fn get(&self, tag: u16) -> Option<Node<'a>> {
        self.kids().into_iter().find(|k| k.tag == tag)
    }
    pub(crate) fn all(&self, tag: u16) -> Vec<Node<'a>> {
        self.kids().into_iter().filter(|k| k.tag == tag).collect()
    }
    pub(crate) fn at(&self, path: &[u16]) -> Option<Node<'a>> {
        path.iter().try_fold(*self, |n, t| n.get(*t))
    }
    fn num(&self) -> u64 {
        self.d.iter().take(8).rev().fold(0, |a, b| (a << 8) | *b as u64)
    }
    fn f64s(&self) -> Vec<f64> {
        self.d.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect()
    }
    pub(crate) fn text(&self) -> String {
        String::from_utf8_lossy(self.d).trim_end_matches('\0').to_string()
    }
    /// Every record with `tag` anywhere below this one, in document order. Iterative and held to `MAX_DEPTH` levels (the
    /// real tree is about 10 deep), so a file of records nested a million deep can't overflow the stack.
    fn find_all(&self, tag: u16, out: &mut Vec<Node<'a>>) {
        let mut stack: Vec<(Node<'a>, usize)> = vec![(*self, 0)];
        while let Some((n, depth)) = stack.pop() {
            let mut below = Vec::new();
            for k in n.kids() {
                if k.tag == tag {
                    out.push(k);
                } else if k.d.len() >= 6 && depth < MAX_DEPTH {
                    below.push((k, depth + 1));
                }
            }
            // the first child is looked into first
            stack.extend(below.into_iter().rev());
        }
    }
}

/// The deepest records `find_all` looks into.
const MAX_DEPTH: usize = 64;
/// The most entries a SketchUp archive may hold (a real one holds a few per material).
const MAX_ENTRIES: usize = 20_000;
/// The most one entry may inflate to, and the most all the ones that are read may inflate to together.
const MAX_MODEL_DAT: u64 = 256 << 20;
const MAX_PICTURE: u64 = 64 << 20;
const MAX_XML: u64 = 64 << 10;
const MAX_INFLATED: u64 = 384 << 20;
/// The most pictures decoded for one file, and the most pixels they may add up to.
const MAX_PICTURES: usize = 256;
const MAX_PIXELS: u64 = 160_000_000;
const MAX_SCENES: usize = 4096;

fn id_of(n: &Node) -> Option<u64> {
    n.at(&[0x5dc, 0x5de]).map(|x| x.num())
}

/// Where a modern file's archive starts, if this is one.
pub fn zip_start(d: &[u8]) -> Option<usize> {
    let head = &d[..d.len().min(256)];
    let v = head.windows(3).position(|w| w == b"VFF")?;
    head[v..].windows(4).position(|w| w == b"PK\x03\x04").map(|p| v + p)
}

#[derive(Debug, Clone)]
pub struct Texture {
    pub data: String,
    /// Tile dimensions in meters, independent of the image aspect.
    pub width: f64,
    pub height: f64,
}

pub fn parse(d: &[u8], version: String) -> Result<SkpFile> {
    let start = zip_start(d).context("not a SketchUp 2013+ archive")?;
    let mut zip = zip::ZipArchive::new(Cursor::new(&d[start..])).context("reading the SketchUp archive")?;
    // the archive's directory is checked before anything is inflated: not too many entries, no two with the same name,
    // none sharing bytes with another (one picture counted as many, a zip bomb's favorite trick)
    if zip.len() > MAX_ENTRIES {
        bail!("the SketchUp archive holds {} entries (the limit is {MAX_ENTRIES})", zip.len());
    }
    let mut spans: Vec<(u64, u64)> = Vec::new();
    let mut names = std::collections::HashSet::new();
    for i in 0..zip.len() {
        let f = zip.by_index_raw(i).context("reading the SketchUp archive")?;
        let name = String::from_utf8_lossy(f.name_raw()).to_string();
        if !names.insert(name.clone()) {
            bail!("the SketchUp archive lists \"{name}\" twice");
        }
        if f.size() > 0 {
            let Some(at) = f.data_start() else { bail!("the SketchUp archive's directory can't be read") };
            spans.push((at, at.saturating_add(f.compressed_size())));
        }
    }
    spans.sort_unstable();
    if spans.windows(2).any(|w| w[1].0 < w[0].1) {
        bail!("the SketchUp archive has entries that share the same bytes, so it was not read");
    }
    let inflated = std::cell::Cell::new(0u64);
    let mut read = |name: &str, limit: u64| -> Option<Vec<u8>> {
        let f = zip.by_name(name).ok()?;
        if f.size() > limit || inflated.get().saturating_add(f.size()) > MAX_INFLATED {
            return None;
        }
        // stop at the limit whatever the directory claims
        let mut v = Vec::new();
        f.take(limit + 1).read_to_end(&mut v).ok()?;
        if v.len() as u64 > limit {
            return None;
        }
        inflated.set(inflated.get() + v.len() as u64);
        Some(v)
    };
    let model = read("model.dat", MAX_MODEL_DAT).context("the archive has no model.dat, or it is more than 256 MB")?;
    if model.len() < 6 {
        bail!("empty model.dat");
    }
    let root = Node { d: &model[..], tag: 0 };
    let top = root.get(0x1f4).context("model.dat has an unexpected layout")?;

    // materials: id -> name; color and image from the archive's material.xml
    let mut file = SkpFile::default();
    file.version = version;
    if let Some(scenes) = top.at(&[0x207, 0x6d60, 0x6d61]) {
        file.views = scenes.all(0x7148).into_iter().take(MAX_SCENES).filter_map(view).collect();
    }
    let mut textures: HashMap<u32, Texture> = HashMap::new();
    let mut recs = Vec::new();
    top.find_all(0x32c8, &mut recs);
    let mut mat_names: HashMap<u64, String> = HashMap::new();
    for r in recs {
        let (Some(id), Some(name)) = (id_of(&r), r.get(0x32cc)) else { continue };
        mat_names.insert(id, name.text());
    }

    // definitions and the model's entities
    let defs = top.at(&[0x1f9, 0x1770, 0x1771]).map(|n| n.all(0x157c)).unwrap_or_default();
    let mut keys = Keys::default();
    for (i, def) in defs.iter().enumerate() {
        if let Some(g) = def.get(0x157e) {
            let g = g.text();
            if !g.is_empty() {
                keys.guid.insert(g, i as u32 + 1);
            }
        }
        if let Some(id) = def.at(&[0x1388, 0x7d0]).and_then(|h| id_of(&h)) {
            keys.id.insert(id, i as u32 + 1);
        }
    }
    let mut tiles: HashMap<u64, Vec<f64>> = HashMap::new();
    for (i, def) in defs.iter().enumerate() {
        let ents = def.get(0x1388).map(|e| entities(&e, &keys, &mut tiles)).unwrap_or_default();
        let name = def.get(0x157d).filter(|guid| guid.d.len() == 16)
            .and_then(|_| def.get(0x157e)).map(|n| n.text().trim().to_owned()).unwrap_or_default();
        file.definitions.insert(i as u32 + 1, (name, ents));
    }
    if let Some(e) = top.at(&[0x1f6, 0x1388]) {
        file.roots.push(entities(&e, &keys, &mut tiles));
    }
    file.vray_lighting = crate::skp_lights::read(&defs, top.at(&[0x1f6, 0x1388]));

    let (mut pictures, mut pixels) = (0usize, 0u64);
    for (&id, name) in &mat_names {
        if crate::skp::out_of_time() {
            bail!("this SketchUp file took too long to read, so it was not imported");
        }
        let Some(xml) = read(&format!("materials/{name}/material.xml"), MAX_XML) else { continue };
        let xml = String::from_utf8_lossy(&xml).to_string();
        if let Some(mut vray) = crate::skp_vray_materials::parse(&xml) {
            vray.load_embedded(|leaf| {
                if pictures >= MAX_PICTURES {
                    return None;
                }
                let bytes = read(&format!("materials/{name}/{leaf}"), MAX_PICTURE)?;
                let (w, h) = crate::safety::image_dimensions(&bytes).ok()?;
                let count = w as u64 * h as u64;
                if pixels.saturating_add(count) > MAX_PIXELS {
                    return None;
                }
                pictures += 1;
                pixels += count;
                shrink(&bytes, name).map(|(data, _)| data)
            });
            file.vray.insert(id as u32, vray);
        }
        let attr = |k: &str| -> Option<String> {
            let i = xml.find(&format!(" {k}=\""))? + k.len() + 3;
            Some(xml[i..i + xml[i..].find('"')?].to_string())
        };
        let ch = |k: &str| attr(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(200.0) / 255.0;
        let color = [ch("colorRed"), ch("colorGreen"), ch("colorBlue")];
        file.materials.insert(id as u32, (name.trim_start_matches("_LC_").to_string(), color));
        if attr("hasTexture").as_deref() == Some("1") {
            let bytes = material_image_entry(&xml, name).and_then(|entry| read(&entry, MAX_PICTURE));
            // bounded work: a count, a pixel budget and (with the deadline above) a time limit
            let shown = name.trim_start_matches("_LC_");
            if bytes.is_none() && xml.contains("<mat:image ") {
                crate::safety::note_skipped(shown, "its picture is missing, over 64 MB or can't be read; the material keeps its color");
            }
            let decoded = bytes.and_then(|b| {
                if pictures >= MAX_PICTURES {
                    crate::safety::note_skipped(shown, "this file has more pictures than are read; the material keeps its color");
                    return None;
                }
                pictures += 1;
                let (w, h) = match crate::safety::image_dimensions(&b) {
                    Ok(wh) => wh,
                    Err(_) => {
                        crate::safety::note_skipped(shown, "its picture can't be read; the material keeps its color");
                        return None;
                    }
                };
                pixels = pixels.saturating_add(w as u64 * h as u64);
                if pixels > MAX_PIXELS {
                    crate::safety::note_skipped(shown, "the pictures in this file add up to more than are read; the material keeps its color");
                    return None;
                }
                shrink(&b, shown)
            });
            if let Some((data, aspect)) = decoded {
                let mut t = tiles.remove(&id).unwrap_or_default();
                t.sort_by(|a, b| a.total_cmp(b));
                let inches = t.get(t.len() / 2).copied().filter(|v| *v > 0.5 && *v < 2000.0).unwrap_or(39.37);
                let (width, height) = texture_size(&xml, inches * INCH, aspect);
                textures.insert(id as u32, Texture { data, width, height });
            }
        }
    }
    let empty = |e: &Entities| e.faces.is_empty() && e.lines.is_empty();
    if file.definitions.values().all(|(_, e)| empty(e)) && file.roots.iter().all(empty) {
        bail!("no geometry found in this SketchUp file");
    }
    file.textures = textures;
    Ok(file)
}

/// A material can reference its own bitmap or share one stored elsewhere in the SKP.
fn material_image_entry(xml: &str, material: &str) -> Option<String> {
    let image = &xml[xml.find("<mat:image ")?..];
    let (tag, _) = image.split_once('>')?;
    let (start, key) = tag.match_indices("path=\"")
        .find(|(at, _)| tag[..*at].ends_with(char::is_whitespace))?;
    let (path, _) = tag[start + key.len()..].split_once('"')?;
    let entry = if let Some(relative) = path.strip_prefix("./") {
        format!("materials/{material}/{relative}")
    } else if path.starts_with("materials/") {
        path.to_owned()
    } else {
        return None;
    };
    if entry.contains('\\') || entry.split('/').any(|part| matches!(part, "" | "." | "..")) {
        return None;
    }
    Some(entry)
}

fn texture_size(xml: &str, fallback_width: f64, bitmap_aspect: f64) -> (f64, f64) {
    let texture = xml.find("<mat:texture ").and_then(|i| xml[i..].split_once('>')).map(|(tag, _)| tag).unwrap_or("");
    let size = |key| {
        let raw: &str = texture.split_once(key)?.1.split('"').next()?;
        let meters = raw.parse::<f64>().ok()? * INCH;
        (meters > 0.0 && model::sane(meters)).then_some(meters)
    };
    let width = size(" xScale=\"").unwrap_or(fallback_width);
    let height = size(" yScale=\"").unwrap_or(width * bitmap_aspect);
    (width, height)
}

/// Invalid or unsupported camera records are optional metadata and should
/// leave the rest of the SketchUp model importable.
fn view(page: Node<'_>) -> Option<View> {
    if page.get(0x7149)?.num() & 1 == 0 {
        return None;
    }
    let name = page.at(&[0x6f54, 0x6f55])?;
    if name.d.len() > 4096 {
        return None;
    }
    let name = std::str::from_utf8(name.d).ok()?.trim_end_matches('\0');
    if name.trim().is_empty() {
        return None;
    }
    let camera = page.at(&[0x714a, 0x34bc])?;
    let vector = |tag| {
        let bytes: &[u8; 24] = camera.get(tag)?.d.try_into().ok()?;
        Some(DVec3::from_array(std::array::from_fn(|i| {
            f64::from_le_bytes(bytes[i * 8..(i + 1) * 8].try_into().unwrap())
        })))
    };
    let scalar = |tag| Some(f64::from_le_bytes(camera.get(tag)?.d.try_into().ok()?));
    let eye = vector(0x34bd)? * INCH;
    let target = vector(0x34be)? * INCH;
    let up = vector(0x34bf)?.try_normalize()?;
    let direction = (target - eye).try_normalize()?;
    if !model::sane3(eye) || !model::sane3(target) || direction.cross(up).length_squared() <= 1e-12 {
        return None;
    }
    let parallel = match camera.get(0x34c2)?.d {
        [0] => true,
        [1] => false,
        _ => return None,
    };
    let aspect_ratio = scalar(0x34c5).filter(|&value| model::sane(value) && value > 0.0);
    let fov = scalar(0x34c4).filter(|&value| value > 0.0 && value < 180.0).and_then(|angle| {
        let vertical = if let Some(aspect) = aspect_ratio.filter(|_| !parallel && camera.get(0x34c6).is_some_and(|flag| flag.d == [0])) {
            2.0 * ((angle.to_radians() / 2.0).tan() / aspect).atan().to_degrees()
        } else {
            angle
        };
        (vertical > 0.0 && vertical < 180.0).then_some(vertical)
    });
    let ortho_height = if parallel {
        let height = scalar(0x34c3)? * INCH;
        if !model::sane(height) || height <= 0.0 {
            return None;
        }
        Some(height)
    } else {
        None
    };
    Some(View {
        name: name.to_owned(),
        eye: eye.to_array(),
        target: target.to_array(),
        fov: if parallel { fov.unwrap_or(45.0) } else { fov? },
        aspect_ratio,
        up: Some(up.to_array()),
        parallel: Some(parallel),
        ortho_height,
        photo: None,
        style: None,
        tags: None,
        hidden: None,
        section: None,
    })
}

/// A bounded texture data URL, keeping PNG color and alpha lossless after any resize.
fn shrink(bytes: &[u8], name: &str) -> Option<(String, f64)> {
    use base64::Engine as _;
    let img = match crate::safety::decode_image(bytes, None) {
        Ok(i) => i,
        Err(e) => {
            // the material keeps its color; the import says which picture was left out
            crate::safety::note_skipped(name, &format!("{e}; the material keeps its color"));
            return None;
        }
    };
    let img = if img.width().max(img.height()) > 1024 { img.thumbnail(1024, 1024) } else { img };
    let mut out = Cursor::new(Vec::new());
    let mime = if image::guess_format(bytes).ok() == Some(image::ImageFormat::Png) {
        img.write_to(&mut out, image::ImageFormat::Png).ok()?;
        "png"
    } else {
        image::DynamicImage::ImageRgb8(img.to_rgb8()).write_to(&mut out, image::ImageFormat::Jpeg).ok()?;
        "jpeg"
    };
    let aspect = img.height() as f64 / img.width().max(1) as f64;
    Some((format!("data:image/{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(out.into_inner())), aspect))
}

#[cfg(test)]
mod texture_tests {
    use super::shrink;
    use base64::Engine as _;
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
    use std::io::Cursor;

    #[test]
    fn opaque_png_texture_keeps_exact_pixels() {
        let original = RgbImage::from_fn(8, 4, |x, y| Rgb([(x * 37) as u8, (y * 83) as u8, ((x + y) * 23) as u8]));
        let mut source = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(original.clone()).write_to(&mut source, ImageFormat::Png).unwrap();

        let (data, aspect) = shrink(source.get_ref(), "pattern").unwrap();
        let encoded = data.strip_prefix("data:image/png;base64,").unwrap();
        let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).unwrap();
        let imported = image::load_from_memory_with_format(&bytes, ImageFormat::Png).unwrap().into_rgb8();

        assert_eq!(aspect, 0.5);
        assert_eq!(imported, original);
    }
}

/// How placements name their definition: by GUID text (up to SketchUp 26)
/// or by the id of the definition's entities record (SketchUp 27).
#[derive(Default)]
struct Keys {
    guid: HashMap<String, u32>,
    id: HashMap<u64, u32>,
}

impl Keys {
    fn of(&self, i: &Node) -> Option<u32> {
        let by_guid = i.get(0x1965).map(|g| g.text()).filter(|g| !g.is_empty()).and_then(|g| self.guid.get(&g).copied());
        by_guid.or_else(|| i.get(0x1967).filter(|n| !n.d.is_empty()).and_then(|n| self.id.get(&n.num()).copied()))
    }
}

fn face_mapping(header: &Node<'_>, normal: DVec3) -> Option<model::FaceUv> {
    let map = header.at(&[0x5dc, 0x5dd, 0x36b1, 0x36b2, 0x2710, 0x2711, 0x2713])?;
    if map.get(0x2714)?.num() != 1 || normal == DVec3::ZERO {
        return None;
    }
    let raw: [f64; 9] = map.get(0x2715)?.f64s().try_into().ok()?;
    let matrix = DMat3::from_cols_array(&raw);
    if matrix.determinant() == 0.0 {
        return None;
    }
    // Native near-horizontal faces use X as their plane's first axis.
    let axis = if normal.x.abs() < 1.0 / 64.0 && normal.y.abs() < 1.0 / 64.0 {
        DVec3::Y
    } else {
        DVec3::Z
    };
    let u = axis.cross(normal).normalize_or_zero();
    let v = normal.cross(u);
    // Only the zero auxiliary vector is verified for this planar placement.
    if map.get(0x2716).is_some_and(|p| p.d.len() != 24 || p.f64s().iter().any(|&x| x != 0.0)) {
        return None;
    }
    let inverse = matrix.inverse().transpose();
    let row = |r: DVec3| (r.x * u + r.y * v).extend(r.z).to_array();
    let uv = model::FaceUv { u: row(inverse.x_axis), v: row(inverse.y_axis), q: row(inverse.z_axis) };
    uv.is_finite().then_some(uv)
}

fn entities(e: &Node, keys: &Keys, tiles: &mut HashMap<u64, Vec<f64>>) -> Entities {
    let mut out = Entities::default();
    let mut verts: HashMap<u64, DVec3> = HashMap::new();
    for list in e.all(0x1389) {
        for v in list.all(0x9c4) {
            let (Some(id), Some(p)) = (id_of(&v), v.get(0x9c5)) else { continue };
            let p = p.f64s();
            if p.len() >= 3 {
                verts.insert(id, DVec3::new(p[0], p[1], p[2]));
            }
        }
    }
    let mut edges: HashMap<u64, (u64, u64, u64)> = HashMap::new();
    for list in e.all(0x138a) {
        for ed in list.all(0xbb8) {
            let Some(h) = ed.get(0x7d0) else { continue };
            let (Some(id), Some(a), Some(b)) = (id_of(&h), ed.get(0xbb9), ed.get(0xbba)) else { continue };
            let flags = h.get(0x7d3).map(|f| f.num()).unwrap_or(0);
            edges.insert(id, (a.num(), b.num(), flags));
        }
    }
    let mut used = std::collections::HashSet::new();
    for list in e.all(0x138b) {
        for f in list.all(0xdac) {
            let h = f.get(0x7d0);
            let material = h.and_then(|h| h.get(0x7d1)).map(|m| m.num()).unwrap_or(0);
            let mut rings: Vec<Vec<DVec3>> = Vec::new();
            for lp in f.get(0xdae).map(|l| l.all(0x1194)).unwrap_or_default() {
                let mut ring = Vec::new();
                for u in lp.get(0x1195).map(|x| x.all(0xfa0)).unwrap_or_default() {
                    let (Some(eid), rev) = (u.get(0xfa1).map(|x| x.num()), u.get(0xfa2).map(|x| x.num()).unwrap_or(0)) else { continue };
                    let Some(&(a, b, _)) = edges.get(&eid) else { continue };
                    used.insert(eid);
                    if let Some(p) = verts.get(if rev != 0 { &b } else { &a }) {
                        ring.push(*p);
                    }
                }
                if ring.len() >= 3 {
                    rings.push(ring);
                }
            }
            if rings.is_empty() {
                continue;
            }
            // texture matrix scale -> tile size in inches
            let mut ms = Vec::new();
            f.find_all(0x2715, &mut ms);
            if let Some(m) = ms.first().map(|m| m.f64s()) {
                if m.len() >= 6 {
                    let s = DVec3::new(m[0], m[1], m[2]).length().max(DVec3::new(m[3], m[4], m[5]).length());
                    tiles.entry(material).or_default().push(s);
                }
            }
            let outer = rings.remove(0);
            let uv = h.and_then(|header| face_mapping(&header, crate::geom::newell(&outer)));
            out.faces.push(SkpFace { outer, holes: rings, material: material as u32, uv });
        }
    }
    // soft flags for the faces' edges, and loose lines
    for (id, (a, b, flags)) in &edges {
        let (Some(pa), Some(pb)) = (verts.get(a), verts.get(b)) else { continue };
        let soft = flags & 0x10 != 0;
        if soft || !used.contains(id) {
            out.lines.push((*pa, *pb, soft));
        }
    }
    // placements: grouped in 0x138d/0x1d4c, and (SketchUp 27) listed in 0x138c
    let mut placements = Vec::new();
    for list in e.all(0x138d) {
        for g in list.all(0x1d4c) {
            placements.extend(g.all(0x1964));
        }
    }
    for list in e.all(0x138c) {
        placements.extend(list.all(0x1964));
    }
    // pictures (images) are placed the same way, one level further down
    for list in e.all(0x1390) {
        for img in list.all(0x1f40) {
            placements.extend(img.all(0x1964));
        }
    }
    {
        {
            for i in placements {
                let Some(key) = keys.of(&i) else { continue };
                let m = i.get(0x1966).map(|m| m.f64s()).unwrap_or_default();
                let t = if m.len() >= 13 {
                    let s = if m[12].abs() > 1e-12 { 1.0 / m[12] } else { 1.0 };
                    DMat4::from_cols(
                        // VFF stores the basis by rows; glam expects columns.
                        DVec4::new(m[0], m[3], m[6], 0.0),
                        DVec4::new(m[1], m[4], m[7], 0.0),
                        DVec4::new(m[2], m[5], m[8], 0.0),
                        DVec4::new(m[9] * s, m[10] * s, m[11] * s, 1.0),
                    )
                } else {
                    DMat4::IDENTITY
                };
                let material = i.at(&[0x7d0, 0x7d1]).and_then(|m| u32::try_from(m.num()).ok()).unwrap_or(0);
                out.instances.push(SkpPlacement { definition: key, transform: t, material });
            }
        }
    }
    out
}
