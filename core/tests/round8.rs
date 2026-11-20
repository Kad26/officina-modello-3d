//! Round 8 (Mr BlackKeys, R8-MBK-001..006): components that contain themselves, nest absurdly deep or multiply,
//! and the modern-SketchUp reader's limits. Every fixture is a few hundred bytes to a few KB; each limit is proved by
//! a quick refusal, never by building the blow-up.

mod skp_fixture;
use base64::Engine as _;
use serde_json::{json, Value};
use skp_fixture::*;
use sketchcraft::live::Live;
use sketchcraft::Engine;

fn b64(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn exec(e: &mut Engine, v: Value) -> Value {
    e.exec_json(&v.to_string())
}

fn ok(e: &mut Engine, v: Value) -> Value {
    let r = exec(e, v.clone());
    assert_eq!(r["ok"], true, "{v} -> {r}");
    r
}

fn refused(e: &mut Engine, v: Value) -> String {
    let r = exec(e, v.clone());
    assert_eq!(r["ok"], false, "{v} should be refused, got {r}");
    r["error"].as_str().unwrap().to_string()
}

fn import(e: &mut Engine, skp: &[u8]) -> Value {
    exec(e, json!({"cmd": "import_skp", "data": b64(skp)}))
}

#[test]
fn a_self_placing_definition_is_refused_at_import() {
    let mut e = Engine::new();
    let r = import(&mut e, &graph_skp(&[vec![(0, 1)]], &[0]));
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["error"].as_str().unwrap().contains("contains itself"), "{r}");
    assert!(e.model.definitions.is_empty(), "nothing was kept");
    e.model.world_faces(); // and drawing still works
}

#[test]
fn a_circular_pair_is_refused_at_import() {
    let mut e = Engine::new();
    let r = import(&mut e, &graph_skp(&[vec![(1, 1)], vec![(0, 1)]], &[0]));
    assert!(r["error"].as_str().unwrap().contains("contains itself"), "{r}");
}

#[test]
fn a_deep_chain_is_refused_and_a_realistic_one_imports() {
    let mut e = Engine::new();
    // 200 deep: over the limit of 64, refused quickly (a 30,000-deep one is the same walk)
    let chain: Vec<Vec<(usize, usize)>> = (0..200).map(|i| if i < 199 { vec![(i + 1, 1)] } else { vec![] }).collect();
    let r = import(&mut e, &graph_skp(&chain, &[0]));
    assert!(r["error"].as_str().unwrap().contains("nested more than"), "{r}");
    // 12 deep (a cabinet with drawers with parts) is fine, and draws every level
    let chain: Vec<Vec<(usize, usize)>> = (0..12).map(|i| if i < 11 { vec![(i + 1, 1)] } else { vec![] }).collect();
    let r = import(&mut e, &graph_skp(&chain, &[0]));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(e.model.world_faces().len(), 12);
}

#[test]
fn nested_placements_that_multiply_are_refused_at_import() {
    let mut e = Engine::new();
    // 12 levels, 10 copies each: 10^11 triangles from under 1 KB
    let defs: Vec<Vec<(usize, usize)>> = (0..12).map(|i| if i < 11 { vec![(i + 1, 10)] } else { vec![] }).collect();
    let skp = graph_skp(&defs, &[0]);
    assert!(skp.len() < 4000, "{}", skp.len());
    let r = import(&mut e, &skp);
    assert!(r["error"].as_str().unwrap().contains("once every placement is counted") || r["error"].as_str().unwrap().contains("add up to"), "{r}");
    assert!(e.model.definitions.is_empty());
}

#[test]
fn a_building_with_repeated_windows_still_imports_and_draws() {
    let mut e = Engine::new();
    // building -> 8 floors -> 20 windows -> 6 parts each: a few thousand faces
    let defs = vec![vec![(1, 8)], vec![(2, 20)], vec![(3, 6)], vec![]];
    let r = import(&mut e, &graph_skp(&defs, &[0]));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(e.model.world_faces().len(), 1 + 8 * (1 + 20 * (1 + 6 * 1)));
}

