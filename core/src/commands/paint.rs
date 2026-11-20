//! Paint, textures, materials and skins.
//! Arms of `Engine::run`; each works on the model being edited (inside `edit`, the open component's definition).

use super::*;
use crate::model::{pairs, Material};
use command::PaintScope;
use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;
mod global;
use global::Finish;

fn paint_matching(
    root: &mut Model, context: Option<Id>, face: Id, placement: Vec<Id>, scope: PaintScope,
    color: Option<[f32; 3]>, material: Option<String>,
) -> Result<Value> {
    if placement.len() > crate::nesting::MAX_NEST {
        bail!("component placement path is too deep");
    }
    let mut object = context;
    let mut inherited = context.and_then(|def| {
        let i = root.instances.values().find(|i| i.def == def)?;
        Some(Finish::Default.placement(root, root, i, root.definitions.get(&def)?))
    }).unwrap_or(Finish::Default);
    if let Some(&first) = placement.first() {
        if context.is_some_and(|context| placement.len() < 2 || root.instances.get(&first).is_none_or(|i| i.def != context)) {
            bail!("finish editing the open component before painting another component");
        }
        object = None;
        inherited = Finish::Default;
        for &id in &placement {
            let model = definition_model(root, object)?;
            model.check_unlocked(&[id])?;
            let instance = model.instances.get(&id).ok_or_else(|| anyhow::anyhow!("no component placement {id}"))?;
            let def = root.definitions.get(&instance.def).ok_or_else(|| anyhow::anyhow!("no component definition {}", instance.def))?;
            inherited = inherited.placement(root, model, instance, def);
            object = Some(instance.def);
        }
    }
    let model = definition_model(root, object)?;
    let seed = model.faces.get(&face).ok_or_else(|| anyhow::anyhow!("no face {face}"))?;
    model.check_unlocked(&[face])?;
    let source = match inherited.apply(model.group_material(seed.group)).apply(seed.material.as_deref()) {
        Finish::Default => None,
        Finish::Material(name) => Some(name.to_string()),
        Finish::DynamicColor => bail!("this color comes from a dynamic component attribute; edit the attribute or paint the face directly"),
    };
    let inherited = match inherited { Finish::Material(name) => Some(name), _ => None };
    let (name, replacement) = resolve_paint(root, model, color, material)?;
    if source.as_deref() == Some(&name) {
        return Ok(json!({"material": name, "faces": 0}));
    }

    let changed = match scope {
        PaintScope::All => match source.as_deref() {
            Some(source) if !has_locks(root) => replace_named(root, source, &name, &replacement),
            source => global::replace(root, source.map_or(Finish::Default, Finish::Material), &name, &replacement)?,
        },
        PaintScope::Connected | PaintScope::Object => {
            let faces = if matches!(scope, PaintScope::Connected) {
                connected_matching_faces(model, face, source.as_deref(), inherited)
            } else if seed.group.is_some() || object.is_some() {
                model.faces.iter().filter(|(_, f)| f.group == seed.group
                    && f.material.as_deref().or_else(|| model.group_material(f.group)).or(inherited) == source.as_deref()
                    && model.locked_by(f.group).is_none())
                    .map(|(&id, _)| id).collect()
            } else {
                vec![face]
            };
            let mut isolated = context;
            // Opening a component edits its shared definition. Outside edit mode,
            // isolate each shared definition along the clicked placement path.
            for &id in placement.iter().skip(usize::from(context.is_some())) {
                isolated = Some(root.make_unique_at(isolated, id)?);
            }
            let model = if let Some(def) = isolated {
                Arc::make_mut(&mut root.definitions.get_mut(&def).unwrap().model)
            } else {
                &mut *root
            };
            apply_faces(model, &faces, &name, &replacement);
            faces.len()
        }
    };
    root.materials.entry(name.clone()).or_insert(replacement);
    Ok(json!({"material": name, "faces": changed}))
}

fn resolve_paint(root: &Model, active: &Model, color: Option<[f32; 3]>, material: Option<String>) -> Result<(String, Material)> {
    if let Some(name) = material {
        let replacement = active.find_material(&name).or_else(|| root.find_material(&name))
            .ok_or_else(|| anyhow::anyhow!("no material {name}"))?.clone();
        Ok((name, replacement))
    } else if let Some(color) = color {
        let name = Model::color_material_name(color);
        let replacement = root.find_material(&name).cloned().unwrap_or(Material { color, ..Default::default() });
        Ok((name, replacement))
    } else {
        bail!("paint needs a color or a material");
    }
}

