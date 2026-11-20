//! Round 6 (security-reports/R6-*.md, fixes in RED-TEAM-REMEDIATION.md): the whole-model byte budget, ids and component
//! definitions on Open and Restore, image limits, redo across a second foreign save, the float tolerance, the side-file
//! index, a file that can't be read at start, and a mutation fuzz of model files.
use serde_json::{json, Value};
use sketchcraft::live::Live;
use sketchcraft::model::Model;
use sketchcraft::policy::{Caller, Workspace};
use sketchcraft::Engine;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sc-round6-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn pause() {
    std::thread::sleep(Duration::from_millis(1100));
}

fn ok(l: &mut Live, v: Value) -> Value {
    let r = l.exec(&v);
    assert_eq!(r["ok"], true, "{v} -> {r}");
    r
}

fn eng(e: &mut Engine, v: Value) -> Value {
    let r = e.exec_json(&v.to_string());
    assert_eq!(r["ok"], true, "{v} -> {r}");
    r
}

fn agent() -> Caller {
    Caller::Mcp(Workspace::anywhere())
}

fn as_agent(e: &mut Engine, v: Value) -> Value {
    e.exec_json_as(&agent(), &v.to_string())
}

/// A small model with a group, a colour, a component and a placement of it.
fn sample() -> Engine {
    let mut e = Engine::new();
    let f = eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}))["face"].as_u64().unwrap();
    eng(&mut e, json!({"cmd": "paint", "ids": [f], "color": [1.0, 0.0, 0.0]}));
    let g = eng(&mut e, json!({"cmd": "group", "ids": [f]}))["group"].as_u64().unwrap();
    let c = eng(&mut e, json!({"cmd": "make_component", "ids": [g], "name": "Thing"}))["component"].as_u64().unwrap();
    eng(&mut e, json!({"cmd": "place", "component": c, "at": [5, 5, 0]}));
    let f2 = eng(&mut e, json!({"cmd": "rect", "from": [10, 0, 0], "to": [12, 2, 0]}))["face"].as_u64().unwrap();
    eng(&mut e, json!({"cmd": "group", "ids": [f2]}));
    e
}

// ---- R6-BRN-001 / R6-MBK-001: the byte budget counts every string and map --------------------------------

#[test]
fn an_agent_cannot_grow_the_model_through_attributes() {
    let mut e = sample();
    let c = *e.model.definitions.keys().next().unwrap();
    let before = e.model.size_estimate();
    let mut refused = None;
    for round in 0..40 {
        // 100 new keys of 60 KB each, new names every time (the flood of the report)
        let attrs: serde_json::Map<String, Value> = (0..100).map(|k| (format!("k{round}_{k}"), json!("v".repeat(60_000)))).collect();
        let r = as_agent(&mut e, json!({"cmd": "set_attrs", "id": c, "attrs": attrs}));
        if r["ok"] != true {
            refused = Some((round, r));
            break;
        }
    }
    let (round, r) = refused.expect("40 x 6 MB of attributes must not all be accepted");
    assert!(round < 4, "stopped at round {round}: {r}");
    assert!(e.model.size_estimate() < before + (16 << 20), "{}", e.model.size_estimate());
    assert!(e.model.text_bytes() <= sketchcraft::model::MAX_TEXT_BYTES);
}

#[test]
fn objects_in_a_command_have_a_key_count_and_a_key_length_limit() {
    let mut e = sample();
    let c = *e.model.definitions.keys().next().unwrap();
    let many: serde_json::Map<String, Value> = (0..1200).map(|k| (format!("a{k}"), json!("x"))).collect();
    let r = as_agent(&mut e, json!({"cmd": "set_attrs", "id": c, "attrs": many}));
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["error"].as_str().unwrap().contains("too many"), "{r}");
    let r = as_agent(&mut e, json!({"cmd": "set_attrs", "id": c, "attrs": {"k".repeat(300): "x"}}));
    assert_eq!(r["ok"], false, "{r}");
    // a handful of ordinary attributes are fine, and the person is held only by the model's own limits
    let r = as_agent(&mut e, json!({"cmd": "set_attrs", "id": c, "attrs": {"lenx": "2", "leny": "3"}}));
    assert_eq!(r["ok"], true, "{r}");
    // 300 attributes on one component is over the per-component limit, for anyone
    let lots: serde_json::Map<String, Value> = (0..300).map(|k| (format!("a{k}"), json!("x"))).collect();
    let r = e.exec_json(&json!({"cmd": "set_attrs", "id": c, "attrs": lots}).to_string());
    assert_eq!(r["ok"], false, "{r}");
}

