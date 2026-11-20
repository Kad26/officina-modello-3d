//! Read constant rectangular lights from a V-Ray scene export and its
//! scene includes. Includes must stay within the directory of the selected file.
//! Geometry, material graphs and other asset references are never loaded.

use anyhow::{bail, ensure, Context, Result};
use glam::DVec3;
use serde::Serialize;
use std::{collections::{BTreeMap, HashSet}, fs::File, io::Read, path::{Component, Path, PathBuf}};

const MAX_BYTES: usize = 128 * 1024 * 1024;
const MAX_LIGHTS: usize = 8;
const MAX_FILES: usize = 48;
type Fields<'a> = BTreeMap<&'a str, &'a str>;

#[derive(Serialize)]
pub(crate) struct Report {
    area_lights: Vec<Rectangle>,
    skipped: BTreeMap<&'static str, usize>,
    warnings: Vec<&'static str>,
    materials: usize,
}

#[derive(Serialize)]
struct Rectangle {
    position: [f64; 3],
    target: [f64; 3],
    u_axis: [f64; 3],
    width: f64,
    height: f64,
    intensity: f64,
    color: [f64; 3],
    uniform_emission: bool,
}

pub(crate) fn read(path: &Path) -> Result<Report> {
    ensure!(has_extension(path), "choose a .vrscene export");
    let path = path.canonicalize()?;
    let root = path.parent().context("the .vrscene file needs a directory")?;
    let mut loaded = SceneFiles { texts: Vec::new(), seen: HashSet::new(), bytes: 0, skipped: 0 };
    loaded.load(root, &path)?;
    parse(&loaded.texts, loaded.skipped)
}

fn has_extension(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("vrscene"))
}

struct SceneFiles {
    texts: Vec<String>,
    seen: HashSet<PathBuf>,
    bytes: usize,
    skipped: usize,
}

impl SceneFiles {
    fn load(&mut self, root: &Path, path: &Path) -> Result<()> {
        if self.seen.contains(path) { return Ok(()); }
        if self.seen.len() >= MAX_FILES {
            self.skipped += 1;
            return Ok(());
        }
        self.seen.insert(path.to_path_buf());
        let text = read_text(path, MAX_BYTES - self.bytes)?;
        self.bytes += text.len();
        let (includes, excess) = for_blocks(&text, |_, _| Ok(()))?;
        let includes = includes.into_iter().map(str::to_owned).collect::<Vec<_>>();
        self.skipped += excess;
        self.texts.push(text);
        let parent = path.parent().context("the included scene needs a directory")?;
        for include in includes {
            let rel = Path::new(include.trim());
            if !has_extension(rel) || !rel.components().all(|part| matches!(part, Component::Normal(_) | Component::CurDir)) {
                self.skipped += 1;
                continue;
            }
            let Ok(file) = parent.join(rel).canonicalize() else {
                self.skipped += 1;
                continue;
            };
            if !file.starts_with(root) || !file.is_file() {
                self.skipped += 1;
                continue;
            }
            self.load(root, &file)?;
        }
        Ok(())
    }
}

fn read_text(path: &Path, bytes_left: usize) -> Result<String> {
    ensure!(path.metadata()?.is_file(), "choose a regular .vrscene file");
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    file.take((bytes_left + 1) as u64).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= bytes_left, "the V-Ray scene and includes exceed 128 MiB");
    String::from_utf8(bytes).context("the V-Ray scene must be a UTF-8 text export")
}

