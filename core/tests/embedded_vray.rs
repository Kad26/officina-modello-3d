mod skp_fixture;

use base64::Engine as _;
use serde_json::{json, Value};
use sketchcraft::{model::Model, scene, Engine};
use std::io::{Cursor, Read};

fn record(tag: u16, data: Vec<u8>) -> Vec<u8> {
    [tag.to_le_bytes().as_slice(), &(data.len() as u32).to_le_bytes(), &data].concat()
}

fn dictionary(plugins: &[Value]) -> Vec<u8> {
    let body = plugins.iter().flat_map(|value| [
        record(0x36b6, value["name"].as_str().unwrap().as_bytes().to_vec()),
        record(0x38a4, record(0x38ad, serde_json::to_vec(value).unwrap())),
    ]).flatten().collect();
    let body = [
        record(0x36b0, vec![0; 6]),
        record(0x36b4, b"VRayPlugins".to_vec()),
        record(0x36b5, body),
    ].concat();
    [0x36b3, 0x36b2, 0x36b1, 0x5dd, 0x5dc, 0x7d0].into_iter().fold(body, |bytes, tag| record(tag, bytes))
}

fn insert(bytes: &[u8], path: &[u16], data: Vec<u8>) -> Vec<u8> {
    if path.is_empty() { return [bytes, &data].concat(); }
    let mut at = 0;
    while at < bytes.len() {
        let tag = u16::from_le_bytes(bytes[at..at+2].try_into().unwrap());
        let len = u32::from_le_bytes(bytes[at+2..at+6].try_into().unwrap()) as usize;
        let end = at + 6 + len;
        if tag == path[0] {
            let updated = record(tag, insert(&bytes[at+6..end], &path[1..], data));
            return [&bytes[..at], &updated, &bytes[end..]].concat();
        }
        at = end;
    }
    panic!("missing record {}", path[0]);
}

fn embedded_lights_fixture() -> Vec<u8> {
    let source = skp_fixture::modern_skp();
    let start = source.windows(4).position(|bytes| bytes == b"PK\x03\x04").unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(&source[start..])).unwrap();
    let mut model = Vec::new();
    archive.by_name("model.dat").unwrap().read_to_end(&mut model).unwrap();
    let model = insert(&model, &[0x1f4, 0x1f9, 0x1770, 0x1771, 0x157c, 0x1388], dictionary(&[
        json!({"name":"/rectangle fixture","class":"LightRectangle","params":{
            "u_size":"10","v_size":"20","intensity":"200","directional":"0.58","ignoreLightNormals":"1",
            "transform":"Transform(Matrix(Vector(1,0,0),Vector(0,1,0),Vector(0,0,1)),Vector(20,30,40))",
        }}),
        json!({"name":"/disabled","class":"LightRectangle","params":{"enabled":"0"}}),
    ]));
    let model = insert(&model, &[0x1f4, 0x1f6, 0x1388], dictionary(&[
        json!({"name":"/camera","class":"CameraPhysical","params":{"f_number":"8","ISO":"468.75","shutter_speed":"300","white_balance":"Color(1,0.947828,0.879593)"}}),
        json!({"name":"/sun","class":"SunLight","params":{"enabled":"0"}}),
        json!({"name":"/environment","class":"SettingsEnvironment","params":{"use_gi":"1","gi_tex":"Color(0.8,0.9,1)","gi_tex_mult":"1"}}),
        json!({"name":"/mapping","class":"SettingsColorMapping","params":{"type":"6","adaptation_only":"2","dark_mult":"1","bright_mult":"0.7"}}),
    ]));
    skp_fixture::vff(&model, &[])
}

fn run(engine: &mut Engine, command: Value) -> Value {
    let reply = engine.exec_json(&command.to_string());
    assert_eq!(reply["ok"], true, "{reply}");
    reply
}

#[test]
fn embedded_lights_follow_import_and_group_transforms_and_survive_serialization() {
    let data = base64::engine::general_purpose::STANDARD.encode(embedded_lights_fixture());
    let mut engine = Engine::new();
    let result = run(&mut engine, json!({"cmd":"import_skp","data":data,"at":[1,2,3]}));
    let group = result["group"].as_u64().unwrap();
    assert_eq!(result["vray_lights"], 1);
    let settings = |model: &Model| scene::build(model, None)["embeddedVray"].clone();
    let imported = settings(&engine.model);
    assert_eq!(imported["skipped"]["disabled"], 1);
    assert_eq!(imported["sun_enabled"], false);
    let light = &imported["area_lights"][0];
    let near = |value: &Value, expected: f64| assert!((value.as_f64().unwrap() - expected).abs() < 1e-6, "{value} != {expected}");
    near(&imported["exposure_ev100"], 12.0);
    near(&light["position"][0], 1.762);
    near(&light["position"][1], 2.762);
    near(&light["position"][2], 4.016);
    near(&light["target"][2], 3.016);
    near(&light["width"], 0.508);
    near(&light["height"], 1.016);
    near(&light["intensity"], 10.0);
    near(&light["directionality"], 0.58);
    assert_eq!(light["uniform_emission"], true);
    near(&imported["sky_color"][0], 0.8);
    near(&imported["white_balance"][2], 0.879593);
    near(&imported["reinhard"][0], 1.0);
    near(&imported["reinhard"][1], 0.7);
    let stored = serde_json::to_vec(&engine.model).unwrap();
    let saved: Model = serde_json::from_slice(&stored).unwrap();
    assert_eq!(settings(&saved), imported);
    run(&mut engine, json!({"cmd":"move","ids":[group],"by":[1,-2,0.5]}));
    let moved = settings(&engine.model);
    near(&moved["area_lights"][0]["position"][0], 2.762);
    near(&moved["area_lights"][0]["position"][1], 0.762);
    near(&moved["area_lights"][0]["position"][2], 4.516);
    run(&mut engine, json!({"cmd":"hide","ids":[group]}));
    assert_eq!(settings(&engine.model)["area_lights"], json!([]));
    run(&mut engine, json!({"cmd":"undo"}));
    assert_eq!(settings(&engine.model), moved);
}

#[test]
fn photoreal_uses_embedded_lights_by_default_and_can_disable_them() {
    let data = base64::engine::general_purpose::STANDARD.encode(embedded_lights_fixture());
    let mut engine = Engine::new();
    run(&mut engine, json!({"cmd":"import_skp","data":data}));
    let render = json!({"cmd":"photoreal","width":64,"height":64,"samples":4,"bounces":2,"sun_strength":0,"sky_strength":0,
        "camera":{"eye":[0.6,0.2,3.0],"target":[0.6,0.2,0.0],"fov":48}});
    let lit = run(&mut engine, render.clone());
    let mut dark = render;
    dark["use_embedded_lights"] = json!(false);
    let unlit = run(&mut engine, dark);
    assert_eq!(lit["embedded_lights"], 1);
    assert_eq!(unlit["embedded_lights"], 0);
    assert_ne!(lit["data"], unlit["data"], "imported rectangle changes the rendered image");
}