#[test]
fn the_estimate_is_never_below_the_real_file_and_not_far_above_it() {
    let mut e = sample();
    let c = *e.model.definitions.keys().next().unwrap();
    eng(&mut e, json!({"cmd": "set_attrs", "id": c, "attrs": {"lenx": "2", "note": "hello \"quoted\" text"}}));
    eng(&mut e, json!({"cmd": "text", "at": [0, 0, 0], "text": "a label"}));
    eng(&mut e, json!({"cmd": "save_view", "name": "Front", "eye": [1, 2, 3], "target": [0, 0, 0]}));
    eng(&mut e, json!({"cmd": "terrain_grid", "at": [0, 0, 0], "width": 30, "depth": 30, "spacing": 1}));
    for m in [Model::default(), e.model.clone()] {
        let real = serde_json::to_vec(&m).unwrap().len();
        let est = m.size_estimate();
        assert!(est >= real, "estimate {est} is below the real size {real}");
        assert!(est <= real * 5 / 2 + 2048, "estimate {est} is far above the real size {real}");
    }
}

#[test]
fn a_model_the_program_saved_always_reopens() {
    // the biggest model the budget allows is far under the open limit
    assert!((sketchcraft::model::MAX_MODEL_BYTES as u64) * 3 / 2 <= sketchcraft::commands::MAX_OPEN_BYTES);
    // an old model over the budget opens for cutting down and Restore takes it too
    let mut big = Model::default();
    big.materials.insert("big".into(), serde_json::from_value(json!({"color": [1, 1, 1], "texture": {"data": "x".repeat(sketchcraft::model::MAX_MODEL_BYTES + 10), "width": 1, "height": 1}})).unwrap());
    let mut e = Engine::new();
    eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}));
    e.replace_model(big).expect("Restore is not blocked by the size limit");
    assert!(e.oversize);
    // it can only get smaller: more is refused, taking the image away works
    assert_eq!(e.exec_json(&json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}).to_string())["ok"], false);
    let r = e.exec_json(&json!({"cmd": "purge"}).to_string());
    assert_eq!(r["ok"], true, "{r}");
}

#[test]
fn a_file_over_the_old_image_budget_opens() {
    // 300 MB of image data in a file: the old budget (256 MB) refused it with a message about a step nobody could take
    let dir = scratch("bigopen");
    let doc = dir.join("big.sketchcraft");
    let mut m = Model::default();
    m.materials.insert("photo".into(), serde_json::from_value(json!({"color": [1, 1, 1], "texture": {"data": format!("data:image/png;base64,{}", "A".repeat(300 << 20)), "width": 1, "height": 1}})).unwrap());
    std::fs::write(&doc, serde_json::to_vec(&m).unwrap()).unwrap();
    drop(m);
    let mut e = Engine::new();
    let r = e.exec_json(&json!({"cmd": "open", "path": doc}).to_string());
    assert_eq!(r["ok"], true, "{r}");
    assert!(!e.oversize, "300 MB is inside the person's budget");
    std::fs::remove_dir_all(&dir).ok();
}

// ---- R6-MBK-003: pictures are measured before they are decoded ---------------------------------------------

