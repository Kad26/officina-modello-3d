//! Section planes and cuts.
//! Arms of `Engine::run`; each works on the model being edited (inside `edit`, the open component's definition).

use super::*;

impl Engine {
    pub(super) fn run_sections(&mut self, cmd: Command) -> Result<Value> {
        let m: &mut Model = active_model!(self);
        Ok(match cmd {
            Command::Section { origin, normal, path } => {
                let cut = m.section(v3(origin), v3(normal))?;
                if let Some(p) = &path {
                    let text = if p.to_ascii_lowercase().ends_with(".dxf") { cut.dxf() } else { cut.svg() };
                    std::fs::write(p, text)?;
                }
                // fills as triangles, ready to draw
                let mut fill: Vec<f64> = Vec::new();
                let mut owners: Vec<Id> = Vec::new();
                for (owner, o, h) in &cut.regions {
                    for t in crate::geom::triangulate(o, h, cut.n) {
                        fill.extend(t.iter().flat_map(|p| [p.x, p.y, p.z]));
                        owners.push(*owner);
                    }
                }
                let lines: Vec<f64> = cut.segments.iter().flat_map(|(a, b)| [a.x, a.y, a.z, b.x, b.y, b.z]).collect();
                json!({"regions": cut.regions.len(), "fill": fill, "fillOwners": owners, "lines": lines, "path": path})
            }
            Command::SectionPlane { name, origin, normal } => {
                let n = DVec3::from(normal);
                if !(n.length() > 1e-9) || !crate::model::finite3(&origin) {
                    bail!("a section plane needs a point and a direction");
                }
                let id = m.new_id();
                let name = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("Section {}", m.sections.len() + 1));
                m.sections.push(crate::model::SectionPlane { id, name, origin, normal: n.normalize().to_array() });
                m.active_section = Some(id);
                json!({"section": id})
            }
            Command::SectionEdit { id, name, origin, normal, active, delete } => {
                let i = m.sections.iter().position(|s| s.id == id).ok_or_else(|| anyhow::anyhow!("no section plane {id}"))?;
                if delete {
                    m.sections.remove(i);
                    if m.active_section == Some(id) {
                        m.active_section = None;
                    }
                    return Ok(json!({"deleted": id}));
                }
                let s = &mut m.sections[i];
                if let Some(n) = name.filter(|n| !n.trim().is_empty()) {
                    s.name = n.trim().chars().take(80).collect();
                }
                if let Some(o) = origin.filter(crate::model::finite3) {
                    s.origin = o;
                }
                if let Some(n) = normal.map(DVec3::from).filter(|n| n.length() > 1e-9) {
                    s.normal = n.normalize().to_array();
                }
                match active {
                    Some(true) => m.active_section = Some(id),
                    Some(false) if m.active_section == Some(id) => m.active_section = None,
                    _ => {}
                }
                json!({"section": id})
            }
            Command::SectionOff => {
                m.active_section = None;
                json!({})
            }
            other => unreachable!("command routed to the wrong group: {other:?}"),
        })
    }
}
