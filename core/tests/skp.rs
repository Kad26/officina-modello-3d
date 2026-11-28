//! The SketchUp importer against a tiny hand-built legacy .skp stream: one
//! component definition holding one triangle, the way SketchUp 7 writes it.

mod skp_fixture;
use skp_fixture::*;
use sketchcraft::Engine;
use serde_json::{json, Value};

fn triangle_skp() -> Vec<u8> {
    triangle_skp_saved_by("{7.1.6087}")
}

fn triangle_skp_saved_by(version: &str) -> Vec<u8> {
    let mut w = W(Vec::new());
    w.0.extend([0xFF, 0xFE, 0xFF, 0x0E]);
    for u in "SketchUp Model".encode_utf16() {
        w.u16(u);
    }
    w.ustr(version);
    w.0.extend([0u8; 64]);
    // definition #9's entities: [def + 1][0][count]
    let base = 11u32;
    w.u32(base - 1).u16(0).u32(1);
    // face (class base, object base+1)
    w.class("CFace").header().f64(0.0).f64(0.0).f64(1.0).f64(0.0).u32(1);
    // loop (class base+2, object base+3)
    w.class("CLoop").u32(0x0101_0000);
    // edge use 1 (class base+4, object base+5) with a new edge (class base+6, object base+7)
    w.class("CEdgeUse").u16(0);
    w.class("CEdge").header();
    w.class("CVertex").vertex(0.0, 0.0, 0.0); // class base+8, object base+9
    w.u16(0x8000 | (base + 8) as u16).vertex(100.0, 0.0, 0.0); // object base+10
    w.u16(0); // no curve
    w.u8(0).u16((base + 3) as u16);
    // edge use 2 (base+11), new edge (base+12): ref base+10 -> new vertex (base+13)
    w.u16(0x8000 | (base + 4) as u16).u16(0);
    w.u16(0x8000 | (base + 6) as u16).header();
    w.u16((base + 10) as u16);
    w.u16(0x8000 | (base + 8) as u16).vertex(0.0, 100.0, 0.0);
    w.u16(0);
    w.u8(0).u16((base + 3) as u16);
    // edge use 3 (base+14), new edge (base+15) closing the loop
    w.u16(0x8000 | (base + 4) as u16).u16(0);
    w.u16(0x8000 | (base + 6) as u16).header();
    w.u16((base + 13) as u16).u16((base + 9) as u16).u16(0);
    w.u8(0).u16((base + 3) as u16);
    w.u16(0); // end of loop
    w.u16(0); // back material
    w.0.extend([0u8; 32]);
    w.0
}

fn run(e: &mut Engine, v: Value) -> Value {
    let r = e.exec_json(&v.to_string());
    assert_eq!(r["ok"], true, "{v} -> {r}");
    r
}