#[test]
fn a_huge_picture_in_a_tiny_file_is_refused_before_decoding() {
    use base64::Engine as _;
    // an all-black grey PNG of 12,000 x 12,000 is about 20 KB
    let mut png = vec![];
    image::GrayImage::new(12_000, 12_000).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    assert!(png.len() < 2_000_000, "{}", png.len());
    let err = sketchcraft::safety::decode_image(&png, None).unwrap_err().to_string();
    assert!(err.contains("12000 x 12000"), "{err}");
    // and a render of a model that holds it doesn't decode it either (no texture, no crash)
    let mut e = sample();
    let data = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&png));
    // (round 7: the size is judged when the picture is stored, so the agent can't put it in; a file from outside can still hold one)
    let r = as_agent(&mut e, json!({"cmd": "texture", "name": "huge", "data": data.clone()}));
    assert_eq!(r["ok"], false, "{r}");
    e.model.materials.insert("huge".into(), serde_json::from_value(json!({"color": [1, 1, 1], "texture": {"data": data, "width": 1, "height": 1}})).unwrap());
    let imgs = sketchcraft::texmap::Images::load(&e.model);
    assert!(imgs.get("huge").is_none());
    // an ordinary picture still decodes
    let mut small = vec![];
    image::RgbImage::new(64, 64).write_to(&mut std::io::Cursor::new(&mut small), image::ImageFormat::Png).unwrap();
    assert!(sketchcraft::safety::decode_image(&small, None).is_ok());
}

// ---- R6-MBK-001: library files are size-checked ---------------------------------------------------------