struct Scanner<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Scanner<'a> {
    fn new(text: &'a str) -> Self { Self { text, at: 0 } }
    fn rest(&self) -> &'a str { &self.text[self.at..] }

    fn trivia(&mut self) -> Result<()> {
        loop {
            self.at = self.text.len() - self.rest().trim_start().len();
            if self.rest().starts_with("//") {
                self.line();
            } else if self.rest().starts_with("/*") {
                let n = self.rest()[2..].find("*/").context("unclosed V-Ray comment")?;
                self.at += n + 4;
            } else {
                return Ok(());
            }
        }
    }

    fn line(&mut self) {
        self.at += self.rest().find('\n').unwrap_or(self.rest().len());
    }

    fn quoted(&mut self) -> Result<()> {
        self.at += 1;
        let mut escaped = false;
        for (i, c) in self.rest().char_indices() {
            if c == '"' && !escaped {
                self.at += i + 1;
                return Ok(());
            }
            escaped = c == '\\' && !escaped;
        }
        bail!("unclosed V-Ray string")
    }

    /// Find the closing delimiter without treating quoted values or comments as syntax.
    fn delimited(&mut self, end: u8) -> Result<&'a str> {
        let start = self.at;
        let mut nesting = 0usize;
        while let Some(b) = self.rest().as_bytes().first().copied() {
            if b == b'"' {
                self.quoted()?;
            } else if self.rest().starts_with("//") || self.rest().starts_with("/*") {
                self.trivia()?;
            } else if b == end && nesting == 0 {
                let value = &self.text[start..self.at];
                self.at += 1;
                return Ok(value);
            } else {
                match b {
                    b'{' | b'(' | b'[' => nesting += 1,
                    b'}' | b')' | b']' => nesting = nesting.checked_sub(1).context("unbalanced V-Ray expression")?,
                    _ => {}
                }
                self.at += self.rest().chars().next().expect("nonempty").len_utf8();
            }
        }
        bail!("unclosed V-Ray block or property")
    }

    fn word(&mut self) -> Result<&'a str> {
        self.trivia()?;
        let start = self.at;
        if self.rest().starts_with('"') {
            self.quoted()?;
        } else {
            let len = self.rest().find(|c: char| c.is_whitespace() || matches!(c, '{' | '}' | '=' | ';')).unwrap_or(self.rest().len());
            self.at += len;
        }
        ensure!(self.at > start, "invalid V-Ray property or plugin name");
        Ok(&self.text[start..self.at])
    }

    fn consume(&mut self, c: char) -> Result<()> {
        self.trivia()?;
        ensure!(self.rest().starts_with(c), "expected {c:?} in the V-Ray scene");
        self.at += c.len_utf8();
        Ok(())
    }
}

fn for_blocks<'a>(text: &'a str, mut block: impl FnMut(&'a str, &'a str) -> Result<()>) -> Result<(Vec<&'a str>, usize)> {
    let mut scan = Scanner::new(text.trim_start_matches('\u{feff}'));
    let mut includes = Vec::new();
    let mut excess = 0;
    loop {
        scan.trivia()?;
        if scan.rest().is_empty() { return Ok((includes, excess)); }
        if scan.rest().starts_with('#') {
            if let Some(arg) = scan.rest().lines().next().and_then(|s| s.strip_prefix("#include")) {
                if includes.len() < MAX_FILES {
                    includes.push(arg.trim().strip_prefix('"').and_then(|s| s.split_once('"')).map_or("", |(path, _)| path));
                } else {
                    excess += 1;
                }
            }
            scan.line();
            continue;
        }
        let kind = scan.word()?;
        scan.word()?;
        scan.consume('{')?;
        block(kind, scan.delimited(b'}')?)?;
        scan.trivia()?;
        if scan.rest().starts_with(';') { scan.at += 1; }
    }
}

fn fields(text: &str) -> Result<Fields<'_>> {
    let mut scan = Scanner::new(text);
    let mut values = BTreeMap::new();
    loop {
        scan.trivia()?;
        if scan.rest().is_empty() { return Ok(values); }
        let key = scan.word()?;
        scan.consume('=')?;
        values.insert(key, scan.delimited(b';')?.trim());
    }
}

fn number(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

fn num(fields: &Fields<'_>, key: &str, default: f64) -> Option<f64> {
    fields.get(key).map_or(Some(default), |value| number(value))
}

fn flag(fields: &Fields<'_>, key: &str, default: bool) -> Option<bool> {
    match fields.get(key).copied() {
        None => Some(default),
        Some("0" | "false") => Some(false),
        Some("1" | "true") => Some(true),
        _ => None,
    }
}

fn has_link(fields: &Fields<'_>, key: &str) -> bool {
    fields.get(key).is_some_and(|value| !matches!(*value, "NULL" | "null" | "0" | "\"\""))
}

/// A simple constant constructor (split arguments at top-level commas).
pub(crate) fn args<'a, const N: usize>(s: &'a str, tag: &str) -> Option<[&'a str; N]> {
    let mut scan = Scanner::new(s.strip_prefix(tag)?.trim_start());
    scan.consume('(').ok()?;
    let inner = scan.delimited(b')').ok()?;
    if !scan.rest().trim().is_empty() { return None; }
    let mut scan = Scanner::new(inner);
    let mut items = [""; N];
    for (i, item) in items.iter_mut().enumerate() {
        *item = if i + 1 < N { scan.delimited(b',').ok()?.trim() } else { scan.rest().trim() };
    }
    Some(items)
}

