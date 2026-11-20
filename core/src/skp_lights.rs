//! Embedded V-Ray rectangular lights in SketchUp entity dictionaries.
//! Only local constant parameters are read. External assets are never resolved.

use crate::model::{self, Id, Model};
use crate::skp::INCH;
use crate::skp_vff::Node;
use crate::vrscene::{args, rgb, vector};
use glam::{DMat4, DVec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

pub(crate) const MAX_LIGHTS: usize = 8;
const MAX_PLUGIN_BYTES: usize = 64 * 1024;
const MAX_PLUGINS_PER_ENTITY: usize = 128;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Lighting {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) rectangles: Vec<LocalLight>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) skipped: BTreeMap<String, usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sun_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) exposure_ev100: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sky_color: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) white_balance: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reinhard: Option<[f32; 2]>,
}

impl Lighting {
    pub(crate) fn is_empty(&self) -> bool { *self == Self::default() }

    pub(crate) fn text_bytes(&self) -> usize {
        self.rectangles.iter().map(|light| light.name.len()).sum::<usize>() + self.skipped.keys().map(String::len).sum::<usize>()
    }

    pub(crate) fn size_estimate(&self) -> usize {
        self.rectangles.len() * 640 + self.skipped.len() * 40 + 6 * self.text_bytes()
    }