#[test]
fn library_loads_refuse_files_over_the_open_limit() {
    let dir = scratch("libsize");
    let p = dir.join("huge.sketchcraft");
    let f = std::fs::File::create(&p).unwrap();
    f.set_len(sketchcraft::commands::MAX_OPEN_BYTES + 1).unwrap(); // sparse: no disk used
    let err = sketchcraft::library::load_file(p.to_str().unwrap()).unwrap_err().to_string();
    assert!(err.contains("MB"), "{err}");
    assert!(sketchcraft::library::thumbnail_of_file(p.to_str().unwrap(), 64).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

// ---- R6-BRN-002 / 003 / R6-MBK-002: ids and component definitions on Open and Restore -----------------------

#[test]
fn open_raises_a_low_next_id_above_every_id_including_inside_components() {
    let dir = scratch("ids");
    let doc = dir.join("a.sketchcraft");
    let e = sample();
    let mut v: Value = serde_json::to_value(&e.model).unwrap();
    v["next_id"] = json!(0);
    for d in v["definitions"].as_object_mut().unwrap().values_mut() {
        d["model"]["next_id"] = json!(1);
    }
    std::fs::write(&doc, serde_json::to_vec(&v).unwrap()).unwrap();
    let mut e2 = Engine::new();
    eng(&mut e2, json!({"cmd": "open", "path": doc}));
    let (points, faces) = (e2.model.vertices.len(), e2.model.faces.len());
    assert!(e2.model.next_id >= *e2.model.vertices.keys().max().unwrap());
    for d in e2.model.definitions.values() {
        assert!(d.model.next_id >= *d.model.vertices.keys().max().unwrap());
    }
    // drawing now takes new ids and overwrites nothing
    eng(&mut e2, json!({"cmd": "rect", "from": [30, 0, 0], "to": [32, 2, 0]}));
    assert_eq!(e2.model.vertices.len(), points + 4);
    assert_eq!(e2.model.faces.len(), faces + 1);
    assert!(e2.model.broken_refs(true).is_empty(), "{:?}", e2.model.broken_refs(true));
    // the same repair for Restore (replace_model) and for a library file
    let mut m: Model = serde_json::from_value(v).unwrap();
    let mut e3 = Engine::new();
    e3.replace_model(m.clone()).unwrap();
    assert!(e3.model.next_id >= *e3.model.vertices.keys().max().unwrap());
    m.next_id = 0;
    std::fs::write(&doc, serde_json::to_vec(&m).unwrap()).unwrap();
    let lib = sketchcraft::library::load_file(doc.to_str().unwrap()).unwrap();
    assert!(lib.next_id >= *lib.vertices.keys().max().unwrap());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn components_are_checked_like_the_top_level_on_open_and_restore() {
    let dir = scratch("defs");
    let doc = dir.join("a.sketchcraft");
    let e = sample();
    let mut v: Value = serde_json::to_value(&e.model).unwrap();
    // a face inside a component names a point that isn't there
    let def = v["definitions"].as_object_mut().unwrap().values_mut().next().unwrap();
    let face = def["model"]["faces"].as_object_mut().unwrap().values_mut().next().unwrap();
    face["outer"] = json!([1, 2, 99999]);
    std::fs::write(&doc, serde_json::to_vec(&v).unwrap()).unwrap();
    let mut e2 = Engine::new();
    let r = e2.exec_json(&json!({"cmd": "open", "path": doc}).to_string());
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["error"].as_str().unwrap().contains("component"), "{r}");
    let m: Model = serde_json::from_value(v).unwrap();
    assert!(Engine::new().replace_model(m).is_err());
    // a placement inside a component that names a component that isn't there
    let mut v: Value = serde_json::to_value(&e.model).unwrap();
    let def = v["definitions"].as_object_mut().unwrap().values_mut().next().unwrap();
    def["model"]["instances"] = json!({"5": {"def": 777, "name": "x", "transform": [1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1]}});
    std::fs::write(&doc, serde_json::to_vec(&v).unwrap()).unwrap();
    let r = Engine::new().exec_json(&json!({"cmd": "open", "path": doc}).to_string());
    assert_eq!(r["ok"], false, "{r}");
    std::fs::remove_dir_all(&dir).ok();
}

/// 300 single or double JSON mutations of a small model: each is opened (it may be refused), drawn, given a few commands and drawn
/// again, under a watchdog. A model that opens must draw, and its commands must never hit a bug.
#[test]
fn mutated_model_files_open_cleanly_or_are_refused() {
    let base: Value = serde_json::to_value(&sample().model).unwrap();
    let mut seed = 0x9e3779b97f4a7c15u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    // every path in the JSON tree
    fn paths(v: &Value, here: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
        out.push(here.clone());
        match v {
            Value::Object(o) => o.iter().for_each(|(k, x)| {
                here.push(k.clone());
                paths(x, here, out);
                here.pop();
            }),
            Value::Array(a) => a.iter().enumerate().for_each(|(i, x)| {
                here.push(i.to_string());
                paths(x, here, out);
                here.pop();
            }),
            _ => {}
        }
    }
    let mut all = vec![];
    paths(&base, &mut vec![], &mut all);
    let pick = |v: &mut Value, p: &[String]| -> bool {
        let mut cur = v;
        for k in &p[..p.len().saturating_sub(1)] {
            cur = match cur {
                Value::Object(o) => match o.get_mut(k) {
                    Some(x) => x,
                    None => return false,
                },
                Value::Array(a) => match k.parse::<usize>().ok().and_then(|i| a.get_mut(i)) {
                    Some(x) => x,
                    None => return false,
                },
                _ => return false,
            };
        }
        let Some(last) = p.last() else { return false };
        let repl = [json!(null), json!(-1), json!(0), json!(99999), json!("x"), json!([]), json!({}), json!(1e300), json!(true)];
        let r = repl[(last.len() * 7 + p.len()) % repl.len()].clone();
        match cur {
            Value::Object(o) => {
                if o.contains_key(last) {
                    if p.len() % 3 == 0 {
                        o.remove(last);
                    } else {
                        o.insert(last.clone(), r);
                    }
                    true
                } else {
                    false
                }
            }
            Value::Array(a) => match last.parse::<usize>().ok().filter(|i| *i < a.len()) {
                Some(i) => {
                    a[i] = r;
                    true
                }
                None => false,
            },
            _ => false,
        }
    };
    let dir = scratch("fuzz");
    let (mut opened, mut refused) = (0, 0);
    for case in 0..300 {
        let mut v = base.clone();
        let n = 1 + (next() % 2) as usize;
        for _ in 0..n {
            let p = &all[(next() as usize) % all.len()];
            pick(&mut v, p);
        }
        let doc = dir.join(format!("m{case}.sketchcraft"));
        std::fs::write(&doc, serde_json::to_vec(&v).unwrap()).unwrap();
        let mut e = Engine::new();
        let r = e.exec_json(&json!({"cmd": "open", "path": doc}).to_string());
        if r["ok"] != true {
            refused += 1;
            continue;
        }
        opened += 1;
        // drawn: the scene builder must not panic or report an error
        let scene = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sketchcraft::scene::build(&e.model, None)));
        let scene = scene.unwrap_or_else(|_| panic!("case {case}: the scene panicked for {v}"));
        assert!(scene.get("error").is_none(), "case {case}: {scene} for {v}");
        for c in [json!({"cmd": "rect", "from": [40, 0, 0], "to": [42, 2, 0]}), json!({"cmd": "faces"}), json!({"cmd": "info"}), json!({"cmd": "components"}), json!({"cmd": "purge"}), json!({"cmd": "undo"})] {
            let r = e.exec_json(&c.to_string());
            let err = r["error"].as_str().unwrap_or("");
            assert!(!err.contains("hit a bug"), "case {case}: {c} -> {r} for {v}");
        }
    }
    assert!(opened > 20 && refused > 20, "opened {opened}, refused {refused}");
    std::fs::remove_dir_all(&dir).ok();
}