pub(crate) fn vector(s: &str) -> Option<DVec3> {
    let [x, y, z] = args::<3>(s, "Vector")?;
    Some(DVec3::new(number(x)?, number(y)?, number(z)?))
}

pub(crate) fn rgb(s: &str) -> Option<[f64; 3]> {
    let [r, g, b] = if s.starts_with("AColor") {
        let [r, g, b, _alpha] = args::<4>(s, "AColor")?;
        [r, g, b]
    } else {
        args::<3>(s, "Color")?
    };
    let rgb = [number(r)?, number(g)?, number(b)?];
    rgb.iter().all(|v| *v >= 0.0).then_some(rgb)
}

fn rectangle(fields: &Fields<'_>, scale: f64) -> Option<Rectangle> {
    let [matrix, position] = args::<2>(fields.get("transform")?, "Transform")?;
    let [u, v, w] = args::<3>(matrix, "Matrix")?;
    let (u, v, w) = (vector(u)?, vector(v)?, vector(w)?);
    let (width_axis, height_axis, normal) = (u.try_normalize()?, v.try_normalize()?, w.try_normalize()?);
    if width_axis.dot(height_axis).abs() > 1e-5 || width_axis.dot(normal).abs() > 1e-5 || height_axis.dot(normal).abs() > 1e-5 {
        return None;
    }
    let position = vector(position)? * scale;
    // V-Ray's exported rectangle faces the negative third transform axis.
    let target = position - normal;
    let width = 2.0 * num(fields, "u_size", 0.5)? * u.length() * scale;
    let height = 2.0 * num(fields, "v_size", 0.5)? * v.length() * scale;
    let color = match fields.get("color") { Some(s) => rgb(s)?, None => [1.0; 3] };
    let peak = color.into_iter().fold(1.0, f64::max);
    let intensity = num(fields, "intensity", 1.0)? * peak;
    // VRay uses linear channels. The Officina command accepts sRGB.
    let color = color.map(|v| {
        let v = v / peak;
        if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
    });
    if !crate::model::sane3(position) || !crate::model::sane3(target) ||
        ![width, height].iter().all(|v| crate::model::sane(*v) && *v >= 0.001) ||
        !(0.0..=10000.0).contains(&intensity) { return None; }
    Some(Rectangle { position: position.to_array(), target: target.to_array(), u_axis: width_axis.to_array(), width, height, intensity, color,
        uniform_emission: flag(fields, "ignoreLightNormals", false)? })
}

