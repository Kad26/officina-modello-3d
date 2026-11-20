//! Shaping what is drawn: welding, flipping, solids, bevels, slicing, subdivision and 3D text.
//! Arms of `Engine::run`; each works on the model being edited (inside `edit`, the open component's definition).

use super::*;

impl Engine {
    pub(super) fn run_shape(&mut self, cmd: Command) -> Result<Value> {
        let m: &mut Model = active_model!(self);
        Ok(match cmd {
            Command::Weld { edges } => json!({"curve": m.weld(&edges)?}),
            Command::ExplodeCurve { edges } => json!({"edges": m.explode_curve(&edges)}),
            Command::OrientFaces { face } => json!({"flipped": m.orient_faces(face)?}),
            Command::Text3d { rings, at, right, up, depth, name } => {
                // its own group, like SketchUp's 3D Text
                let g = m.begin_group(name.unwrap_or_else(|| "3D Text".into()));
                let r = m.text_3d(&rings, v3(at), v3(right), v3(up), depth);
                m.active_group = m.groups.get(&g).and_then(|g| g.parent);
                let faces = r?;
                json!({"group": g, "faces": faces})
            }
            Command::Subd { name, points, faces, materials, creases, levels, editable, symmetry } => {
                let n = faces.len();
                let mut ed = crate::subd::Editable { points, faces, materials: (0..n).map(|i| materials.get(i).cloned().flatten()).collect(), creases, levels: levels.clamp(0, 5), symmetry };
                if ed.symmetry.is_none() {
                    ed.detect_symmetry();
                }
                let g = crate::subd::build(m, ed.cage(), ed.levels, &name.unwrap_or_else(|| "Smooth surface".into()))?;
                let sym = ed.symmetry;
                if editable {
                    m.cages.insert(g, ed);
                }
                json!({"group": g, "faces": m.faces.values().filter(|f| f.group == Some(g)).count(), "symmetry": sym})
            }
            Command::CageEdit { id, moves, mirror, crease, uncrease, op, levels, symmetry } => {
                m.check_unlocked(&[id])?;
                let Some(mut ed) = m.cages.get(&id).cloned() else { bail!("{id} isn't an editable subdivision object") };
                match symmetry.as_ref() {
                    Some(Value::String(s)) if s == "off" => ed.symmetry = None,
                    Some(Value::String(s)) if s == "auto" => ed.detect_symmetry(),
                    Some(Value::Number(a)) => {
                        let axis = a.as_u64().unwrap_or(1).min(2) as usize;
                        let mid = ed.points.iter().map(|p| p[axis]).fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
                        ed.symmetry = Some(crate::subd::Symmetry { axis, center: (mid.0 + mid.1) / 2.0 });
                    }
                    _ => {}
                }
                if !moves.is_empty() {
                    let mv: Vec<(usize, [f64; 3])> = moves.iter().map(|m| (m[0] as usize, [m[1], m[2], m[3]])).collect();
                    ed.move_points(&mv, mirror)?;
                }
                // creases, mirrored when symmetric
                let partners = ed.mirrors();
                let mirror_edges = |es: &[[usize; 2]]| -> Vec<[usize; 2]> {
                    let mut all = es.to_vec();
                    if mirror {
                        for e in es {
                            if let (Some(Some(a)), Some(Some(b))) = (partners.get(e[0]), partners.get(e[1])) {
                                all.push([*a, *b]);
                            }
                        }
                    }
                    all
                };
                ed.set_creases(&mirror_edges(&crease), true);
                ed.set_creases(&mirror_edges(&uncrease), false);
                let mut out = json!({});
                if let Some(op) = op {
                    match op {
                        CageOp::InsertLoop { edge, t } => out["points"] = json!(ed.insert_loop(edge[0], edge[1], t)?),
                        CageOp::Extrude { faces, distance } => out["faces"] = json!(ed.extrude(&faces, distance)?),
                        CageOp::DeleteFaces { faces } => ed.delete_faces(&faces),
                        CageOp::Merge { points } => ed.merge(&points)?,
                    }
                }
                if let Some(l) = levels {
                    ed.levels = l.clamp(0, 5);
                }
                crate::subd::fill(m, ed.cage(), ed.levels, id)?;
                out["cage_points"] = json!(ed.points.len());
                out["cage_faces"] = json!(ed.faces.len());
                out["symmetry"] = json!(ed.symmetry);
                m.cages.insert(id, ed);
                out
            }
            Command::Subdivide { id, levels, keep_sharp } => {
                let Some(old) = m.groups.get(&id).cloned() else { bail!("{id} is not a group") };
                m.check_unlocked(&[id])?;
                let cage = crate::subd::cage_of(m, id, keep_sharp)?;
                m.delete(&[id])?;
                let prev = m.active_group;
                m.active_group = old.parent;
                let g = crate::subd::build(m, cage, levels, &old.name)?;
                m.active_group = prev;
                if let Some(x) = m.groups.get_mut(&g) {
                    x.parent = old.parent;
                    x.tag = old.tag;
                }
                json!({"group": g, "faces": m.faces.values().filter(|f| f.group == Some(g)).count()})
            }
            Command::FlipEdge { edge } => {
                m.flip_edge(edge)?;
                json!({})
            }
            Command::Bevel { edges, size, segments } => {
                let groups = m.bevel(&edges, size, segments)?;
                json!({"groups": groups, "group": groups.first()})
            }
            Command::Divide { edge, count } => json!({"points": m.divide(edge, count)?}),
            Command::Slice { id, origin, normal } => {
                let groups = m.slice(id, v3(origin), v3(normal))?;
                json!({"groups": groups, "group": groups.first()})
            }
            Command::Intersect { ids, with } => json!({"lines": m.intersect(&ids, with != "selection")?}),
            Command::Flip { ids, axis } => json!({"moved": m.flip(&ids, axis_index(&axis)?)?}),
            Command::Solid { op, ids } => {
                let groups = crate::csg::solid(m, &op, &ids)?;
                json!({"groups": groups, "group": groups.first()})
            }
            // on a scratch copy: checking adds paint entries it doesn't need to keep
            Command::Volume { id } => match crate::csg::solid_info(&mut m.clone(), id) {
                Some(v) => json!({"solid": true, "volume": v}),
                None => json!({"solid": false}),
            },
            Command::SolidInspect { id } => crate::csg::inspect_solid(m, id)?,
            other => unreachable!("command routed to the wrong group: {other:?}"),
        })
    }
}