// ---- R6-BRN-005: the tolerance cannot hide a real foreign edit at survey coordinates --------------------------

fn survey_trial(northing: f64, nudge: f64) -> Result<(), String> {
    let dir = scratch(&format!("tol-{}", (northing + nudge * 1e6) as i64));
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    let f = ok(&mut a, json!({"cmd": "rect", "from": [4.1e5, northing, 0], "to": [4.1e5 + 3.3, northing + 2.7, 0]}))["face"].as_u64().unwrap();
    ok(&mut a, json!({"cmd": "pushpull", "face": f, "distance": 3.7}));
    pause();
    // another program nudges the very same face's points by `nudge` (a real edit) and saves
    let mut b = Live::open(doc.clone());
    let ids: Vec<u64> = b.engine.model.vertices.keys().map(|k| *k as u64).collect();
    ok(&mut b, json!({"cmd": "move", "ids": ids, "by": [0, nudge, 0]}));
    let r = a.exec(&json!({"cmd": "undo"}));
    std::fs::remove_dir_all(&dir).ok();
    if r["ok"] == true { Ok(()) } else { Err(r["error"].to_string()) }
}

#[test]
fn undo_refuses_a_millimetre_edit_by_another_program_at_survey_coordinates() {
    // northing 5,000,000 m: units in the last place are a nanometre; 4 mm and 2 cm are real edits and are refused
    for nudge in [0.004, 0.02, 0.0003] {
        assert!(survey_trial(5.0e6, nudge).is_err(), "a {nudge} m foreign edit at 5e6 was overwritten by Undo");
    }
}

#[test]
fn the_float_match_follows_the_size_of_the_number() {
    // bit-exact, one unit in the last place, and a nanometre are the same number; a millimetre at 5e6 is not
    let (a, b) = (5_000_000.123456789f64, 5_000_000.123456789f64);
    assert!(sketchcraft::history::floats_match_for_test(a, b));
    assert!(sketchcraft::history::floats_match_for_test(a, f64::from_bits(a.to_bits() + 1)));
    assert!(sketchcraft::history::floats_match_for_test(0.0, 1e-12));
    assert!(!sketchcraft::history::floats_match_for_test(a, a + 0.001));
    assert!(!sketchcraft::history::floats_match_for_test(a, a + 1e-7));
    assert!(!sketchcraft::history::floats_match_for_test(1.0, 1.0 + 1e-6));
    assert!(!sketchcraft::history::floats_match_for_test(f64::NAN, f64::NAN));
}

// ---- R6-BRN-004: a redo step survives a second foreign save --------------------------------------------------

#[test]
fn redo_survives_a_second_save_by_another_program() {
    let dir = scratch("redo2");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    ok(&mut a, json!({"cmd": "rect", "from": [0.31, 0.17, 0], "to": [3.31, 2.17, 0]}));
    pause();
    let mut b = Live::open(doc.clone());
    ok(&mut b, json!({"cmd": "rect", "from": [100, 0, 0], "to": [102, 2, 0]}));
    ok(&mut a, json!({"cmd": "rect", "from": [20.31, 0.17, 0], "to": [23.31, 2.17, 0]}));
    ok(&mut a, json!({"cmd": "undo"}));
    ok(&mut a, json!({"cmd": "undo"}));
    pause();
    ok(&mut b, json!({"cmd": "rect", "from": [200, 0, 0], "to": [202, 2, 0]}));
    // the next thing A does reloads B's second save: the notice must not claim an Undo step was lost, and both redos still work
    let r = ok(&mut a, json!({"cmd": "redo"}));
    assert!(r.get("error").is_none());
    assert!(a.notice.as_deref().is_none_or(|n| !n.contains("oldest")), "{:?}", a.notice);
    ok(&mut a, json!({"cmd": "redo"}));
    assert_eq!(a.engine.model.faces.len(), 4, "A's two rectangles and B's two are all there");
    // and the whole chain undoes again
    ok(&mut a, json!({"cmd": "undo"}));
    ok(&mut a, json!({"cmd": "undo"}));
    assert_eq!(a.engine.model.faces.len(), 2);
    std::fs::remove_dir_all(&dir).ok();
}