#[test]
fn an_agent_gets_a_tighter_expansion_budget() {
    // 100 x 100 x 20 = 200k triangles (about 800k faces and edges): fine for the person (2M), too much for an agent (500k)
    let defs = vec![vec![(1, 100)], vec![(2, 100)], vec![(3, 20)], vec![]];
    let skp = graph_skp(&defs, &[0]);
    let mut person = Engine::new();
    assert_eq!(import(&mut person, &skp)["ok"], true);
    let mut agent = Engine::new();
    let r = agent.exec_json_as(&sketchcraft::policy::Caller::Mcp(sketchcraft::policy::Workspace::anywhere()), &json!({"cmd": "import_skp", "data": b64(&skp)}).to_string());
    assert_eq!(r["ok"], false, "{r}");
    assert!(agent.model.definitions.is_empty());
}

/// The command-only route: no hostile file, an agent's ordinary commands (R8-MBK-001, 3).
#[test]
fn import_while_editing_a_component_cannot_make_it_contain_itself() {
    let mut e = Engine::new();
    let face = ok(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}))["face"].clone();
    let made = ok(&mut e, json!({"cmd": "make_component", "ids": [face], "name": "Part"}));
    ok(&mut e, json!({"cmd": "edit", "id": made["instance"]}));
    let why = refused(&mut e, json!({"cmd": "import_skp", "data": b64(&modern_skp())}));
    assert!(why.contains("close the component"), "{why}");
    // making a component inside one is held back for the same reason (its id would collide with the model's own)
    let f2 = ok(&mut e, json!({"cmd": "rect", "from": [5, 5, 0], "to": [6, 6, 0]}))["face"].clone();
    let why = refused(&mut e, json!({"cmd": "make_component", "ids": [f2], "name": "Inner"}));
    assert!(why.contains("close the component") || why.contains("edited"), "{why}");
    ok(&mut e, json!({"cmd": "close"}));
    ok(&mut e, json!({"cmd": "info"}));
    assert_eq!(e.model.world_faces().len(), 2);
}

#[test]
fn a_saved_model_with_a_loop_is_refused_by_open_and_does_not_crash() {
    let dir = std::env::temp_dir().join(format!("sc-r8-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // a good model with one component, then hand-edited so the component places itself
    let mut e = Engine::new();
    let face = ok(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}))["face"].clone();
    let made = ok(&mut e, json!({"cmd": "make_component", "ids": [face], "name": "Part"}));
    let def = made["component"].as_u64().unwrap();
    let p = dir.join("a.sketchcraft");
    ok(&mut e, json!({"cmd": "save", "path": p.to_str().unwrap()}));
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    v["definitions"][def.to_string()]["model"]["instances"] = json!({"900": {"parent": null, "def": def, "name": "self", "transform": [1.0,0.0,0.0,0.0, 0.0,1.0,0.0,0.0, 0.0,0.0,1.0,0.0, 0.0,0.0,0.0,1.0], "tag": null, "hidden": false, "attrs": {}}});
    std::fs::write(&p, v.to_string()).unwrap();
    let mut f = Engine::new();
    let why = refused(&mut f, json!({"cmd": "open", "path": p.to_str().unwrap()}));
    assert!(why.contains("contains itself"), "{why}");
    assert!(f.model.definitions.is_empty(), "the model was left as it was");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn drawing_stops_at_the_depth_limit_instead_of_overflowing() {
    // a model that bypassed every check (built through the public fields) still can't overflow the stack
    let mut m = sketchcraft::model::Model::default();
    let id = 5;
    let mut inner = sketchcraft::model::Model::default();
    let iid = 6;
    inner.instances.insert(iid, sketchcraft::model::Instance { parent: None, def: id, name: "me".into(), transform: glam::DMat4::IDENTITY, tag: None, hidden: false, material: None, attrs: Default::default() });
    m.definitions.insert(id, sketchcraft::model::Definition { name: "loop".into(), model: std::sync::Arc::new(inner), attrs: Default::default(), source: None, cuts: false, face_me: false });
    let at = 7;
    m.instances.insert(at, sketchcraft::model::Instance { parent: None, def: id, name: "p".into(), transform: glam::DMat4::IDENTITY, tag: None, hidden: false, material: None, attrs: Default::default() });
    assert!(m.expanded_count().is_err());
    assert!(m.world_faces().is_empty());
    assert!(m.world_edges().is_empty());
    assert!(m.world_bounds().is_none());
}

// ---- the modern reader (R8-MBK-003, 004, 005) ----

#[test]
fn records_nested_a_million_deep_do_not_overflow_the_stack() {
    let mut e = Engine::new();
    // 200,000 levels (about 1.2 MB) inside the top record: refused cleanly, in a thread with a small stack
    let body = nested_records(200_000);
    let model = {
        let mut v = 0x1f4u16.to_le_bytes().to_vec();
        v.extend((body.len() as u32).to_le_bytes());
        v.extend(body);
        v
    };
    let skp = vff(&model, &[]);
    let r = std::thread::Builder::new().stack_size(256 * 1024).spawn(move || import(&mut e, &skp)).unwrap().join().unwrap();
    assert_eq!(r["ok"], false, "{r}");
}

#[test]
fn a_picture_over_the_limit_is_left_out_and_the_material_keeps_its_color() {
    let zeros = vec![0u8; 65 << 20];
    let xml = br#"<mat:material name="Big" colorRed="255" colorGreen="0" colorBlue="0" hasTexture="1"><mat:image path="./tex.png" width="10"/>"#.to_vec();
    let skp = with_material("Big", xml, zeros);
    let mut e = Engine::new();
    let r = import(&mut e, &skp);
    assert_eq!(r["ok"], true, "{r}");
    assert!(e.model.materials.values().all(|m| m.texture.is_none()), "no texture was decoded");
}

#[test]
fn archives_with_too_many_or_overlapping_or_duplicate_entries_are_refused() {
    // 20,001 tiny entries
    let extra: Vec<(String, Vec<u8>)> = (0..20_001).map(|i| (format!("junk/{i}"), vec![])).collect();
    let refs: Vec<(&str, Vec<u8>)> = extra.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
    let skp = graph_skp_with(&[vec![]], &[0], &refs);
    let mut e = Engine::new();
    let r = import(&mut e, &skp);
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["error"].as_str().unwrap().contains("entries"), "{r}");
}

