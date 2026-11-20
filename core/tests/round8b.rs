//! Round 8b (Burn1t, R8-BRN-005 and R8-BRN-007): the message when a component is placed while one is open, and the
//! pictures a SketchUp file drops for being unreadable or too big, which are said like every other skipped picture.

mod skp_fixture;
use base64::Engine as _;
use serde_json::{json, Value};
use skp_fixture::*;
use sketchcraft::Engine;

fn exec(e: &mut Engine, v: Value) -> Value {
    e.exec_json(&v.to_string())
}

fn ok(e: &mut Engine, v: Value) -> Value {
    let r = exec(e, v.clone());
    assert_eq!(r["ok"], true, "{v} -> {r}");
    r
}

fn import(e: &mut Engine, skp: &[u8]) -> Value {
    exec(e, json!({"cmd": "import_skp", "data": base64::engine::general_purpose::STANDARD.encode(skp)}))
}

#[test]
fn placing_a_component_while_editing_one_says_what_to_do() {
    let mut e = Engine::new();
    let face = ok(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}))["face"].clone();
    let made = ok(&mut e, json!({"cmd": "make_component", "ids": [face], "name": "Part"}));
    let (inst, def) = (made["instance"].clone(), made["component"].clone());
    let f2 = ok(&mut e, json!({"cmd": "rect", "from": [5, 5, 0], "to": [6, 6, 0]}))["face"].clone();
    let other = ok(&mut e, json!({"cmd": "make_component", "ids": [f2], "name": "Other"}));
    ok(&mut e, json!({"cmd": "edit", "id": inst}));
    for component in [other["component"].clone(), def] {
        let r = exec(&mut e, json!({"cmd": "place", "component": component, "at": [10, 0, 0]}));
        assert_eq!(r["ok"], false, "{r}");
        let why = r["error"].as_str().unwrap();
        assert!(why.starts_with("Close the component you're editing first"), "{why}");
        assert!(!why.contains("no component"), "{why}");
    }
    // closed again, the same placement works
    ok(&mut e, json!({"cmd": "close"}));
    ok(&mut e, json!({"cmd": "place", "component": other["component"].clone(), "at": [10, 0, 0]}));
}

fn skipped(r: &Value) -> String {
    r["skipped_pictures"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" | ")).unwrap_or_default()
}

#[test]
fn a_picture_with_an_unreadable_header_is_reported() {
    let xml = br#"<mat:material name="Junk" colorRed="255" colorGreen="0" colorBlue="0" hasTexture="1"><mat:image path="./tex.png" width="10"/>"#.to_vec();
    let skp = with_material("Junk", xml, b"this is not a picture".to_vec());
    let mut e = Engine::new();
    let r = import(&mut e, &skp);
    assert_eq!(r["ok"], true, "{r}");
    assert!(skipped(&r).contains("Junk"), "{r}");
    assert!(e.model.materials.values().all(|m| m.texture.is_none()));
}

#[test]
fn a_picture_over_64_mb_is_reported() {
    let xml = br#"<mat:material name="Big" colorRed="255" colorGreen="0" colorBlue="0" hasTexture="1"><mat:image path="./tex.png" width="10"/>"#.to_vec();
    let skp = with_material("Big", xml, vec![0u8; 65 << 20]);
    let mut e = Engine::new();
    let r = import(&mut e, &skp);
    assert_eq!(r["ok"], true, "{r}");
    assert!(skipped(&r).contains("Big") && skipped(&r).contains("64 MB"), "{r}");
}

#[test]
fn new_while_editing_a_component_starts_a_whole_new_model() {
    let mut e = Engine::new();
    let face = ok(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}))["face"].clone();
    let made = ok(&mut e, json!({"cmd": "make_component", "ids": [face], "name": "Part"}));
    ok(&mut e, json!({"cmd": "edit", "id": made["instance"].clone()}));
    ok(&mut e, json!({"cmd": "new"}));
    assert!(e.ctx.is_none(), "no component is open in the new model");
    assert!(e.model.definitions.is_empty() && e.model.instances.is_empty() && e.model.faces.is_empty());
    // and it can be drawn in, made into a component and so on
    let f = ok(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}))["face"].clone();
    ok(&mut e, json!({"cmd": "make_component", "ids": [f], "name": "Again"}));
}