// ---- R6-MBK-005 / R6-BRN-006: the side-file index ---------------------------------------------------------

fn make_side_files(dir: &Path, doc: &Path, n: usize) -> Live {
    let mut a = Live::open(doc.to_path_buf());
    ok(&mut a, json!({"cmd": "rect", "from": [0, 0, 0], "to": [4, 4, 0]}));
    for k in 0..n {
        pause();
        let mut other = Live::open(doc.to_path_buf());
        a.exec(&json!({"cmd": "begin_batch"}));
        a.exec_as(&Caller::Plugin, &json!({"cmd": "rect", "from": [100 + k * 10, 0, 0], "to": [104 + k * 10, 4, 0]}));
        ok(&mut other, json!({"cmd": "rect", "from": [-100 - 10 * k as i64, 0, 0], "to": [-96 - 10 * k as i64, 4, 0]}));
        a.exec(&json!({"cmd": "end_batch"}));
    }
    let _ = dir;
    a
}

#[test]
fn a_lost_or_unreadable_index_neither_orphans_files_nor_lifts_the_cap() {
    let dir = scratch("index");
    let doc = dir.join("live.sketchcraft");
    let mut a = make_side_files(&dir, &doc, 3);
    let side = |dir: &Path| -> Vec<String> { std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.contains(".unsaved-")).collect() };
    let made = side(&dir);
    assert!(made.len() >= 2, "{made:?}");
    // the index is deleted: the files are still listed (as not verified) in Restore
    std::fs::remove_file(sketchcraft::backups::index_path(&doc)).unwrap();
    let l = a.exec(&json!({"cmd": "list_backups"}));
    let listed = |l: &Value| l["items"].as_array().unwrap().iter().filter(|i| i["kind"] == "unsaved").count();
    assert_eq!(listed(&l), made.len(), "{l}");
    // the next set-aside file rebuilds the index with the earlier ones (they were written when their names say)
    pause();
    let mut other = Live::open(doc.clone());
    a.exec(&json!({"cmd": "begin_batch"}));
    a.exec_as(&Caller::Plugin, &json!({"cmd": "rect", "from": [900, 0, 0], "to": [904, 4, 0]}));
    ok(&mut other, json!({"cmd": "rect", "from": [-900, 0, 0], "to": [-896, 4, 0]}));
    a.exec(&json!({"cmd": "end_batch"}));
    let idx = sketchcraft::backups::read_index(&doc);
    assert_eq!(idx.len(), side(&dir).len(), "{idx:?}");
    let l = a.exec(&json!({"cmd": "list_backups"}));
    assert!(l["items"].as_array().unwrap().iter().filter(|i| i["kind"] == "unsaved").all(|i| i["verified"] == true), "{l}");
    // garbage in the index: same
    std::fs::write(sketchcraft::backups::index_path(&doc), b"not json").unwrap();
    let l = a.exec(&json!({"cmd": "list_backups"}));
    assert_eq!(listed(&l), side(&dir).len());
    std::fs::remove_dir_all(&dir).ok();
}

// ---- R6-BRN-003 / start-up: a file that can't be read is never overwritten by the first command --------------

