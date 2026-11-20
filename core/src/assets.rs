//! Built-in asset library. Each asset is a command script (core/assets/*.jsonl)
//! run in a scratch engine; the result becomes a component, so every door you
//! place shares one definition.

use crate::model::{Definition, Id, Model};
use crate::Engine;
use anyhow::{bail, Result};

pub struct Asset {
    pub key: &'static str,
    pub name: &'static str,
    pub about: &'static str,
    pub script: &'static str,
    /// a flat figure that turns to face the camera
    pub face_me: bool,
}

pub const ASSETS: &[Asset] = &[
    Asset { key: "door", name: "Door", about: "Panel door 1.0 × 2.1 m", script: include_str!("../assets/door.jsonl"), face_me: false },
    Asset { key: "window", name: "Window", about: "Framed window 1.2 × 1.5 m", script: include_str!("../assets/window.jsonl"), face_me: false },
    Asset { key: "garage_door", name: "Garage door", about: "Sectional door 2.7 × 2.2 m", script: include_str!("../assets/garage_door.jsonl"), face_me: false },
    Asset { key: "tree", name: "Tree", about: "Street tree, 4.5 m", script: include_str!("../assets/tree.jsonl"), face_me: false },
    Asset { key: "palm", name: "Palm", about: "Canary Island palm, 7.5 m", script: include_str!("../assets/palm.jsonl"), face_me: false },
    Asset { key: "car", name: "Car", about: "Compact car 4.4 m", script: include_str!("../assets/car.jsonl"), face_me: false },
    Asset { key: "streetlight", name: "Streetlight", about: "6 m pole with arm", script: include_str!("../assets/streetlight.jsonl"), face_me: false },
    Asset { key: "bench", name: "Bench", about: "Park bench 1.8 m", script: include_str!("../assets/bench.jsonl"), face_me: false },
    Asset { key: "person", name: "Person", about: "Scale figure 1.75 m, always faces you", script: include_str!("../assets/person.jsonl"), face_me: true },
    Asset { key: "beaver", name: "Beaver", about: "The SketchCraft beaver with a clipboard, 1.5 m, always faces you", script: include_str!("../assets/beaver.jsonl"), face_me: true },
    Asset { key: "hydrant", name: "Hydrant", about: "Fire hydrant", script: include_str!("../assets/hydrant.jsonl"), face_me: false },
    // scale figures for paths and streets (tools/scale_figures/scale_figures.py)
    Asset { key: "walker", name: "Walker", about: "Scale figure 1.78 m walking, always faces you", script: include_str!("../assets/walker.jsonl"), face_me: true },
    Asset { key: "shopper", name: "Shopper", about: "Scale figure 1.66 m with a tote bag, always faces you", script: include_str!("../assets/shopper.jsonl"), face_me: true },
    Asset { key: "jogger", name: "Jogger", about: "Scale figure 1.70 m running, always faces you", script: include_str!("../assets/jogger.jsonl"), face_me: true },
    Asset { key: "child", name: "Child", about: "Scale figure 1.15 m, always faces you", script: include_str!("../assets/child.jsonl"), face_me: true },
    Asset { key: "dog_walker", name: "Dog walker", about: "Scale figure walking a dog, 1.75 m, always faces you", script: include_str!("../assets/dog_walker.jsonl"), face_me: true },
    Asset { key: "stroller", name: "Stroller", about: "Scale figure pushing a stroller, 1.70 m, always faces you", script: include_str!("../assets/stroller.jsonl"), face_me: true },
    Asset { key: "wheelchair", name: "Wheelchair user", about: "Scale figure in a wheelchair, 1.30 m, always faces you", script: include_str!("../assets/wheelchair.jsonl"), face_me: true },
    Asset { key: "cyclist", name: "Cyclist", about: "Scale figure on a bike, 1.75 m long, always faces you", script: include_str!("../assets/cyclist.jsonl"), face_me: true },
    Asset { key: "car_flat", name: "Car (flat)", about: "Flat hatchback 4.3 m in profile, for elevations; rotate it along the road", script: include_str!("../assets/car_flat.jsonl"), face_me: false },
    Asset { key: "suv_flat", name: "SUV (flat)", about: "Flat SUV 4.7 m in profile, for elevations; rotate it along the road", script: include_str!("../assets/suv_flat.jsonl"), face_me: false },
];

pub fn find(key: &str) -> Option<&'static Asset> {
    let k = key.to_ascii_lowercase().replace(' ', "_");
    ASSETS.iter().find(|a| a.key == k || a.name.eq_ignore_ascii_case(key))
}

/// Build an asset's geometry by running its script.
pub fn build(asset: &Asset) -> Result<Model> {
    let mut e = Engine::new();
    for (n, line) in asset.script.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let r = e.exec_json(line);
        if r["ok"] != true {
            bail!("asset {} line {}: {}", asset.key, n + 1, r["error"]);
        }
    }
    Ok(e.model)
}

/// The component for an asset in `m`, building it on first use.
pub fn definition(m: &mut Model, key: &str) -> Result<Id> {
    let Some(asset) = find(key) else { bail!("no asset {key}; try: {}", ASSETS.iter().map(|a| a.key).collect::<Vec<_>>().join(", ")) };
    if let Some(id) = m.find_definition(asset.name) {
        return Ok(id);
    }
    let model = build(asset)?;
    let id = m.new_id();
    // doors and windows cut their opening in the wall they're placed on
    let cuts = matches!(asset.key, "door" | "window" | "garage_door");
    m.definitions.insert(id, Definition { name: asset.name.to_string(), model: std::sync::Arc::new(model), attrs: Default::default(), source: None, cuts, face_me: asset.face_me });
    Ok(id)
}