    pub(crate) fn merge_settings(&mut self, other: Self) {
        if self.is_empty() {
            *self = other;
            return;
        }
        for (kind, count) in other.skipped {
            let previous = self.skipped.entry(kind).or_default();
            *previous = previous.saturating_add(count);
        }
        if self.sun_enabled != other.sun_enabled { self.sun_enabled = None; }
        if self.exposure_ev100 != other.exposure_ev100 { self.exposure_ev100 = None; }
        if self.sky_color != other.sky_color { self.sky_color = None; }
        if self.white_balance != other.white_balance { self.white_balance = None; }
        if self.reinhard != other.reinhard { self.reinhard = None; }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct LocalLight {
    name: String,
    position: [f64; 3],
    u: [f64; 3],
    v: [f64; 3],
    normal: [f64; 3],
    color: [f32; 3],
    intensity: f32,
    source_intensity: f64,
    uniform_emission: bool,
    directionality: f32,
    shadows: bool,
}

#[derive(Debug, Default)]
pub(crate) struct ImportLighting {
    pub(crate) settings: Lighting,
    pub(crate) root: Vec<LocalLight>,
    pub(crate) definitions: BTreeMap<u32, Vec<LocalLight>>,
}

#[derive(Deserialize)]
struct Plugin<'a> {
    #[serde(borrow)]
    name: std::borrow::Cow<'a, str>,
    #[serde(rename = "class", borrow)]
    kind: std::borrow::Cow<'a, str>,
    params: BTreeMap<String, String>,
}

fn plugins(n: Node<'_>) -> Vec<Plugin<'_>> {
    n.at(&[0x7d0, 0x5dc, 0x5dd, 0x36b1, 0x36b2]).into_iter()
        .flat_map(|n| n.all(0x36b3))
        .filter(|dict| dict.get(0x36b4).is_some_and(|name| name.d == b"VRayPlugins"))
        .filter_map(|dict| dict.get(0x36b5))
        .flat_map(|dict| dict.all(0x38a4))
        .take(MAX_PLUGINS_PER_ENTITY)
        .filter_map(|value| value.get(0x38ad))
        .filter(|record| record.d.len() <= MAX_PLUGIN_BYTES)
        .filter_map(|record| serde_json::from_slice(record.d).ok())
        .collect()
}

impl Plugin<'_> {
    fn value(&self, key: &str) -> Option<&str> { self.params.get(key).map(String::as_str) }

    fn num(&self, key: &str, default: f64) -> Option<f64> {
        self.value(key).map_or(Some(default), |v| v.parse::<f64>().ok().filter(|v| v.is_finite()))
    }

    fn flag(&self, key: &str, default: bool) -> Option<bool> {
        match self.value(key) {
            None => Some(default),
            Some("0" | "false") => Some(false),
            Some("1" | "true") => Some(true),
            _ => None,
        }
    }

    fn linked(&self, key: &str) -> bool {
        self.value(key).is_some_and(|v| !matches!(v, "" | "null" | "NULL" | "0"))
    }
}

fn camera_ev100(camera: &Plugin<'_>) -> Option<f64> {
    if camera.flag("exposure", true) != Some(true) || camera.num("type", 0.0) != Some(0.0) { return None; }
    let ev = (camera.num("f_number", 8.0)?.powi(2) * camera.num("shutter_speed", 200.0)? * 100.0 / camera.num("ISO", 100.0)?).log2();
    (0.0..=24.0).contains(&ev).then_some(ev)
}

fn sky_color(environment: &Plugin<'_>) -> Option<[f32; 3]> {
    if environment.flag("use_gi", false) != Some(true) { return None; }
    let color = rgb(environment.value("gi_tex").or_else(|| environment.value("gi_color"))?)?;
    let multiplier = environment.num("gi_tex_mult", 1.0)?;
    let color = color.map(|v| v * multiplier);
    color.into_iter().all(|v| (0.0..=10.0).contains(&v)).then_some(color.map(|v| v as f32))
}

fn white_balance(camera: &Plugin<'_>) -> Option<[f32; 3]> {
    let white = rgb(camera.value("white_balance")?)?.map(|v| v as f32);
    valid_white_balance(white).then_some(white)
}

fn valid_white_balance(white: [f32; 3]) -> bool {
    white.into_iter().all(|v| (0.001..=16.0).contains(&v))
}

fn reinhard(mapping: &Plugin<'_>) -> Option<[f32; 2]> {
    // Modes 0 and 2 apply color mapping; the renderer still writes the usual
    // sRGB PNG transfer. There is no additional gamma stage.
    if mapping.num("type", 0.0) != Some(6.0) || !matches!(mapping.num("adaptation_only", 2.0), Some(0.0 | 2.0)) {
        return None;
    }
    let value = [mapping.num("dark_mult", 1.0)? as f32, mapping.num("bright_mult", 1.0)? as f32];
    valid_reinhard(value).then_some(value)
}

fn valid_reinhard([dark, burn]: [f32; 2]) -> bool {
    (0.0..=64.0).contains(&dark) && (0.0..=1.0).contains(&burn)
}

fn rectangle(p: &Plugin<'_>, exposure: f64) -> Option<LocalLight> {
    let [m, point] = args::<2>(p.value("transform").unwrap_or("Transform(Matrix(Vector(1,0,0),Vector(0,1,0),Vector(0,0,1)),Vector(0,0,0))"), "Transform")?;
    let [u, v, w] = args::<3>(m, "Matrix")?;
    let (u, v, w) = (vector(u)?, vector(v)?, vector(w)?);
    let position = vector(point)? * INCH;
    let width = 2.0 * p.num("u_size", 0.5)? * INCH;
    let height = 2.0 * p.num("v_size", 0.5)? * INCH;
    let normal = -w.try_normalize()?;
    let color = p.value("color").map_or(Some([1.0; 3]), rgb)?;
    let peak = color.into_iter().fold(1.0, f64::max);
    let color = color.map(|v| {
        let v = v / peak;
        (if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }) as f32
    });
    let source_intensity = p.num("intensity", 1.0)?;
    let intensity = source_intensity * peak * exposure;
    let directionality = p.num("directional", 0.0)?;
    if !model::sane3(position) || ![width, height].into_iter().all(|v| model::sane(v) && v >= 0.001)
        || !(0.0..=10000.0).contains(&intensity) || !(0.0..=1.0).contains(&directionality) {
        return None;
    }
    Some(LocalLight {
        name: p.name.chars().take(256).collect(),
        position: position.to_array(), u: (u * width).to_array(), v: (v * height).to_array(),
        normal: normal.to_array(), color, intensity: intensity as f32, source_intensity,
        uniform_emission: p.flag("ignoreLightNormals", false)?, directionality: directionality as f32, shadows: p.flag("shadows", true)?,
    })
}