#[test]
fn an_unreadable_file_at_start_is_never_overwritten_and_restore_brings_it_back() {
    let dir = scratch("startup");
    let doc = dir.join("live.sketchcraft");
    // a good model with a bad component, as a sync tool might leave it
    let e = sample();
    let mut v: Value = serde_json::to_value(&e.model).unwrap();
    let def = v["definitions"].as_object_mut().unwrap().values_mut().next().unwrap();
    def["model"]["faces"].as_object_mut().unwrap().values_mut().next().unwrap()["outer"] = json!([1, 2, 99999]);
    let bytes = serde_json::to_vec(&v).unwrap();
    std::fs::write(&doc, &bytes).unwrap();
    let mut live = Live::open(doc.clone());
    assert!(live.notice.as_deref().is_some_and(|n| n.contains("Nothing will be saved over the file")), "{:?}", live.notice);
    for _ in 0..3 {
        let r = live.exec(&json!({"cmd": "rect", "from": [0, 0, 0], "to": [4, 4, 0]}));
        assert!(r["save_error"].is_string(), "{r}");
        assert_eq!(std::fs::read(&doc).unwrap(), bytes, "the first commands left the file alone");
        assert!(live.scene()["notice"].as_str().is_some_and(|n| n.contains("Restore")), "the notice keeps saying what to do: {:?}", live.notice);
    }
    // the aside copy is listed; restoring a good backup (here: a good model written as the aside copy) works and saving resumes
    let l = live.exec(&json!({"cmd": "list_backups"}));
    let aside = l["items"].as_array().unwrap().iter().find(|i| i["kind"] == "damaged").unwrap_or_else(|| panic!("{l}"))["name"].as_str().unwrap().to_string();
    std::fs::write(dir.join(&aside), serde_json::to_vec(&e.model).unwrap()).unwrap();
    let r = live.exec(&json!({"cmd": "restore_backup", "kind": "damaged", "name": aside, "mode": "replace"}));
    assert_eq!(r["ok"], true, "{r}");
    assert!(r.get("save_error").is_none(), "{r}");
    let on_disk: Model = serde_json::from_slice(&std::fs::read(&doc).unwrap()).unwrap();
    assert_eq!(on_disk.faces.len(), e.model.faces.len());
    std::fs::remove_dir_all(&dir).ok();
}

// ---- add_location is the person's -------------------------------------------------------------------------------

#[test]
fn an_agent_cannot_start_a_network_fetch() {
    let mut e = Engine::new();
    let r = as_agent(&mut e, json!({"cmd": "add_location", "lat": 30.2, "lon": -97.7}));
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["error"].as_str().unwrap().contains("isn't available") || r["error"].as_str().unwrap().contains("not available") || r["error"].as_str().unwrap().contains("agent"), "{r}");
}

// ---- R6-BRN-007: a model near the budget stays usable (timings are printed; the limits are generous so a slow machine passes) ----

/// `cargo test --release -p sketchcraft-core --test round6 -- --ignored --nocapture big_model_timings`
#[test]
#[ignore = "takes a minute and 2 GB of memory; run by hand"]
fn big_model_timings() {
    let dir = scratch("perf");
    let doc = dir.join("live.sketchcraft");
    let mut live = Live::open(doc.clone());
    let t = std::time::Instant::now();
    ok(&mut live, json!({"cmd": "terrain_grid", "at": [0, 0, 0], "width": 400, "depth": 400, "spacing": 1}));
    ok(&mut live, json!({"cmd": "terrain_grid", "at": [500, 0, 0], "width": 330, "depth": 330, "spacing": 1}));
    eprintln!("terrain_grid 400 + 330: {:?}, {} entities, estimate {} MB, file {} MB", t.elapsed(), live.engine.model.entity_count(), live.engine.model.size_estimate() >> 20, std::fs::metadata(&doc).map(|m| m.len() >> 20).unwrap_or(0));
    for i in 0..3 {
        let t = std::time::Instant::now();
        ok(&mut live, json!({"cmd": "rect", "from": [i * 10, 0, 1000], "to": [i * 10 + 4, 4, 1000]}));
        eprintln!("rect #{i}: {:?}", t.elapsed());
    }
    let t = std::time::Instant::now();
    let s = live.scene();
    eprintln!("scene: {:?} ({} bytes)", t.elapsed(), s.to_string().len());
    let t = std::time::Instant::now();
    ok(&mut live, json!({"cmd": "undo"}));
    eprintln!("undo: {:?}", t.elapsed());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
#[ignore = "run by hand"]
fn big_model_phase_timings() {
    let mut e = Engine::new();
    eng(&mut e, json!({"cmd": "terrain_grid", "at": [0, 0, 0], "width": 400, "depth": 400, "spacing": 1}));
    eng(&mut e, json!({"cmd": "terrain_grid", "at": [500, 0, 0], "width": 330, "depth": 330, "spacing": 1}));
    let t = std::time::Instant::now();
    let c = e.model.clone();
    eprintln!("clone: {:?}", t.elapsed());
    drop(c);
    let t = std::time::Instant::now();
    let n = e.model.size_estimate();
    eprintln!("size_estimate: {:?} ({n})", t.elapsed());
    let t = std::time::Instant::now();
    e.model.check_sane().unwrap();
    eprintln!("check_sane: {:?}", t.elapsed());
    let t = std::time::Instant::now();
    let n = e.model.approx_bytes();
    eprintln!("approx_bytes: {:?} ({n})", t.elapsed());
    let t = std::time::Instant::now();
    let b = serde_json::to_vec(&e.model).unwrap();
    eprintln!("to_vec: {:?} ({})", t.elapsed(), b.len());
    let t = std::time::Instant::now();
    let _m: Model = serde_json::from_slice(&b).unwrap();
    eprintln!("from_slice: {:?}", t.elapsed());
    let t = std::time::Instant::now();
    eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 1000], "to": [4, 4, 1000]}));
    eprintln!("engine rect: {:?}", t.elapsed());
    let t = std::time::Instant::now();
    let s = sketchcraft::scene::build(&e.model, None);
    eprintln!("scene::build: {:?}", t.elapsed());
    let t = std::time::Instant::now();
    let txt = s.to_string();
    eprintln!("scene to_string: {:?} ({})", t.elapsed(), txt.len());
}

