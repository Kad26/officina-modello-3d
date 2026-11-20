//! Tags, visibility, locks, names and attributes.
//! Arms of `Engine::run`; each works on the model being edited (inside `edit`, the open component's definition).

use super::*;

impl Engine {
    pub(super) fn run_tags(&mut self, cmd: Command) -> Result<Value> {
        let m: &mut Model = active_model!(self);
        Ok(match cmd {
            Command::Hide { ids } => {
                m.set_hidden(&ids, true)?;
                json!({})
            }
            Command::Unhide { ids } => {
                if ids.is_empty() { m.unhide_all() } else { m.set_hidden(&ids, false)? }
                json!({})
            }
            Command::Tag { ids, tag } => {
                m.set_tag(&ids, tag)?;
                json!({})
            }
            Command::TagVisible { tag, visible } => {
                m.tags.insert(tag, visible);
                json!({})
            }
            Command::TagColor { tag, color } => {
                match color {
                    Some(c) => m.tag_colors.insert(tag, c),
                    None => m.tag_colors.remove(&tag),
                };
                json!({})
            }
            Command::RenameTag { tag, to } => {
                let to = to.trim().to_string();
                if to.is_empty() {
                    bail!("a tag needs a name");
                }
                if let Some(v) = m.tags.remove(&tag) {
                    m.tags.insert(to.clone(), v);
                }
                if let Some(c) = m.tag_colors.remove(&tag) {
                    m.tag_colors.insert(to.clone(), c);
                }
                for g in m.groups.values_mut().filter(|g| g.tag.as_deref() == Some(tag.as_str())) {
                    g.tag = Some(to.clone());
                }
                for i in m.instances.values_mut().filter(|i| i.tag.as_deref() == Some(tag.as_str())) {
                    i.tag = Some(to.clone());
                }
                json!({"tag": to})
            }
            Command::SetParent { ids, parent } => {
                m.set_parent(&ids, parent)?;
                json!({})
            }
            Command::Lock { ids, locked } => {
                for id in &ids {
                    let Some(g) = m.groups.get_mut(id) else { bail!("{id} is not a group") };
                    g.locked = locked;
                }
                json!({})
            }
            Command::Soften { ids, soft, coplanar } => {
                let n = if coplanar { m.soften_coplanar() } else { m.soften(&ids, soft)? };
                json!({"edges": n})
            }
            Command::Rename { id, name } => {
                if let Some(g) = m.groups.get_mut(&id) {
                    g.name = name;
                } else if let Some(i) = m.instances.get_mut(&id) {
                    i.name = name;
                } else if let Some(d) = m.definitions.get_mut(&id) {
                    d.name = name;
                } else {
                    bail!("{id} can't be renamed (groups and components only)");
                }
                json!({})
            }
            Command::Opacity { material, value } => {
                let Some(mat) = m.materials.get_mut(&material) else { bail!("no material {material}") };
                mat.opacity = value.clamp(0.05, 1.0);
                json!({"opacity": mat.opacity})
            }
            Command::SetAttrs { id, attrs } => {
                let target = if let Some(i) = m.instances.get_mut(&id) {
                    &mut i.attrs
                } else if let Some(d) = m.definitions.get_mut(&id) {
                    &mut d.attrs
                } else {
                    bail!("{id} is not a component placement or a component");
                };
                for (k, v) in attrs {
                    let k = k.trim().to_ascii_lowercase();
                    if k.is_empty() || !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                        bail!("attribute names are letters, digits and _ ({k:?})");
                    }
                    if v.trim().is_empty() {
                        target.remove(&k);
                    } else {
                        target.insert(k, v.trim().to_string());
                    }
                }
                // check every formula still works out
                let (inst, def) = match m.instances.get(&id) {
                    Some(i) => (Some(i.clone()), m.definitions.get(&i.def)),
                    None => (None, m.definitions.get(&id)),
                };
                let all = match &inst {
                    Some(i) => crate::dynamic::attrs_of(i, def),
                    None => def.map(|d| d.attrs.clone()).unwrap_or_default(),
                };
                let vals = crate::dynamic::resolve(&all, &Default::default());
                for (k, v) in &all {
                    if v.starts_with('=') && !vals.contains_key(k) && !v.contains("parent!") {
                        let err = crate::dynamic::eval(v, &|n| vals.get(n).copied().ok_or_else(|| anyhow::anyhow!("no attribute {n}"))).err();
                        bail!("{k}: {}", err.map(|e| e.to_string()).unwrap_or_else(|| "can't be worked out".into()));
                    }
                }
                json!({"attrs": all, "values": vals})
            }
            Command::Classify { ids, class } => {
                let class = class.trim().to_string();
                if !class.is_empty() && !crate::ifc::CLASSES.iter().any(|c| c.eq_ignore_ascii_case(&class)) {
                    bail!("unknown IFC type {class:?}; one of {}", crate::ifc::CLASSES.join(", "));
                }
                let class = crate::ifc::CLASSES.iter().find(|c| c.eq_ignore_ascii_case(&class)).map(|c| c.to_string());
                for id in &ids {
                    if !m.groups.contains_key(id) && !m.instances.contains_key(id) {
                        bail!("{id} is not a group or component");
                    }
                    match &class {
                        Some(c) => m.classes.insert(*id, c.clone()),
                        None => m.classes.remove(id),
                    };
                }
                json!({"classified": ids.len()})
            }
            Command::Reveal { path } => {
                let p = crate::safety::path_arg(path.map(std::path::PathBuf::from).unwrap_or_else(crate::library::default_dir));
                std::fs::create_dir_all(if p.extension().is_some() { p.parent().unwrap_or(&p) } else { &p }).ok();
                // folders open; files and app bundles are only shown, never launched
                #[cfg(target_os = "macos")]
                let r = {
                    let bundle = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("app"));
                    if p.is_dir() && !bundle { std::process::Command::new("open").arg(&p).status() } else { std::process::Command::new("open").arg("-R").arg(&p).status() }
                };
                // never `explorer <file>`: that opens (runs) the file. Folders open; a file is only selected.
                #[cfg(target_os = "windows")]
                let r = if p.is_dir() { std::process::Command::new("explorer").arg(&p).status() } else { std::process::Command::new("explorer").arg(format!("/select,{}", p.display())).status() };
                #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
                let r = std::process::Command::new("xdg-open").arg(if p.is_dir() { &p } else { p.parent().unwrap_or(&p) }).status();
                anyhow::Context::context(r, "couldn't open the file browser")?;
                json!({"path": p.to_string_lossy()})
            }
            other => unreachable!("command routed to the wrong group: {other:?}"),
        })
    }
}