fn read_lights(plugins: Vec<Plugin<'_>>, skipped: &mut BTreeMap<String, usize>, exposure: f64) -> Vec<LocalLight> {
    let mut lights = Vec::new();
    for p in plugins {
        let reason = if p.kind == "SunLight" {
            continue;
        } else if !p.kind.starts_with("Light") {
            continue;
        } else if p.flag("enabled", true) == Some(false) {
            "disabled"
        } else if p.kind != "LightRectangle" {
            "other_light"
        } else if p.num("units", 0.0) != Some(0.0) {
            "photometric_units"
        } else if ["is_disc", "doubleSided", "noDecay", "lightPortal", "use_rect_tex", "decay_near_on", "decay_far_on"].iter().any(|k| p.flag(k, false) != Some(false))
            || ["enabled", "affectDiffuse", "affectSpecular"].iter().any(|k| p.flag(k, true) != Some(true))
            || p.num("directional_strength", 1.0) != Some(1.0)
            || p.linked("color_tex") || p.linked("intensity_tex") {
            "unsupported_rectangle"
        } else if let Some(light) = rectangle(&p, exposure) {
            lights.push(light);
            continue;
        } else {
            "invalid_rectangle"
        };
        *skipped.entry(reason.into()).or_default() += 1;
    }
    lights
}