#[test]
fn imports_a_hand_built_legacy_skp() {
    let dir = std::env::temp_dir().join(format!("sketchcraft-skp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tri.skp");
    std::fs::write(&path, triangle_skp()).unwrap();
    let mut e = Engine::new();
    let r = run(&mut e, json!({"cmd":"import_skp","path":path.to_string_lossy()}));
    assert_eq!(r["faces"], 1);
    assert_eq!(r["version"], "{7.1.6087}");
    // nothing places the definition, so it's placed once at the origin
    assert_eq!(r["placed"], 1);
    let faces = e.model.world_faces();
    assert_eq!(faces.len(), 1);
    let area = sketchcraft::geom::area(&faces[0].outer);
    let inch = 0.0254;
    assert!((area - 0.5 * (100.0 * inch) * (100.0 * inch)).abs() < 1e-9, "100-inch right triangle, in meters: {area}");
    assert!(faces[0].normal.z > 0.99, "faces up like the stored plane");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn imports_vff_cameras_with_projection_and_leaves_other_views_intact() {
    use base64::Engine as _;
    let mut e = Engine::new();
    let label = "Prospettiva";
    run(&mut e, json!({"cmd":"save_view","name":label,"eye":[0,0,3],"target":[0,0,0],"camera_only":true}));
    let data = base64::engine::general_purpose::STANDARD.encode(modern_skp_with_views());
    let r = run(&mut e, json!({"cmd":"import_skp","data":data,"at":[1,2,3]}));
    assert_eq!(r["views"], 3);
    assert_eq!(r["faces"], 1);
    assert_eq!(r["placed"], 1);
    assert_eq!(e.model.views.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), vec![label, "Prospettiva (2)", "Sezione B–B", "Campo orizzontale"]);
    let show = |e: &mut Engine, name| run(e, json!({"cmd":"show_view","name":name}));
    assert_eq!(show(&mut e, label)["eye"], json!([0.0,0.0,3.0]));
    let perspective = show(&mut e, "Prospettiva (2)");
    let close = |actual: &Value, expected: f64| {
        assert!((actual.as_f64().unwrap() - expected).abs() < 1e-10, "got {actual}, wanted {expected}");
    };
    for (i, coordinate) in [2.27, 0.222, 4.4732].into_iter().enumerate() {
        close(&perspective["eye"][i], coordinate);
    }
    for (i, coordinate) in [1.508, 2.254, 3.0762].into_iter().enumerate() {
        close(&perspective["target"][i], coordinate);
    }
    close(&perspective["up"][0], 0.3 / 0.89_f64.sqrt());
    assert_eq!(perspective["fov"], 42.0);
    assert_eq!(perspective["aspect_ratio"], 1.6);
    assert_eq!(perspective["parallel"], false);
    assert!(perspective.get("ortho_height").is_none());
    let parallel = show(&mut e, "Sezione B–B");
    close(&parallel["eye"][0], -0.905);
    assert_eq!(parallel["parallel"], true);
    close(&parallel["ortho_height"], 1.8288);
    assert!(parallel["style"].is_null(), "the scene has camera metadata only");
    let horizontal = show(&mut e, "Campo orizzontale");
    close(&horizontal["fov"], 2.0 * ((21.0_f64.to_radians()).tan() / 1.6).atan().to_degrees());
    assert_eq!(horizontal["aspect_ratio"], 1.6);
    assert_eq!(e.model.world_faces().len(), 1);

    let saved = serde_json::to_vec(&e.model).unwrap();
    let mut reopened = Engine::new();
    reopened.model = serde_json::from_slice(&saved).unwrap();
    assert_eq!(show(&mut reopened, "Sezione B–B"), parallel);
}

#[test]
fn imports_parent_and_nested_finishes_with_their_physical_texture_size() {
    use base64::Engine as _;
    let mut e = Engine::new();
    let data = base64::engine::general_purpose::STANDARD.encode(modern_skp_with_inherited_finishes());
    run(&mut e, json!({"cmd":"import_skp","data":data}));
    let placement = e.model.instances.values().next().unwrap();
    assert_eq!(placement.material.as_deref(), Some("skp:Copper"));
    let nested = e.model.definitions[&placement.def].model.instances.values().next().unwrap();
    assert_eq!(nested.material.as_deref(), Some("skp:Red"));
    assert!(e.model.definitions[&nested.def].model.faces.values().all(|f| f.material.is_none()));
    assert_eq!(e.model.world_faces()[0].material.as_deref(), Some("skp:Red"));
    let texture = e.model.materials["skp:Red"].texture.as_ref().unwrap();
    assert!((texture.width - 0.8).abs() < 1e-10);
    assert!((texture.height - 2.1).abs() < 1e-10);
}

#[test]
fn materials_share_an_archived_image_and_keep_their_own_tile_sizes() {
    let xml = |path: &str, w: u8, h: u8| {
        format!(r#"<mat:material hasTexture="1"><mat:texture xScale="{w}" yScale="{h}"><mat:images><mat:image id="1"
path="{path}"/></mat:images></mat:texture></mat:material>"#).into_bytes()
    };
    let path = "materials/Original/wood grain.png";
    let file = materials_skp(
        &[(1, "Original".into()), (2, "Variant".into())],
        &[
            ("materials/Original/material.xml", xml("./wood grain.png", 8, 12)),
            ("materials/Variant/material.xml", xml(path, 24, 24)),
            (path, tiny_png()),
        ],
    );
    let parsed = sketchcraft::skp::parse(&file).unwrap();
    let original = &parsed.textures[&1];
    let variant = parsed.textures.get(&2).expect("the variant uses the original material's image");
    assert_eq!(original.data, variant.data);
    assert!((original.width - 8.0 * 0.0254).abs() < 1e-10);
    assert!((original.height - 12.0 * 0.0254).abs() < 1e-10);
    assert!((variant.width - 24.0 * 0.0254).abs() < 1e-10);
    assert!((variant.height - 24.0 * 0.0254).abs() < 1e-10);
}

#[test]
fn imports_positioned_texture_in_meters_through_nested_placements() {
    use base64::Engine as _;
    use glam::DVec3;
    let mut e = Engine::new();
    let data = base64::engine::general_purpose::STANDARD.encode(modern_skp_with_positioned_texture());
    run(&mut e, json!({"cmd":"import_skp","data":data,"at":[1,2,3]}));
    let verify = |model: &sketchcraft::model::Model| {
        let faces = model.world_faces();
        let uv = faces[0].uv.expect("an imported planar mapping");
        let zero = DVec3::new(1.0 + 14.0 * 0.0254, 2.0 + 8.0 * 0.0254, 3.0);
        assert!(uv.at(zero).unwrap().length() < 1e-12);
        let repeats = uv.at(zero + DVec3::new(-2.1, 0.8, 0.0)).unwrap();
        assert!((repeats - glam::DVec2::ONE).length() < 1e-12);
    };
    verify(&e.model);
    verify(&serde_json::from_slice(&serde_json::to_vec(&e.model).unwrap()).unwrap());

    let leaf = e.model.definitions.values().find(|d| d.model.faces.len() == 1).unwrap();
    let mut part = (*leaf.model).clone();
    let face = *part.faces.keys().next().unwrap();
    let point = DVec3::new(0.3, 0.4, 0.0);
    let uv = part.faces[&face].uv.unwrap().at(point).unwrap();
    let move_by = DVec3::new(0.6, 0.8, 0.4);
    let copy = part.duplicate(&[face], move_by).unwrap()[0];
    assert!((part.faces[&copy].uv.unwrap().at(point + move_by).unwrap() - uv).length() < 1e-12);
    part.rotate(&[face], DVec3::ZERO, DVec3::Z, 90.0).unwrap();
    assert!((part.faces[&face].uv.unwrap().at(DVec3::new(-0.4, 0.3, 0.0)).unwrap() - uv).length() < 1e-12);
    part.paint_material(&[face], "skp:Red").unwrap();
    assert!(part.faces[&face].uv.is_some(), "reapplying the same finish preserves the mapping");
    part.paint(&[face], [0.3, 0.4, 0.5]).unwrap();
    assert!(part.faces[&face].uv.is_none(), "repainting starts with the new material's mapping");
}

#[test]
fn rejects_non_sketchup_files() {
    let mut e = Engine::new();
    let r = e.exec_json(&json!({"cmd":"import_skp","data":"aGVsbG8gd29ybGQ="}).to_string());
    assert_eq!(r["ok"], false);
}

#[test]
fn imports_named_wire_profile_as_an_editable_assembly() {
    use base64::Engine as _;
    let mut e = Engine::new();
    let data = base64::engine::general_purpose::STANDARD.encode(wire_profile_skp());
    let r = run(&mut e, json!({"cmd":"import_skp","data":data,"name":"Door drawing"}));
    assert_eq!(r["placed"], 1);
    assert!(e.model.world_faces().is_empty());
    assert_eq!(e.model.world_edges().len(), 3);
    let outer = e.model.instances.values().next().unwrap();
    assert_eq!(outer.parent, Some(r["group"].as_u64().unwrap() as u32));
    assert_eq!(outer.name, "Outer...");
    let profile = e.model.definitions[&outer.def].model.instances.values().next().unwrap();
    assert_eq!(profile.name, "Door profile");
    assert_eq!(e.model.definitions[&profile.def].name, "Door profile");
}

// ---- modern (SketchUp 2013+) files: header + "VFF" + ZIP with model.dat ----

#[test]
fn imports_a_modern_skp() {
    use base64::Engine as _;
    let mut e = Engine::new();
    let data = base64::engine::general_purpose::STANDARD.encode(modern_skp());
    let r = run(&mut e, json!({"cmd":"import_skp","data":data}));
    assert_eq!(r["version"], "{26.2.0}");
    assert_eq!(r["faces"], 1);
    assert_eq!(r["placed"], 1);
    let faces = e.model.world_faces();
    assert_eq!(faces.len(), 1, "the face inside the nested component draws");
    let inch = 0.0254;
    let area = sketchcraft::geom::area(&faces[0].outer);
    assert!((area - 0.5 * (100.0 * inch) * (100.0 * inch)).abs() < 1e-9, "{area}");
    let min_x = faces[0].outer.iter().map(|p| p.x).fold(f64::MAX, f64::min);
    assert!((min_x - 10.0 * inch).abs() < 1e-9, "placed by the instance transform: {min_x}");
    assert_eq!(faces[0].color, Some([1.0, 0.0, 0.0]), "color from material.xml");
}

/// SketchUp 27 names a placement's definition by id, not GUID (issue #1:
/// files loaded their components but placed none, so nothing showed).
#[test]
fn imports_a_sketchup_27_skp() {
    use base64::Engine as _;
    let mut e = Engine::new();
    let data = base64::engine::general_purpose::STANDARD.encode(skp_fixture::su27_skp());
    let r = run(&mut e, json!({"cmd":"import_skp","data":data}));
    assert_eq!(r["version"], "{27.0.0}");
    assert_eq!(r["placed"], 1, "{r}");
    let faces = e.model.world_faces();
    assert_eq!(faces.len(), 1, "the face inside the nested component draws");
    let min_x = faces[0].outer.iter().map(|p| p.x).fold(f64::MAX, f64::min);
    assert!((min_x - 10.0 * 0.0254).abs() < 1e-9, "placed by the instance transform: {min_x}");
}

/// Files from SketchUp 2013-2020 use the classic layout, not the 2021+ ZIP
/// (issue #1: they were refused as "not a SketchUp 2013+ archive").
#[test]
fn a_2015_file_goes_to_the_classic_reader() {
    use base64::Engine as _;
    let mut e = Engine::new();
    let data = base64::engine::general_purpose::STANDARD.encode(triangle_skp_saved_by("{15.0.0}"));
    let r = run(&mut e, json!({"cmd":"import_skp","data":data}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["version"], "{15.0.0}");
    assert_eq!(e.model.world_faces().len(), 1);
}

/// SketchUp 2017-2020 files add an id to every entity and hang dictionaries
/// (attributes, texture mappings) off the entity itself; the classic reader
/// used to find no geometry in them (issue #1).
#[test]
fn imports_a_2017_skp() {
    use base64::Engine as _;
    let mut e = Engine::new();
    let data = base64::engine::general_purpose::STANDARD.encode(classic_2017_skp());
    let r = run(&mut e, json!({"cmd":"import_skp","data":data}));
    assert_eq!(r["version"], "{17.0.1}");
    assert_eq!(r["faces"], 1, "{r}");
    assert_eq!(r["placed"], 1, "{r}");
    let faces = e.model.world_faces();
    assert_eq!(faces.len(), 1);
    let inch = 0.0254;
    let area = sketchcraft::geom::area(&faces[0].outer);
    assert!((area - 0.5 * (100.0 * inch) * (100.0 * inch)).abs() < 1e-9, "{area}");
    let min_x = faces[0].outer.iter().map(|p| p.x).fold(f64::MAX, f64::min);
    assert!((min_x - 50.0 * inch).abs() < 1e-9, "placed by the instance transform: {min_x}");
}

/// A truncated or scrambled 2017 file is an error or a smaller model, never a panic or hang.
#[test]
fn a_damaged_2017_skp_does_not_panic() {
    use base64::Engine as _;
    let good = classic_2017_skp();
    for cut in (40..good.len()).step_by(7) {
        let mut e = Engine::new();
        let data = base64::engine::general_purpose::STANDARD.encode(&good[..cut]);
        let _ = e.exec_json(&json!({"cmd":"import_skp","data":data}).to_string());
    }
    for i in (60..good.len()).step_by(5) {
        let mut bad = good.clone();
        bad[i] ^= 0xA5;
        let mut e = Engine::new();
        let data = base64::engine::general_purpose::STANDARD.encode(&bad);
        let _ = e.exec_json(&json!({"cmd":"import_skp","data":data}).to_string());
    }
}

/// Pictures (images) are placements too, under 0x1390/0x1f40.
#[test]
fn a_picture_in_a_modern_skp_is_placed() {
    use base64::Engine as _;
    let mut e = Engine::new();
    let data = base64::engine::general_purpose::STANDARD.encode(su27_picture_skp());
    let r = run(&mut e, json!({"cmd":"import_skp","data":data}));
    assert_eq!(r["placed"], 1, "{r}");
    assert_eq!(e.model.world_faces().len(), 1);
}

/// When the model's own list draws nothing, the definitions nothing else
/// places are shown rather than leaving an empty model.
#[test]
fn definitions_show_when_the_model_draws_nothing() {
    use base64::Engine as _;
    let mut e = Engine::new();
    let data = base64::engine::general_purpose::STANDARD.encode(only_definitions_skp());
    let r = run(&mut e, json!({"cmd":"import_skp","data":data}));
    assert_eq!(r["placed"], 1, "{r}");
    assert_eq!(e.model.world_faces().len(), 1, "{r}");
}

/// A damaged classic file is refused quickly (R8-BRN-001): the look-ahead that skips an item it can't read is bounded by
/// the work done, not by a 4 to 30 second deadline, so one damaged file never makes the app look hung.
#[test]
fn a_damaged_classic_skp_is_refused_or_read_within_a_second() {
    use base64::Engine as _;
    let seeds = [triangle_skp(), classic_2017_skp()];
    let mut x = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let mut slowest = std::time::Duration::ZERO;
    for good in &seeds {
        // a bigger file made of the same stuff over and over, each copy damaged a little differently: the kind of
        // thing that sends the reader looking ahead at every list
        let head = 0x20 + 64;
        let mut big = good[..good.len().min(head)].to_vec();
        for _ in 0..60 {
            let mut part = good[good.len().min(head)..].to_vec();
            for _ in 0..3 {
                let i = (next() as usize) % part.len().max(1);
                part[i] = next() as u8;
            }
            big.extend(part);
        }
        for bytes in [big, good.clone()] {
            for round in 0..40 {
                let mut bad = bytes.clone();
                for _ in 0..1 + round % 4 {
                    let i = 40 + (next() as usize) % (bad.len() - 40);
                    bad[i] = next() as u8;
                }
                let mut e = Engine::new();
                let data = base64::engine::general_purpose::STANDARD.encode(&bad);
                let t = std::time::Instant::now();
                let _ = e.exec_json(&json!({"cmd":"import_skp","data":data}).to_string());
                slowest = slowest.max(t.elapsed());
            }
        }
    }
    assert!(slowest < std::time::Duration::from_secs(1), "the slowest damaged file took {slowest:?}");
}
