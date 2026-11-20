//! What a viewer needs to draw a model: triangles (with the face or component
//! each belongs to, for picking), edges (with ids, for picking lines), face
//! outlines, the outliner tree, tags, guides and saved views. Shared by the
//! desktop app and the browser bridge.

use crate::geom;
use crate::model::Model;
use serde_json::{json, Value};

const DEFAULT_COLOR: [f32; 3] = [0.95, 0.94, 0.91];
const BACK_COLOR: [f32; 3] = [0.65, 0.68, 0.82];

/// `ctx`: a component definition being edited. Its geometry is then listed
/// in `faces` (editable), placed at the first instance's transform.
pub fn build(root: &Model, ctx: Option<u32>) -> Value {
    let mut positions: Vec<f32> = Vec::new();
    let mut colors: Vec<f32> = Vec::new();
    let mut back_colors: Vec<f32> = Vec::new();
    let mut tri_face: Vec<u32> = Vec::new();
    // image materials: texture coordinates per vertex, a texture index per triangle
    let mut uvs: Vec<f32> = Vec::new();
    let mut tri_tex: Vec<i32> = Vec::new();
    // roughness and metalness per triangle, for the physically based style
    let mut pbr: Vec<f32> = Vec::new();
    let mut textures: Vec<Value> = Vec::new();
    let mut tex_index: std::collections::HashMap<String, i32> = std::collections::HashMap::new();
    let mut paint_index = std::collections::HashMap::new();
    let mut paint_materials = Vec::new();
    let mut tri_paint_material = Vec::new();
    let mut paint_paths = Vec::new();
    let mut paint_path_index = std::collections::HashMap::new();
    let mut tri_paint_source = Vec::new();
    // face-me figures are drawn on their own (the viewer turns them to the camera)
    let figs: std::collections::BTreeMap<u32, (glam::DVec3, f64)> = root.face_me_placements().into_iter().map(|(id, p, a)| (id, (p, a))).collect();
    let mut fig_tris: std::collections::BTreeMap<u32, (Vec<f32>, Vec<f32>, Vec<f32>)> = Default::default();
    for f in root.world_faces() {
        if let Some((pivot, _)) = figs.get(&f.owner) {
            let e = fig_tris.entry(f.owner).or_default();
            let color = f.color.unwrap_or(DEFAULT_COLOR);
            for t in geom::triangulate(&f.outer, &f.holes, f.normal) {
                e.1.extend(color);
                for p in t {
                    let q = p - *pivot;
                    e.0.extend([q.x as f32, q.y as f32, q.z as f32]);
                }
            }
            continue;
        }
        let tex = f.material.as_ref().and_then(|name| root.find_material(name).and_then(|m| m.texture.as_ref()).map(|t| (name.clone(), t)));
        let ti = match &tex {
            Some((name, t)) => *tex_index.entry(name.clone()).or_insert_with(|| {
                let _ = t;
                textures.push(json!(name));
                textures.len() as i32 - 1
            }),
            None => -1,
        };
        let paint = f.material.as_ref().map(|name| *paint_index.entry(name.clone()).or_insert_with(|| {
            paint_materials.push(name.clone());
            paint_materials.len() - 1
        }));
        let paint_source = f.paint_source.as_ref().map(|(path, id)| {
            let at = *paint_path_index.entry(path.clone()).or_insert_with(|| {
                paint_paths.push(path.to_vec());
                paint_paths.len() - 1
            });
            (at, *id)
        });
        // tile axes: along the face, or along the projection direction
        let map = tex.as_ref().map(|(name, t)| {
            let transform = root.find_material(name).and_then(|material| material.texture_transform);
            crate::texmap::Mapping::on_face(t, &f, transform)
        });
        let color = f.color.unwrap_or(DEFAULT_COLOR);
        let (rough, metal) = f.material.as_ref().and_then(|n| root.find_material(n).map(|m| m.pbr(n))).unwrap_or((0.7, 0.0));
        // painted faces show paint on both sides; bare backs are blue
        let back = f.color.map(|c| [c[0] * 0.85, c[1] * 0.85, c[2] * 0.85]).unwrap_or(BACK_COLOR);
        for t in geom::triangulate(&f.outer, &f.holes, f.normal) {
            // one color per triangle (the app spreads it to the corners)
            colors.extend(color);
            back_colors.extend(back);
            pbr.extend([rough, metal]);
            for p in t {
                positions.extend([p.x as f32, p.y as f32, p.z as f32]);
                let uv = map.map(|mp| mp.uv(p)).unwrap_or([p.x, p.y]);
                uvs.extend([uv[0] as f32, uv[1] as f32]);
            }
            tri_face.push(f.owner);
            tri_tex.push(ti);
            tri_paint_material.push(paint);
            tri_paint_source.push(paint_source);
        }
    }
    let (mut edges, mut edge_ids, mut soft_edges) = (Vec::<f32>::new(), Vec::<u32>::new(), Vec::<f32>::new());
    for e in root.world_edges() {
        if let Some((pivot, _)) = figs.get(&e.owner) {
            if !e.soft {
                let (a, b) = (e.a - *pivot, e.b - *pivot);
                fig_tris.entry(e.owner).or_default().2.extend([a.x as f32, a.y as f32, a.z as f32, b.x as f32, b.y as f32, b.z as f32]);
            }
            continue;
        }
        let seg = [e.a.x as f32, e.a.y as f32, e.a.z as f32, e.b.x as f32, e.b.y as f32, e.b.z as f32];
        if e.soft {
            soft_edges.extend(seg);
        } else {
            edges.extend(seg);
            edge_ids.push(e.owner);
        }
    }

    // editable faces: top level, or the definition being edited (in world space)
    let (m, t) = match ctx.and_then(|d| root.definitions.get(&d).map(|def| (d, def))) {
        Some((d, def)) => {
            let t = root.instances.values().find(|i| i.def == d).map(|i| i.transform).unwrap_or_default();
            (&*def.model, t)
        }
        None => (root, glam::DMat4::IDENTITY),
    };
    let mirrored = t.determinant() < 0.0;
    let editing_paint = ctx.and_then(|def| root.instances.values().find(|i| i.def == def))
        .and_then(|i| i.material.as_deref().or_else(|| root.group_material(i.parent)));
    let faces: Vec<Value> = m
        .faces
        .iter()
        .map(|(&id, f)| {
            let outer = m.placed_loop(&f.outer, t, mirrored);
            json!({
                "id": id,
                "normal": geom::newell(&outer).to_array(),
                "outer": outer.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
                "group": f.group,
                "area": m.face_area(id),
                "material": f.material,
                "effectiveMaterial": f.material.as_deref().or_else(|| m.group_material(f.group)).or(editing_paint),
                "hidden": f.hidden,
            })
        })
        .collect();
    // when editing a component, its own faces are pickable one by one
    let (mut edit_pos, mut edit_tri) = (Vec::<f32>::new(), Vec::<u32>::new());
    if ctx.is_some() {
        for (&id, f) in &m.faces {
            let tp = |vertices: &[u32]| m.placed_loop(vertices, t, mirrored);
            let outer = tp(&f.outer);
            let holes: Vec<_> = f.holes.iter().map(|h| tp(h)).collect();
            for tri in geom::triangulate(&outer, &holes, geom::newell(&outer)) {
                for p in tri {
                    edit_pos.extend([p.x as f32, p.y as f32, p.z as f32]);
                }
                edit_tri.push(id);
            }
        }
    }
    let edge_list: Vec<Value> = m
        .edges
        .iter()
        .map(|(&id, e)| {
            json!({"id": id, "a": t.transform_point3(m.p(e.a)).to_array(), "b": t.transform_point3(m.p(e.b)).to_array(), "va": e.a, "vb": e.b, "soft": e.soft, "group": e.group, "curve": e.curve})
        })
        .collect();
    let groups: Vec<Value> = root
        .groups
        .iter()
        .map(|(id, g)| json!({"id": id, "name": g.name, "tag": g.tag, "hidden": g.hidden, "parent": g.parent, "locked": g.locked, "material": g.material, "faces": root.faces.values().filter(|f| f.group == Some(*id)).count()}))
        .collect();
    let instances: Vec<Value> = root
        .instances
        .iter()
        .map(|(id, i)| json!({"id": id, "name": i.name, "component": i.def, "tag": i.tag, "hidden": i.hidden, "parent": i.parent, "transform": i.transform.to_cols_array(), "attrs": i.attrs, "material": i.material, "faceMe": root.definitions.get(&i.def).is_some_and(|d| d.face_me)}))
        .collect();
    let components: Vec<Value> = root
        .definitions
        .iter()
        .map(|(id, d)| {
            // placed in the model, and placed inside other components (parts)
            let top = root.instances.values().filter(|i| i.def == *id).count();
            let inside: usize = root.definitions.values().map(|o| o.model.instances.values().filter(|i| i.def == *id).count()).sum();
            json!({"id": id, "name": d.name, "placed": top, "inside": inside, "source": d.source, "cuts": d.cuts})
        })
        .collect();
    // Rounded so big imports stay a few MB of JSON: 0.01 mm positions,
    // 1/1000 colors; texture coordinates only when something is textured.
    let q = |v: &[f32], step: f64| -> Vec<f64> { v.iter().map(|x| (*x as f64 / step).round() * step).map(|x| (x * 1e6).round() / 1e6).collect() };
    let textured = tri_tex.iter().any(|t| *t >= 0);
    // point clouds: what they are and where; the points come by cloud_data
    let clouds: Vec<Value> = root
        .clouds
        .iter()
        .map(|(id, c)| {
            let (lo, hi) = c.bounds().unwrap_or_default();
            json!({"id": id, "name": c.name, "count": c.len(), "colored": !c.colors.is_empty(), "hidden": c.hidden, "version": c.version, "lo": lo.to_array(), "hi": hi.to_array()})
        })
        .collect();
    let mut scene = json!({
        "sections": root.sections,
        "activeSection": root.active_section,
        "axes": root.axes,
        "clouds": clouds,
        "positions": q(&positions, 1e-5),
        "colors": q(&colors, 1e-3),
        "backColors": q(&back_colors, 1e-3),
        "pbr": q(&pbr, 1e-2),
        "uvs": if textured { q(&uvs, 1e-4) } else { Vec::new() },
        "triTex": tri_tex,
        "textures": textures,
        "materials": root.materials.iter().map(|(name, m)| json!({
            "name": name,
            "color": m.color,
            "texture": m.texture.as_ref().map(|t| json!({"data": t.data, "width": t.width, "height": t.height, "project": t.project})),
            "opacity": m.opacity,
            "roughness": m.roughness,
            "metalness": m.metalness,
            "glass": m.glass,
            "emission": m.emission,
        })).collect::<Vec<_>>(),
        "triFace": tri_face,
        // face-me figures: triangles and edges around their own point, and the
        // way they face now (radians, of their -y axis); turn them to the camera
        "figures": fig_tris.iter().map(|(id, (pos, col, edges))| {
            let (p, a) = figs[id];
            json!({"id": id, "pivot": p.to_array(), "facing": a, "positions": pos, "colors": col, "edges": edges})
        }).collect::<Vec<_>>(),
        "faces": faces,
        "edgeList": edge_list,
        "edges": q(&edges, 1e-5),
        "edgeIds": edge_ids,
        "softEdges": soft_edges,
        "groups": groups,
        "instances": instances,
        "components": components,
        "loose": root.faces.values().filter(|f| f.group.is_none()).count(),
        "activeGroup": root.active_group,
        "editing": ctx,
        "editPositions": edit_pos,
        "editTriFace": edit_tri,
        "editTransform": t.to_cols_array(),
        "tags": root.tags,
        "tagColors": root.tag_colors,
        "guides": root.guides.iter().map(|g| json!({"id": g.id, "p": g.p.to_array(), "dir": g.dir.map(|d| d.to_array())})).collect::<Vec<_>>(),
        "views": root.views,
        "timeline": root.timeline,
        "classes": root.classes,
        "location": root.location,
        "environment": root.environment,
        "photoPoints": root.photo_points,
        "cages": root.cages.iter().filter(|(g, _)| root.groups.contains_key(g)).map(|(g, c)| (g.to_string(), c)).collect::<std::collections::BTreeMap<_, _>>(),
        "photoEdges": root.photo_edges,
        "annotations": root.annotations,
        "units": root.units,
    });
    scene["paint"] = json!({
        "paths": paint_paths,
        "triSource": tri_paint_source,
        "materials": paint_materials,
        "triMaterial": tri_paint_material,
    });
    let lighting = crate::skp_lights::world(root);
    if lighting.has_data() {
        scene["embeddedVray"] = json!(lighting);
    }
    scene
}