fn definition_model(root: &Model, id: Option<Id>) -> Result<&Model> {
    match id {
        Some(id) => root.definitions.get(&id).map(|d| &*d.model).ok_or_else(|| anyhow::anyhow!("no component definition {id}")),
        None => Ok(root),
    }
}

fn connected_matching_faces(model: &Model, face: Id, source: Option<&str>, inherited: Option<&str>) -> Vec<Id> {
    let seed = &model.faces[&face];
    let mut by_edge: HashMap<_, Vec<_>> = HashMap::new();
    let edges = |f: &crate::model::Face| f.loops().flat_map(|lp| pairs(lp).map(|(a, b)| (a.min(b), a.max(b)))).collect::<Vec<_>>();
    let group_paint = model.group_material(seed.group).or(inherited);
    for (&id, f) in model.faces.iter().filter(|(_, f)| f.group == seed.group && f.material.as_deref().or(group_paint) == source) {
        for edge in edges(f) {
            by_edge.entry(edge).or_default().push(id);
        }
    }
    let mut seen = BTreeSet::from([face]);
    let mut queue = VecDeque::from([face]);
    while let Some(id) = queue.pop_front() {
        for edge in edges(&model.faces[&id]) {
            for neighbor in by_edge.remove(&edge).unwrap_or_default() {
                if seen.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
    }
    seen.into_iter().collect()
}

fn replace_named(model: &mut Model, source: &str, name: &str, replacement: &Material) -> usize {
    let ids: Vec<_> = model.faces.iter().filter(|(_, f)| f.material.as_deref() == Some(source)).map(|(&id, _)| id)
        .chain(model.groups.iter().filter(|(_, g)| g.material.as_deref() == Some(source)).map(|(&id, _)| id))
        .chain(model.instances.iter().filter(|(_, i)| i.material.as_deref() == Some(source)).map(|(&id, _)| id))
        .collect();
    apply_objects(model, &ids, name, replacement);
    let mut count = ids.len();
    for def in model.definitions.values_mut() {
        if def.model.material_uses(source) > 0 {
            count += replace_named(Arc::make_mut(&mut def.model), source, name, replacement);
        }
    }
    count
}

fn has_locks(model: &Model) -> bool {
    model.groups.values().any(|g| g.locked) || model.definitions.values().any(|d| has_locks(&d.model))
}

fn apply_objects(model: &mut Model, ids: &[Id], name: &str, material: &Material) {
    if ids.is_empty() { return; }
    model.materials.entry(name.to_string()).or_insert_with(|| material.clone());
    for id in ids {
        if let Some(g) = model.groups.get_mut(id) {
            g.material = Some(name.to_string());
        } else if let Some(i) = model.instances.get_mut(id) {
            i.material = Some(name.to_string());
        } else {
            model.faces.get_mut(id).unwrap().set_material(Some(name.to_string()));
        }
    }
}

fn apply_faces(model: &mut Model, faces: &[Id], name: &str, material: &Material) {
    if faces.is_empty() { return; }
    model.materials.entry(name.to_string()).or_insert_with(|| material.clone());
    for id in faces {
        model.faces.get_mut(id).unwrap().set_material(Some(name.to_string()));
    }
}

impl Engine {
    pub(super) fn run_paint(&mut self, cmd: Command) -> Result<Value> {
        if let Command::PaintMatching { face, placement, scope, color, material } = cmd {
            let editing = self.editing_placement();
            let result = paint_matching(&mut self.model, self.ctx, face, placement, scope, color, material)?;
            if let Some(id) = editing {
                self.ctx = self.model.instances.get(&id).map(|i| i.def);
            }
            return Ok(result);
        }
        if let Command::PaintObjects { ids, color, material } = cmd {
            let active = definition_model(&self.model, self.ctx)?;
            active.check_unlocked(&ids)?;
            for id in &ids {
                if !active.faces.contains_key(id) && !active.groups.contains_key(id) && !active.instances.contains_key(id) {
                    bail!("{id} is not a face, group or component");
                }
            }
            let (name, replacement) = resolve_paint(&self.model, active, color, material)?;
            apply_objects(active_model!(self), &ids, &name, &replacement);
            self.model.materials.entry(name.clone()).or_insert(replacement);
            return Ok(json!({"material": name}));
        }
        let shared = if let Command::Paint { material: Some(ref name), .. } = cmd {
            let active = self.ctx.map(|id| &*self.model.definitions[&id].model).unwrap_or(&self.model);
            (!active.materials.contains_key(name)).then(|| self.model.find_material(name).cloned()).flatten()
        } else {
            None
        };
        let m: &mut Model = active_model!(self);
        Ok(match cmd {
            Command::Paint { ids, color, material } => match (material, color) {
                (Some(name), _) => {
                    if let Some(shared) = shared {
                        m.materials.insert(name.clone(), shared);
                    }
                    m.paint_material(&ids, &name)?;
                    json!({"material": name})
                }
                (None, Some(c)) => json!({"material": m.paint(&ids, c)?}),
                _ => bail!("paint needs a color or a material"),
            },
            Command::Texture { name, data, width, height, color, project, origin } => {
                let mat = m.materials.entry(name.clone()).or_default();
                let mut tex = mat.texture.clone().unwrap_or(crate::model::Texture { data: String::new(), width: 1.0, height: 1.0, project: None, origin: None });
                if let Some(d) = data {
                    crate::safety::check_picture(&d).map_err(|e| anyhow::anyhow!("texture {name}: {e}"))?;
                    tex.data = d;
                }
                if tex.data.is_empty() {
                    bail!("a new texture needs image data");
                }
                if let Some(w) = width {
                    tex.height = height.unwrap_or(tex.height * w / tex.width.max(1e-9));
                    tex.width = w;
                } else if let Some(h) = height {
                    tex.height = h;
                }
                if let Some(p) = project {
                    tex.project = p;
                }
                if let Some(o) = origin {
                    tex.origin = Some(o);
                }
                if let Some(c) = color {
                    mat.color = c;
                } else if mat.color == [0.0; 3] {
                    mat.color = [0.8, 0.8, 0.8];
                }
                mat.texture = Some(tex);
                json!({"material": name})
            }
            Command::Purge => json!({"removed": crate::library::purge(m)}),
            Command::DeleteMaterial { name, replace_with } => {
                let faces = m.remove_material(&name, replace_with.as_deref())?;
                json!({"material": name, "faces": faces})
            }
            Command::PurgeMaterials => {
                let (names, bytes) = m.purge_materials();
                json!({"removed": names.len(), "materials": names, "freed_mb": bytes >> 20})
            }
            Command::Skins => {
                let dir = skins_dir();
                let mut out = Vec::new();
                if let Ok(rd) = std::fs::read_dir(&dir) {
                    let mut files: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().and_then(|x| x.to_str()) == Some("sketchskin")).collect();
                    files.sort();
                    for p in files.into_iter().take(200) {
                        // JSON files only, up to 4 MB (a skin can carry an icon set)
                        if std::fs::metadata(&p).map(|m| m.len() > 4 << 20).unwrap_or(true) {
                            continue;
                        }
                        if let Some(skin) = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
                            out.push(json!({"path": p.to_string_lossy(), "name": p.file_stem().map(|s| s.to_string_lossy().to_string()), "skin": skin}));
                        }
                    }
                }
                json!({"dir": dir.to_string_lossy(), "skins": out})
            }
            Command::SaveSkin { name, skin } => {
                let dir = skins_dir();
                std::fs::create_dir_all(&dir)?;
                let path = dir.join(format!("{}.sketchskin", crate::library::safe(&name)));
                std::fs::write(&path, serde_json::to_string_pretty(&skin)?)?;
                json!({"path": path.to_string_lossy()})
            }
            Command::Material { name, color, roughness, metalness, glass, emission } => {
                let mat = m.materials.entry(name.clone()).or_default();
                if let Some(c) = color {
                    mat.color = c;
                }
                let unit = |x: f32| if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.5 };
                if let Some(r) = roughness {
                    mat.roughness = Some(unit(r));
                }
                if let Some(x) = metalness {
                    mat.metalness = Some(unit(x));
                }
                if let Some(g) = glass {
                    mat.glass = Some(g);
                }
                if let Some(e) = emission {
                    mat.emission = Some(if e.is_finite() { e.clamp(0.0, 100.0) } else { 0.0 });
                }
                json!({"material": name})
            }
            other => unreachable!("command routed to the wrong group: {other:?}"),
        })
    }
}