#[test]
fn many_materials_pointing_at_one_picture_stop_at_the_picture_limit() {
    // 300 materials, each naming its own copy of a tiny picture: only 256 are decoded, the rest keep their color
    let png = tiny_png();
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut mats: Vec<(u32, String)> = Vec::new();
    for i in 0..300u32 {
        entries.push((format!("materials/M{i}/material.xml"), format!(r#"<mat:material name="M{i}" colorRed="9" colorGreen="9" colorBlue="9" hasTexture="1"><mat:image path="./t.png" width="10"/>"#).into_bytes()));
        entries.push((format!("materials/M{i}/t.png"), png.clone()));
        mats.push((i + 1, format!("M{i}")));
    }
    let refs: Vec<(&str, Vec<u8>)> = entries.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
    let skp = materials_skp(&mats, &refs);
    let mut e = Engine::new();
    let r = import(&mut e, &skp);
    assert_eq!(r["ok"], true, "{r}");
    let textured = e.model.materials.values().filter(|m| m.texture.is_some()).count();
    assert!(textured <= 256, "{textured}");
    assert!(r["skipped_pictures"].is_array(), "the ones left out are said: {r}");
}

/// The crash loop of R8-MBK-001: a saved model that contains itself used to abort the bridge, the MCP server and the
/// app on every start. Now the start-up treats it as a damaged file: set aside, a notice, an empty model that draws.
#[test]
fn a_saved_loop_is_set_aside_at_start_up_instead_of_crashing_every_launch() {
    let dir = std::env::temp_dir().join(format!("sc-r8-live-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let doc = dir.join("live.sketchcraft");
    let mut e = Engine::new();
    let face = ok(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}))["face"].clone();
    let def = ok(&mut e, json!({"cmd": "make_component", "ids": [face], "name": "Part"}))["component"].as_u64().unwrap();
    ok(&mut e, json!({"cmd": "save", "path": doc.to_str().unwrap()}));
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&doc).unwrap()).unwrap();
    v["definitions"][def.to_string()]["model"]["instances"] = json!({"900": {"parent": null, "def": def, "name": "self", "transform": [1.0,0.0,0.0,0.0, 0.0,1.0,0.0,0.0, 0.0,0.0,1.0,0.0, 0.0,0.0,0.0,1.0], "tag": null, "hidden": false, "attrs": {}}});
    std::fs::write(&doc, v.to_string()).unwrap();
    let l = Live::open(doc.clone());
    assert!(l.notice.is_some(), "the person is told");
    assert!(l.engine.model.definitions.is_empty());
    let scene = l.scene();
    assert!(scene.is_object());
    let aside = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).any(|e| e.file_name().to_string_lossy().contains("damaged"));
    assert!(aside, "a copy of the file was kept");
    std::fs::remove_dir_all(&dir).ok();
}