fn parse(texts: &[String], skipped_includes: usize) -> Result<Report> {
    let mut report = Report { area_lights: Vec::new(), skipped: BTreeMap::new(), warnings: Vec::new(), materials: 0 };
    let mut units = None;
    let mut supported_axes = true;
    let mut supported_colors = true;
    let mut has_blocks = false;
    for text in texts { for_blocks(text, |kind, body| {
        has_blocks = true;
        if kind == "SettingsUnitsInfo" {
            let f = fields(body)?;
            units = f.get("meters_scale").and_then(|v| number(v)).filter(|v| *v > 0.0);
            supported_axes = num(&f, "coordinate_system", 0.0) == Some(0.0) &&
                f.get("scene_upDir").is_none_or(|v| vector(v).and_then(DVec3::try_normalize).is_some_and(|n| n.dot(DVec3::Z) > 0.999));
            supported_colors = num(&f, "rgb_color_space", 1.0) == Some(1.0) && !has_link(&f, "ocio_config_file");
        }
        if kind.starts_with("Mtl") || kind.starts_with("BRDF") { report.materials += 1; }
        Ok(())
    })?; }
    ensure!(has_blocks || skipped_includes > 0, "no V-Ray plugin blocks found");
    if skipped_includes > 0 {
        report.skipped.insert("include", skipped_includes);
        report.warnings.push("some scene includes could not be read inside the selected folder");
    }
    for text in texts { for_blocks(text, |kind, body| {
        let reason = if kind == "LightRectangle" {
            let f = fields(body)?;
            if flag(&f, "enabled", true) == Some(false) {
                "disabled"
            } else if !supported_axes || !supported_colors || units.is_none() {
                "scene_space"
            } else if flag(&f, "is_disc", false) != Some(false) || flag(&f, "doubleSided", false) != Some(false) ||
                flag(&f, "noDecay", false) != Some(false) || flag(&f, "lightPortal", false) != Some(false) ||
                (flag(&f, "use_rect_tex", false) != Some(false) &&
                    (flag(&f, "use_rect_tex", false) != Some(true) || f.get("rect_tex").and_then(|s| rgb(s)) != Some([1.0; 3]))) ||
                flag(&f, "shadows", true) != Some(true) || flag(&f, "affectDiffuse", true) != Some(true) ||
                flag(&f, "affectSpecular", true) != Some(true) || flag(&f, "enabled", true) != Some(true) ||
                num(&f, "directional", 0.0) != Some(0.0) || has_link(&f, "color_tex") || has_link(&f, "intensity_tex") {
                "unsupported_rectangle"
            } else if !matches!(num(&f, "units", 0.0), Some(0.0 | 4.0)) {
                "photometric_units"
            } else if let Some(light) = rectangle(&f, units.expect("checked before conversion")) {
                if report.area_lights.len() == MAX_LIGHTS { "light_limit" } else {
                    report.area_lights.push(light);
                    return Ok(());
                }
            } else {
                "invalid_rectangle"
            }
        } else if kind.starts_with("Light") || kind == "SunLight" {
            "other_light"
        } else {
            return Ok(());
        };
        *report.skipped.entry(reason).or_default() += 1;
        Ok(())
    })?; }
    if !report.area_lights.is_empty() {
        report.warnings.push("output brightness is approximate and visible emitter geometry is not preserved");
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECT: &str = r#"
        LightRectangle lamp {
            transform=Transform(Matrix(Vector(0,2,0),Vector(-1,0,0),Vector(0,0,1)),Vector(50,-100,200));
            color=AColor(0.25,0.5,1.0,1);
            intensity=8;
            u_size=25;
            v_size=100;
            use_rect_tex=1;
            rect_tex=AColor(1,1,1,1);
            ignoreLightNormals=1;
        }
    "#;

    #[test]
    fn reads_scene_units_orientation_and_linear_color() {
        let settings = "SettingsUnitsInfo settings { meters_scale=.01; scene_upDir=Vector(0,0,1); }";
        let extras = r#"
            Node ignored { filename="ignore } ; \" further"; }
            BRDFVRayMtl mat { diffuse=Color(.5,.5,.5); }
            LightDome unsupported { enabled=1; }
            LightRectangle disabled { enabled=false; }
        "#;
        let report = parse(&[settings.to_owned(), format!("{RECT}{extras}")], 0).unwrap();
        assert_eq!(report.materials, 1);
        assert_eq!(report.skipped["disabled"], 1);
        assert_eq!(report.skipped["other_light"], 1);
        let light = &report.area_lights[0];
        assert_eq!(light.position, [0.5, -1.0, 2.0]);
        assert_eq!(light.target, [0.5, -1.0, 1.0]);
        assert_eq!(light.u_axis, [0.0, 1.0, 0.0]);
        assert_eq!((light.width, light.height, light.intensity), (1.0, 2.0, 8.0));
        assert!(light.uniform_emission);
        assert!((light.color[0] - 0.53709873).abs() < 1e-6);
    }

    #[test]
    fn reports_incompatible_frames_and_light_modes() {
        let units = "SettingsUnitsInfo settings { meters_scale=1; }";
        let portal = RECT.replace("intensity=8;", "intensity=8; lightPortal=1;");
        let physical = RECT.replace("intensity=8;", "intensity=8; units=1;");
        let report = parse(&[format!("{units}{portal}{physical}")], 1).unwrap();
        assert!(report.area_lights.is_empty());
        assert_eq!(report.skipped["include"], 1);
        assert_eq!(report.skipped["photometric_units"], 1);
        assert_eq!(report.skipped["unsupported_rectangle"], 1);
        assert_eq!(parse(&[RECT.to_owned()], 0).unwrap().skipped["scene_space"], 1);
    }
}