pub(crate) fn read(definitions: &[Node<'_>], root: Option<Node<'_>>) -> ImportLighting {
    let mut data = ImportLighting::default();
    let root_plugins = root.map(plugins).unwrap_or_default();
    let known_space = root_plugins.iter().find(|p| p.kind == "SettingsUnitsInfo").is_none_or(|p| {
        p.num("coordinate_system", 0.0) == Some(0.0) && p.num("rgb_color_space", 1.0) == Some(1.0)
            && !p.linked("ocio_config_file")
            && p.value("scene_upDir").is_none_or(|v| vector(v).is_some_and(|up| up == DVec3::Z))
    });
    let camera = root_plugins.iter().find(|p| p.kind == "CameraPhysical");
    data.settings.exposure_ev100 = camera.and_then(camera_ev100);
    data.settings.sun_enabled = root_plugins.iter().find(|p| p.kind == "SunLight").and_then(|p| p.flag("enabled", true));
    data.settings.sky_color = root_plugins.iter().find(|p| p.kind == "SettingsEnvironment").and_then(sky_color);
    if !known_space {
        data.settings.sky_color = None;
        data.settings.skipped.insert("scene_space".into(), 1);
        return data;
    }
    data.settings.white_balance = camera.and_then(white_balance);
    data.settings.reinhard = root_plugins.iter().find(|p| p.kind == "SettingsColorMapping").and_then(reinhard);
    // V-Ray Default units are renderer-specific scalar intensity. Transfer the
    // camera's relative exposure around a calibrated Officina reference (EV100 12).
    let exposure = 0.05 * 2.0_f64.powf(12.0 - data.settings.exposure_ev100.unwrap_or(12.0));
    data.root = read_lights(root_plugins, &mut data.settings.skipped, exposure);
    let mut total = data.root.len();
    for (i, def) in definitions.iter().enumerate() {
        if crate::skp::out_of_time() { break; }
        let Some(entities) = def.get(0x1388) else { continue };
        let mut lights = read_lights(plugins(entities), &mut data.settings.skipped, exposure);
        let room = 1024usize.saturating_sub(total);
        if lights.len() > room {
            *data.settings.skipped.entry("light_limit".into()).or_default() += lights.len() - room;
            lights.truncate(room);
        }
        if !lights.is_empty() {
            total += lights.len();
            data.definitions.insert(i as u32 + 1, lights);
        }
    }
    data
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct WorldLight {
    pub(crate) name: String,
    pub(crate) position: [f64; 3],
    pub(crate) target: [f64; 3],
    pub(crate) u_axis: [f64; 3],
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) intensity: f32,
    pub(crate) color: [f32; 3],
    pub(crate) uniform_emission: bool,
    pub(crate) directionality: f32,
    pub(crate) shadows: bool,
    pub(crate) source_intensity: f64,
}

impl LocalLight {
    fn placed(&self, t: DMat4) -> Option<WorldLight> {
        let position = t.transform_point3(DVec3::from(self.position));
        let u = t.transform_vector3(DVec3::from(self.u));
        let v = t.transform_vector3(DVec3::from(self.v));
        let u_axis = u.try_normalize()?;
        let v_axis = v.try_normalize()?;
        let facing = t.transform_vector3(DVec3::from(self.normal)).try_normalize()?;
        let mut normal = u_axis.cross(v_axis).try_normalize()?;
        if normal.dot(facing) < 0.0 { normal = -normal; }
        let target = position + normal;
        let (width, height) = (u.length(), v.length());
        if !model::sane3(position) || !model::sane3(target)
            || ![width, height].into_iter().all(|v| model::sane(v) && v >= 0.001)
            || u_axis.dot(v_axis).abs() > 1e-4
            || !(0.0..=10000.0).contains(&self.intensity) || !(0.0..=1.0).contains(&self.directionality)
            || !self.color.iter().all(|v| (0.0..=1.0).contains(v)) {
            return None;
        }
        Some(WorldLight {
            name: self.name.clone(), position: position.to_array(), target: target.to_array(),
            u_axis: u_axis.to_array(), width, height, intensity: self.intensity, color: self.color,
            uniform_emission: self.uniform_emission, directionality: self.directionality, shadows: self.shadows,
            source_intensity: self.source_intensity,
        })
    }
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct WorldLighting {
    pub(crate) area_lights: Vec<WorldLight>,
    skipped: BTreeMap<String, usize>,
    pub(crate) sun_enabled: Option<bool>,
    exposure_ev100: Option<f64>,
    pub(crate) sky_color: Option<[f32; 3]>,
    pub(crate) white_balance: Option<[f32; 3]>,
    pub(crate) reinhard: Option<[f32; 2]>,
}

impl WorldLighting {
    pub(crate) fn has_data(&self) -> bool {
        !self.area_lights.is_empty() || !self.skipped.is_empty() || self.sun_enabled.is_some() || self.exposure_ev100.is_some()
            || self.sky_color.is_some() || self.white_balance.is_some() || self.reinhard.is_some()
    }
}

pub(crate) fn world(model: &Model) -> WorldLighting {
    let mut report = WorldLighting { skipped: model.skp_lighting.skipped.clone(), sun_enabled: model.skp_lighting.sun_enabled,
        exposure_ev100: model.skp_lighting.exposure_ev100,
        sky_color: model.skp_lighting.sky_color.filter(|channels| channels.into_iter().all(|v| (0.0..=10.0).contains(v))),
        white_balance: model.skp_lighting.white_balance.filter(|value| valid_white_balance(*value)),
        reinhard: model.skp_lighting.reinhard.filter(|value| valid_reinhard(*value)),
        ..Default::default() };
    let mut parents: HashMap<Id, Vec<Id>> = HashMap::new();
    let mut pending = Vec::new();
    for (&id, def) in &model.definitions {
        if !def.model.skp_lighting.rectangles.is_empty() { pending.push(id); }
        for child in def.model.instances.values() {
            parents.entry(child.def).or_default().push(id);
        }
    }
    let mut lit = HashSet::new();
    while let Some(id) = pending.pop() {
        if lit.insert(id) {
            pending.extend(parents.get(&id).into_iter().flatten().copied());
        }
    }
    fn visit(root: &Model, model: &Model, t: DMat4, lit: &HashSet<Id>, depth: usize, work: &mut usize, report: &mut WorldLighting) {
        if depth > crate::nesting::MAX_NEST || *work == 0 { return; }
        *work -= 1;
        for light in &model.skp_lighting.rectangles {
            if report.area_lights.len() == MAX_LIGHTS {
                let count = report.skipped.entry("light_limit".into()).or_default();
                *count = count.saturating_add(1);
                return;
            }
            if let Some(light) = light.placed(t) {
                report.area_lights.push(light);
            } else {
                let count = report.skipped.entry("invalid_rectangle".into()).or_default();
                *count = count.saturating_add(1);
            }
        }
        for i in model.instances.values().filter(|i| lit.contains(&i.def) && !i.hidden && model.tag_visible(&i.tag) && model.group_visible(i.parent)) {
            if let Some(d) = root.definitions.get(&i.def) {
                visit(root, &d.model, t * i.transform, lit, depth + 1, work, report);
            }
            if report.skipped.contains_key("light_limit") && report.area_lights.len() == MAX_LIGHTS { break; }
        }
    }
    let mut work = 65_536;
    visit(model, model, DMat4::IDENTITY, &lit, 0, &mut work, &mut report);
    if work == 0 {
        let count = report.skipped.entry("light_limit".into()).or_default();
        *count = count.saturating_add(1);
    }
    report
}