// ---- R6-MBK-008 / R6-MBK-004: undo memory and who decides which step is skipped ------------------------------

#[test]
fn undo_memory_counts_the_model_copy_a_reload_makes() {
    let mut e = Engine::new();
    eng(&mut e, json!({"cmd": "terrain_grid", "at": [0, 0, 0], "width": 60, "depth": 60, "spacing": 1}));
    eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 100], "to": [4, 4, 100]}));
    let one_model = e.model.approx_bytes();
    let h = e.history_for_reload().expect("one step");
    let held: usize = h.undo.iter().map(|s| s.approx_bytes()).sum();
    // the step holds the model before it and a fresh copy of the one that was on screen: two models, not one
    assert!(held as f64 > 1.8 * one_model as f64, "{held} vs one model of {one_model}");
}

#[test]
fn only_the_persons_second_press_skips_a_refused_step_and_another_programs_save_never_does() {
    let dir = scratch("skip");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    let f = ok(&mut a, json!({"cmd": "rect", "from": [0.3, 0.2, 0], "to": [3.3, 2.2, 0]}))["face"].as_u64().unwrap();
    ok(&mut a, json!({"cmd": "pushpull", "face": f, "distance": 1.7}));
    ok(&mut a, json!({"cmd": "rect", "from": [50.3, 0.2, 0], "to": [53.3, 2.2, 0]}));
    pause();
    // another program pushes the same face further: the first step's undo is now in the way of the second program's work
    let mut b = Live::open(doc.clone());
    let top = b.engine.model.faces.iter().find(|(_, fc)| fc.outer.iter().all(|v| (b.engine.model.vertices[v].p.z - 1.7).abs() < 1e-9)).map(|(k, _)| *k as u64).unwrap();
    ok(&mut b, json!({"cmd": "pushpull", "face": top, "distance": 2.3}));
    let r = a.exec(&json!({"cmd": "undo"})); // the second rectangle (it is the person's newest step) comes off first
    assert_eq!(r["ok"], true, "{r}");
    let r = a.exec(&json!({"cmd": "undo"}));
    assert_eq!(r["ok"], false, "refused: {r}");
    // an unrelated save by the other program in between does not skip it: the same step is tried again
    pause();
    ok(&mut b, json!({"cmd": "rect", "from": [300, 0, 0], "to": [302, 2, 0]}));
    let r = a.exec(&json!({"cmd": "undo"}));
    assert_eq!(r["ok"], false, "still refused, not skipped: {r}");
    assert!(r["error"].as_str().unwrap().contains("kept"), "{r}");
    // the person's own second press is what skips it
    let r = a.exec(&json!({"cmd": "undo"}));
    assert!(r["ok"] == false || r["skipped"].is_number(), "{r}");
    std::fs::remove_dir_all(&dir).ok();
}
