//! Round 7 (security-reports/R7-*.md, fixes in RED-TEAM-REMEDIATION.md): components inside components survive Open, Restore and
//! reload; text and attribute limits don't lock a model out; pictures up to realistic sizes and never silently dropped; the file
//! size estimate counts escapes; formulas are worked out once; a blocked file clears; ids never wrap; materials can be removed.
mod skp_fixture;

use serde_json::{json, Value};
use sketchcraft::live::Live;
use sketchcraft::model::Model;
use sketchcraft::policy::{Caller, Workspace};
use sketchcraft::Engine;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sc-round7-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn eng(e: &mut Engine, v: Value) -> Value {
    let r = e.exec_json(&v.to_string());
    assert_eq!(r["ok"], true, "{v} -> {r}");
    r
}

fn live(l: &mut Live, v: Value) -> Value {
    let r = l.exec(&v);
    assert_eq!(r["ok"], true, "{v} -> {r}");
    r
}

fn agent() -> Caller {
    Caller::Mcp(Workspace::anywhere())
}

fn as_agent(e: &mut Engine, v: Value) -> Value {
    e.exec_json_as(&agent(), &v.to_string())
}

fn as_plugin(e: &mut Engine, v: Value) -> Value {
    e.exec_json_as(&Caller::Plugin, &v.to_string())
}

fn b64(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// A real PNG of the given size (all one grey), which compresses to a few hundred KB at most.
fn png(w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![];
    image::GrayImage::new(w, h).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

/// Just the header of a PNG that claims `w` x `h` (nothing to decode: it is the header that is judged).
fn claimed_png(w: u32, h: u32) -> Vec<u8> {
    let mut ihdr = vec![];
    ihdr.extend(w.to_be_bytes());
    ihdr.extend(h.to_be_bytes());
    ihdr.extend([1, 0, 0, 0, 0]); // 1 bit, grey
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    for (kind, body) in [(&b"IHDR"[..], &ihdr[..]), (&b"IDAT"[..], &[0x78, 0x9c, 0x03, 0, 0, 0, 0, 1][..]), (&b"IEND"[..], &[][..])] {
        let mut chunk = kind.to_vec();
        chunk.extend(body);
        out.extend((body.len() as u32).to_be_bytes());
        out.extend(&chunk);
        out.extend(crc32(&chunk).to_be_bytes());
    }
    out
}

fn url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", b64(bytes))
}

// ---- R7-MBK-001: components inside components --------------------------------------------------------------

fn nested_import() -> Engine {
    let mut e = Engine::new();
    let r = eng(&mut e, json!({"cmd": "import_skp", "data": b64(&skp_fixture::modern_skp())}));
    assert_eq!(r["placed"], 1);
    // the real fixture: definition 2 (the outer part) holds a placement of definition 1, which is in the root's table only
    let nested: usize = e.model.definitions.values().map(|d| d.model.instances.len()).sum();
    assert_eq!(nested, 1, "the fixture has a component inside a component");
    assert!(e.model.definitions.values().all(|d| d.model.definitions.is_empty()), "a component's own table is empty");
    e
}

#[test]
fn a_nested_import_is_a_consistent_model() {
    let e = nested_import();
    assert!(e.model.broken_refs(false).is_empty(), "{:?}", e.model.broken_refs(false));
    assert!(e.model.broken_refs(true).is_empty(), "{:?}", e.model.broken_refs(true));
}

#[test]
fn a_nested_model_saves_and_reopens_in_a_fresh_engine() {
    let dir = scratch("nested-open");
    let doc = dir.join("n.sketchcraft");
    let mut e = nested_import();
    eng(&mut e, json!({"cmd": "save", "path": doc}));
    let mut fresh = Engine::new();
    let r = fresh.exec_json(&json!({"cmd": "open", "path": doc}).to_string());
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(fresh.model.world_faces().len(), e.model.world_faces().len());
    assert_eq!(fresh.model.world_faces().len(), 1);
    // Restore takes it too
    let mut again = Engine::new();
    again.replace_model(e.model.clone()).expect("Restore accepts components inside components");
    assert_eq!(again.model.world_faces().len(), 1);
    // and a library file with parts inside parts
    let m = sketchcraft::library::load_file(doc.to_str().unwrap()).expect("library accepts components inside components");
    assert_eq!(m.definitions.len(), e.model.definitions.len());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_nested_model_survives_a_restart_a_reload_and_a_restore_from_backup() {
    let dir = scratch("nested-live");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    live(&mut a, json!({"cmd": "import_skp", "data": b64(&skp_fixture::modern_skp())}));
    assert!(doc.exists());
    // a restart: no notice, saving on, nothing set aside
    let mut b = Live::open(doc.clone());
    assert!(b.notice.is_none(), "{:?}", b.notice);
    let r = live(&mut b, json!({"cmd": "rect", "from": [30, 0, 0], "to": [32, 2, 0]}));
    assert!(r.get("save_error").is_none(), "{r}");
    assert!(!std::fs::read_dir(&dir).unwrap().flatten().any(|f| f.file_name().to_string_lossy().contains("damaged")));
    // the other program's reload of the file just saved
    std::thread::sleep(Duration::from_millis(1100));
    let r = live(&mut b, json!({"cmd": "rect", "from": [40, 0, 0], "to": [42, 2, 0]}));
    assert!(r.get("save_error").is_none(), "{r}");
    assert!(a.sync(), "the first program reloads the second's save");
    assert!(a.notice.is_none(), "{:?}", a.notice);
    assert_eq!(a.engine.model.world_faces().len(), 3);
    // a backup of it restores (a "new" takes a backup of the outgoing file first)
    live(&mut a, json!({"cmd": "new"}));
    let l = a.exec(&json!({"cmd": "list_backups"}));
    let first = l["items"].as_array().unwrap().iter().find(|i| i["kind"] == "backup").expect("a backup was taken");
    let r = a.exec(&json!({"cmd": "restore_backup", "kind": "backup", "name": first["name"], "mode": "replace"}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(a.engine.model.world_faces().len(), 3);
    std::fs::remove_dir_all(&dir).ok();
}

// ---- R7-BRN-001: the estimate counts what the file holds ---------------------------------------------------

/// Strings that grow when written: control characters, quotes, backslashes, non-ASCII, DEL.
fn nasty(seed: u32, len: usize) -> String {
    let set = ['\u{0}', '\u{1}', '\u{8}', '\n', '\t', '\u{1f}', '"', '\\', 'a', 'é', '€', '😀', '\u{7f}', '/', '<'];
    let mut x = seed.wrapping_mul(2654435761).wrapping_add(12345);
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            set[(x % set.len() as u32) as usize]
        })
        .collect()
}

#[test]
fn the_estimate_is_at_least_the_real_size_for_adversarial_strings() {
    for seed in 0..12u32 {
        let mut e = Engine::new();
        let f = eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}))["face"].as_u64().unwrap();
        eng(&mut e, json!({"cmd": "paint", "ids": [f], "color": [1.0, 0.0, 0.0]}));
        let g = eng(&mut e, json!({"cmd": "group", "ids": [f], "name": "G"}))["group"].as_u64().unwrap();
        let c = eng(&mut e, json!({"cmd": "make_component", "ids": [g], "name": "C"}))["component"].as_u64().unwrap();
        let p = eng(&mut e, json!({"cmd": "place", "component": c, "at": [5, 5, 0]}))["instance"].as_u64().unwrap() as u32;
        eng(&mut e, json!({"cmd": "text", "at": [0, 0, 0], "text": "t"}));
        eng(&mut e, json!({"cmd": "save_view", "name": "V", "eye": [1, 2, 3], "target": [0, 0, 0]}));
        eng(&mut e, json!({"cmd": "tag", "ids": [p], "name": "Walls"}));
        eng(&mut e, json!({"cmd": "texture", "name": "Tex", "data": url(&png(8, 8))}));
        eng(&mut e, json!({"cmd": "section_plane", "origin": [0, 0, 1], "normal": [0, 0, 1], "name": "S"}));
        let m = &mut e.model;
        let n = |k: u32, l: usize| nasty(seed * 100 + k, l);
        // every free string in the model becomes adversarial
        let mats: Vec<_> = std::mem::take(&mut m.materials).into_iter().collect();
        for (i, (_, mut mat)) in mats.into_iter().enumerate() {
            if let Some(t) = mat.texture.as_mut() {
                t.data = format!("data:image/png;base64,{}", n(1, 400));
            }
            let name = n(10 + i as u32, 20);
            for f in m.faces.values_mut() {
                f.material = Some(name.clone());
            }
            m.materials.insert(name, mat);
        }
        for g in m.groups.values_mut() {
            g.name = n(2, 50);
            g.tag = Some(n(3, 30));
        }
        for i in m.instances.values_mut() {
            i.name = n(4, 60);
            i.tag = Some(n(5, 40));
            i.attrs.insert(n(6, 8), n(7, 100));
        }
        let _ = p;
        for d in m.definitions.values_mut() {
            d.name = n(8, 40);
            d.source = Some(n(9, 70));
            d.attrs.insert(n(11, 8), n(12, 90));
        }
        for v in m.views.iter_mut() {
            v.name = n(13, 30);
            v.photo = Some(format!("data:image/png;base64,{}", n(14, 300)));
            v.tags = Some([(n(15, 20), true)].into());
            v.style = Some(json!({"k": n(16, 40)}));
        }
        for a in m.annotations.iter_mut() {
            if let sketchcraft::model::Annotation::Text { text, .. } = a {
                *text = n(17, 200);
            }
        }
        for s in m.sections.iter_mut() {
            s.name = n(18, 30);
        }
        m.tags = [(n(19, 25), true), (n(20, 25), false)].into();
        m.tag_colors = [(n(21, 25), [1.0, 0.0, 0.0])].into();
        m.classes = m.groups.keys().map(|k| (*k, n(22, 30))).collect();
        m.environment = Some(n(23, 30));
        m.units = n(24, 10);
        let real = serde_json::to_vec(&*m).unwrap().len();
        let est = m.size_estimate();
        assert!(est >= real, "seed {seed}: estimate {est} is below the real size {real}");
        // not absurdly above it either (it must not refuse ordinary work)
        assert!(est <= real * 3 + 4096, "seed {seed}: estimate {est} is far above the real size {real}");
    }
}

#[test]
fn the_text_helper_counts_escapes_exactly_enough() {
    for s in ["", "plain", "quote\"s", "back\\slash", "\u{0}\u{1}\u{1f}", "é€😀", "\u{7f}", "line\nbreak\ttab"] {
        let real = serde_json::to_string(s).unwrap().len() - 2;
        assert!(sketchcraft::model::jl(s) >= real, "{s:?}: {} < {real}", sketchcraft::model::jl(s));
        assert!(sketchcraft::model::jl(s) <= real * 3 + 1, "{s:?}");
    }
}

#[test]
fn an_agent_flooding_control_characters_cannot_save_a_model_that_wont_reopen() {
    let dir = scratch("escapes");
    let doc = dir.join("e.sketchcraft");
    let mut l = Live::open(doc.clone());
    let f = live(&mut l, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}))["face"].as_u64().unwrap();
    let g = live(&mut l, json!({"cmd": "group", "ids": [f]}))["group"].as_u64().unwrap();
    let c = live(&mut l, json!({"cmd": "make_component", "ids": [g], "name": "C"}))["component"].as_u64().unwrap();
    let mut accepted = 0;
    for i in 0..40 {
        // each value 60 KB of U+0001 (six bytes each in the file)
        let attrs: serde_json::Map<String, Value> = (0..20).map(|k| (format!("k{i}_{k}"), json!("\u{1}".repeat(9_000)))).collect();
        let r = l.exec_as(&agent(), &json!({"cmd": "set_attrs", "id": c, "attrs": attrs}));
        if r["ok"] == true {
            accepted += 1;
        } else {
            break;
        }
    }
    assert!(accepted >= 1, "the agent can still write attributes");
    let size = std::fs::metadata(&doc).unwrap().len();
    let est = l.engine.model.size_estimate() as u64;
    assert!(size <= est, "file {size} over the estimate {est}");
    assert!(size < 40 << 20, "the agent's text stays within its small share ({size})");
    let mut again = Live::open(doc);
    assert!(again.notice.is_none(), "{:?}", again.notice);
    assert!(again.engine.model.definitions.len() == 1);
    std::fs::remove_dir_all(&dir).ok();
}

// ---- R7-BRN-005 / R7-BRN-006: the text pool ----------------------------------------------------------------

fn place_one(e: &mut Engine) -> Id {
    let f = eng(e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}))["face"].as_u64().unwrap();
    let g = eng(e, json!({"cmd": "group", "ids": [f]}))["group"].as_u64().unwrap();
    eng(e, json!({"cmd": "make_component", "ids": [g], "name": "Thing"}))["component"].as_u64().unwrap() as Id
}

type Id = u32;

#[test]
fn an_agent_cannot_use_up_the_persons_text_room() {
    let mut e = Engine::new();
    let c = place_one(&mut e);
    let mut refused = None;
    for i in 0..400 {
        let r = as_agent(&mut e, json!({"cmd": "place", "component": c, "at": [i, 5, 0], "name": "x".repeat(60_000)}));
        if r["ok"] != true {
            refused = Some((i, r));
            break;
        }
    }
    let (i, r) = refused.expect("an agent's names stop at its share");
    assert!(i < 60, "{i}");
    assert!(r["error"].as_str().unwrap().contains("agent"), "{r}");
    assert!(e.model.text_bytes() < 4 << 20);
    // the person's own naming, notes, groups and new colours all still work, in the large
    for k in 0..30 {
        eng(&mut e, json!({"cmd": "text", "at": [0, 0, 0], "text": format!("note {k} {}", "n".repeat(9_000))}));
    }
    let f = eng(&mut e, json!({"cmd": "rect", "from": [0, 20, 0], "to": [2, 22, 0]}))["face"].as_u64().unwrap();
    eng(&mut e, json!({"cmd": "paint", "ids": [f], "color": [0.1, 0.2, 0.3]}));
    eng(&mut e, json!({"cmd": "group", "ids": [f], "name": "Mine"}));
    eng(&mut e, json!({"cmd": "save_view", "name": "My view", "eye": [1, 2, 3], "target": [0, 0, 0]}));
}

#[test]
fn a_model_over_the_text_or_attribute_limits_opens_for_cutting_down() {
    let dir = scratch("textopen");
    let doc = dir.join("t.sketchcraft");
    let mut e = Engine::new();
    let c = place_one(&mut e);
    let mut ids = vec![];
    for i in 0..20 {
        ids.push(eng(&mut e, json!({"cmd": "place", "component": c, "at": [i, 5, 0]}))["instance"].as_u64().unwrap());
    }
    // the file holds 70 MB of names (as a model from before the limit, or from another program, might) and a placement with 300 attributes
    let mut m = e.model.clone();
    let keys: Vec<Id> = m.instances.keys().copied().collect();
    for k in &keys[..keys.len().min(2)] {
        m.instances.get_mut(k).unwrap().name = "N".repeat(36 << 20);
    }
    let last = *keys.last().unwrap();
    m.instances.get_mut(&last).unwrap().attrs = (0..300).map(|i| (format!("a{i}"), "1".into())).collect();
    assert!(m.text_bytes() > sketchcraft::model::MAX_TEXT_BYTES);
    std::fs::write(&doc, serde_json::to_vec(&m).unwrap()).unwrap();
    drop(m);
    let mut l = Live::open(doc.clone());
    assert!(l.engine.oversize, "opened for cutting down, not refused");
    assert!(l.notice.as_deref().unwrap_or("").contains("over the size limits"), "{:?}", l.notice);
    // adding is refused, with a reason; removing works; saving resumes when it is under the limits
    let r = l.exec(&json!({"cmd": "text", "at": [0, 0, 0], "text": "more"}));
    assert_eq!(r["ok"], false, "{r}");
    let r = l.exec(&json!({"cmd": "delete", "ids": [keys[0], keys[1]]}));
    assert_eq!(r["ok"], true, "{r}");
    let r = l.exec(&json!({"cmd": "delete", "ids": [last]}));
    assert_eq!(r["ok"], true, "{r}");
    assert!(!l.engine.oversize);
    assert!(l.exec(&json!({"cmd": "text", "at": [0, 0, 0], "text": "more"}))["ok"] == true);
    // Restore of such a model works too
    let mut big = e.model.clone();
    big.instances.get_mut(&keys[0]).unwrap().name = "N".repeat(70 << 20);
    let mut e2 = Engine::new();
    e2.replace_model(big).expect("Restore opens it for cutting down");
    assert!(e2.oversize);
    let _ = ids;
    std::fs::remove_dir_all(&dir).ok();
}

// ---- R7-BRN-002: a blocked file clears ---------------------------------------------------------------------

#[cfg(unix)]
mod blocked {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn unreadable_start(name: &str) -> (PathBuf, PathBuf, Live) {
        let dir = scratch(name);
        let doc = dir.join("b.sketchcraft");
        let mut e = Engine::new();
        eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}));
        std::fs::write(&doc, serde_json::to_vec(&e.model).unwrap()).unwrap();
        std::fs::set_permissions(&doc, std::fs::Permissions::from_mode(0o000)).unwrap();
        let l = Live::open(doc.clone());
        (dir, doc, l)
    }

    fn skip_if_root() -> bool {
        // root reads mode-000 files: the situation can't be made
        std::fs::File::open("/etc/shadow").is_ok()
    }

    #[test]
    fn saving_comes_back_when_the_permission_is_put_back() {
        if skip_if_root() {
            return;
        }
        let (dir, doc, mut l) = unreadable_start("blocked-perm");
        assert!(l.notice.as_deref().unwrap_or("").contains("couldn't be copied aside"), "{:?}", l.notice);
        let r = l.exec(&json!({"cmd": "rect", "from": [10, 0, 0], "to": [12, 2, 0]}));
        assert!(r["save_error"].is_string(), "saving is off while the file is unreadable: {r}");
        std::fs::set_permissions(&doc, std::fs::Permissions::from_mode(0o644)).unwrap();
        l.sync();
        let r = l.exec(&json!({"cmd": "rect", "from": [20, 0, 0], "to": [22, 2, 0]}));
        assert!(r.get("save_error").is_none(), "saving resumed without quitting: {r}");
        // what the session drew while it was blocked was set aside, not lost, and the file's own model was loaded
        let saved: Model = serde_json::from_slice(&std::fs::read(&doc).unwrap()).unwrap();
        assert_eq!(saved.faces.len(), 2, "the file's rectangle and the new one");
        assert!(std::fs::read_dir(&dir).unwrap().flatten().any(|f| f.file_name().to_string_lossy().contains("unsaved")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn saving_comes_back_when_the_file_is_moved_away() {
        if skip_if_root() {
            return;
        }
        let (dir, doc, mut l) = unreadable_start("blocked-moved");
        std::fs::rename(&doc, dir.join("elsewhere.sketchcraft")).unwrap();
        l.sync();
        let r = l.exec(&json!({"cmd": "rect", "from": [20, 0, 0], "to": [22, 2, 0]}));
        assert!(r.get("save_error").is_none(), "{r}");
        assert!(doc.exists());
        std::fs::set_permissions(dir.join("elsewhere.sketchcraft"), std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn new_and_open_end_the_block() {
        if skip_if_root() {
            return;
        }
        let (dir, doc, mut l) = unreadable_start("blocked-new");
        assert_eq!(l.exec(&json!({"cmd": "new"}))["ok"], true);
        let r = l.exec(&json!({"cmd": "rect", "from": [20, 0, 0], "to": [22, 2, 0]}));
        assert!(r.get("save_error").is_none(), "{r}");
        std::fs::set_permissions(&doc, std::fs::Permissions::from_mode(0o644)).ok();
        std::fs::remove_dir_all(&dir).ok();
    }
}

// ---- R7-MBK-003 / R7-BRN-003: ids never wrap --------------------------------------------------------------

#[test]
fn a_counter_or_id_near_the_top_is_refused_and_never_wraps() {
    let dir = scratch("ids");
    let doc = dir.join("a.sketchcraft");
    let mut e = Engine::new();
    eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}));
    let mut v = serde_json::to_value(&e.model).unwrap();
    v["next_id"] = json!(u32::MAX);
    std::fs::write(&doc, serde_json::to_vec(&v).unwrap()).unwrap();
    let r = Engine::new().exec_json(&json!({"cmd": "open", "path": doc}).to_string());
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["error"].as_str().unwrap().contains("ids"), "{r}");
    // an id key at the top with a low counter
    let mut v = serde_json::to_value(&e.model).unwrap();
    let vx = v["vertices"].as_object_mut().unwrap();
    let first = vx.values().next().unwrap().clone();
    vx.insert(u32::MAX.to_string(), first);
    std::fs::write(&doc, serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(Engine::new().exec_json(&json!({"cmd": "open", "path": doc}).to_string())["ok"], false);
    // Restore, and a library file
    let m: Model = serde_json::from_value(v).unwrap();
    assert!(Engine::new().replace_model(m).is_err());
    assert!(sketchcraft::library::load_file(doc.to_str().unwrap()).is_err());
    // a model that is in memory at the very top: the command fails and is undone; nothing is overwritten
    let mut e = Engine::new();
    eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}));
    let before = serde_json::to_string(&e.model).unwrap();
    e.model.next_id = u32::MAX;
    let r = e.exec_json(&json!({"cmd": "rect", "from": [5, 0, 0], "to": [7, 2, 0]}).to_string());
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(e.model.next_id, u32::MAX);
    assert_eq!(e.model.faces.len(), 1);
    assert_eq!(e.model.vertices.len(), 4);
    let _ = before;
    std::fs::remove_dir_all(&dir).ok();
}

// ---- R7-MBK-002: formulas are worked out once ---------------------------------------------------------------

#[test]
fn a_long_chain_of_formulas_is_linear_and_loops_and_depth_are_safe() {
    let mut e = Engine::new();
    let c = place_one(&mut e);
    let p = eng(&mut e, json!({"cmd": "place", "component": c, "at": [5, 5, 0]}))["instance"].as_u64().unwrap();
    // a0 = a1 + a1, a1 = a2 + a2 ... a40 = 1 (2^40 evaluations the old way)
    let mut attrs = serde_json::Map::new();
    for i in 0..40 {
        attrs.insert(format!("a{i}"), json!(format!("=a{}+a{}", i + 1, i + 1)));
    }
    attrs.insert("a40".into(), json!("1"));
    attrs.insert("lenx".into(), json!("=a0/1099511627776"));
    let t = Instant::now();
    let r = as_agent(&mut e, json!({"cmd": "set_attrs", "id": p, "attrs": attrs}));
    assert_eq!(r["ok"], true, "{r}");
    let info = eng(&mut e, json!({"cmd": "options", "id": p}));
    let _ = info;
    eng(&mut e, json!({"cmd": "report"}));
    assert!(e.model.world_faces().len() >= 2);
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
    // wide fan-out too: every name used four times by the next
    let mut attrs = serde_json::Map::new();
    for i in 0..60 {
        attrs.insert(format!("w{i}"), json!(format!("=w{}+w{}+w{}+w{}", i + 1, i + 1, i + 1, i + 1)));
    }
    attrs.insert("w60".into(), json!("1"));
    let t = Instant::now();
    let vals = sketchcraft::dynamic::resolve(&attrs.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect(), &Default::default());
    assert!(t.elapsed() < Duration::from_secs(1), "{:?}", t.elapsed());
    assert!(vals["w0"] > 1e30);
    // a loop gives no value to any name in it (and doesn't hang)
    let loopy: std::collections::BTreeMap<String, String> = [("a".to_string(), "=b+1".to_string()), ("b".to_string(), "=a+1".to_string()), ("c".to_string(), "=2".to_string())].into();
    let vals = sketchcraft::dynamic::resolve(&loopy, &Default::default());
    assert!(!vals.contains_key("a") && !vals.contains_key("b"));
    assert_eq!(vals["c"], 2.0);
    // thousands of brackets and signs don't overflow the stack
    let deep: std::collections::BTreeMap<String, String> = [("x".to_string(), format!("={}1{}", "(".repeat(20_000), ")".repeat(20_000))), ("y".to_string(), format!("={}1", "-".repeat(20_000))), ("z".to_string(), format!("=1{}", "^1".repeat(20_000)))].into();
    let vals = sketchcraft::dynamic::resolve(&deep, &Default::default());
    assert!(vals.is_empty(), "{vals:?}");
    // an ordinary formula still works
    let ok: std::collections::BTreeMap<String, String> = [("a".to_string(), "=b*2+(3-1)^2".to_string()), ("b".to_string(), "5".to_string())].into();
    assert_eq!(sketchcraft::dynamic::resolve(&ok, &Default::default())["a"], 14.0);
}

// ---- R7-BRN-004 / R7-MBK-004: picture sizes ------------------------------------------------------------------

#[test]
fn realistic_photos_and_textures_are_accepted_and_hostile_sizes_refused_at_the_door() {
    let mut e = Engine::new();
    let f = eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}))["face"].as_u64().unwrap();
    // a 48 MP phone photo and an 8K texture (real pictures; they compress to very little)
    for (w, h) in [(8064u32, 6048u32), (8192, 8192), (16384, 4096)] {
        let p = url(&png(w, h));
        eng(&mut e, json!({"cmd": "texture", "name": format!("t{w}"), "data": p.clone()}));
        eng(&mut e, json!({"cmd": "save_view", "name": format!("v{w}"), "eye": [1, 2, 3], "target": [0, 0, 0], "photo": p}));
    }
    eng(&mut e, json!({"cmd": "paint", "ids": [f], "material": "t8192"}));
    // sizes beyond the limits are refused for every caller and every command that stores a picture, by their header
    for (w, h) in [(30_000u32, 30_000u32), (17_000, 100), (12_000, 12_000)] {
        let p = url(&claimed_png(w, h));
        for caller in [0, 1, 2] {
            let cmds = [
                json!({"cmd": "texture", "name": "big", "data": p}),
                json!({"cmd": "save_view", "name": "big", "eye": [1, 2, 3], "target": [0, 0, 0], "photo": p}),
                json!({"cmd": "match_photo", "name": "big", "width": 100, "height": 100, "red": [[10, 50, 90, 50], [10, 60, 90, 60]], "green": [[50, 90, 50, 10], [60, 90, 60, 10]], "origin": [50, 50], "distance": 5, "photo": p}),
            ];
            for c in cmds {
                let r = match caller {
                    0 => e.exec_json(&c.to_string()),
                    1 => as_agent(&mut e, c.clone()),
                    _ => as_plugin(&mut e, c.clone()),
                };
                assert_eq!(r["ok"], false, "{w}x{h} {c} -> {r}");
                assert!(r["error"].as_str().unwrap().contains("pixels") || r["error"].as_str().unwrap().contains("picture"), "{r}");
            }
        }
    }
    assert!(!e.model.materials.contains_key("big"));
    // something that isn't a PNG or a JPEG isn't stored as a picture
    assert_eq!(e.exec_json(&json!({"cmd": "texture", "name": "svg", "data": "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4="}).to_string())["ok"], false);
    assert_eq!(e.exec_json(&json!({"cmd": "texture", "name": "junk", "data": "data:image/png;base64,AAAA"}).to_string())["ok"], false);
}

#[test]
fn a_48_megapixel_photo_is_read_and_a_render_with_it_works() {
    let p = png(8064, 6048);
    let img = sketchcraft::safety::decode_image(&p, None).expect("48 MP decodes");
    assert_eq!((img.width(), img.height()), (8064, 6048));
    let mut e = Engine::new();
    let f = eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}))["face"].as_u64().unwrap();
    eng(&mut e, json!({"cmd": "texture", "name": "photo", "data": url(&p)}));
    eng(&mut e, json!({"cmd": "paint", "ids": [f], "material": "photo"}));
    let imgs = sketchcraft::texmap::Images::load(&e.model);
    assert!(imgs.get("photo").is_some_and(|i| i.width().max(i.height()) <= 2048), "decoded and shrunk");
    assert!(sketchcraft::safety::take_skipped().is_empty());
}

#[test]
fn pictures_left_out_are_said_never_silent() {
    let dir = scratch("skipped");
    // a render of a model that carries an oversize picture (a file from outside): the reply names it
    let mut e = Engine::new();
    let f = eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}))["face"].as_u64().unwrap();
    eng(&mut e, json!({"cmd": "texture", "name": "ok", "data": url(&png(8, 8))}));
    e.model.materials.get_mut("ok").unwrap().texture.as_mut().unwrap().data = url(&claimed_png(20_000, 20_000));
    eng(&mut e, json!({"cmd": "paint", "ids": [f], "material": "ok"}));
    let out = dir.join("r.png");
    let r = eng(&mut e, json!({"cmd": "render", "out": out, "width": 64, "height": 64}));
    let list = r["skipped_pictures"].as_array().expect("the reply lists the skipped picture");
    assert!(list[0].as_str().unwrap().contains("ok") && list[0].as_str().unwrap().contains("20000"), "{r}");
    // opening a file that holds one takes it out and says so
    let doc = dir.join("o.sketchcraft");
    std::fs::write(&doc, serde_json::to_vec(&e.model).unwrap()).unwrap();
    let mut e2 = Engine::new();
    let r = eng(&mut e2, json!({"cmd": "open", "path": doc}));
    assert!(r["skipped_pictures"].as_array().is_some_and(|a| !a.is_empty()), "{r}");
    assert!(e2.model.materials["ok"].texture.is_none(), "the picture the window would have decoded is not in the model");
    let mut l = Live::open(doc.clone());
    assert!(l.notice.as_deref().unwrap_or("").contains("left out"), "{:?}", l.notice);
    l.housekeeping();
    // a SketchUp import: the oversize texture is skipped with its name, the material keeps its colour
    let skp = skp_fixture::modern_skp_with(Some(&claimed_png(20_000, 20_000)));
    let mut e3 = Engine::new();
    let r = eng(&mut e3, json!({"cmd": "import_skp", "data": b64(&skp)}));
    assert!(r["skipped_pictures"][0].as_str().unwrap().contains("Red"), "{r}");
    assert_eq!(r["faces"], 1);
    // and an ordinary-size one is shrunk and kept
    let skp = skp_fixture::modern_skp_with(Some(&png(3000, 2000)));
    let mut e4 = Engine::new();
    let r = eng(&mut e4, json!({"cmd": "import_skp", "data": b64(&skp)}));
    assert!(r.get("skipped_pictures").is_none(), "{r}");
    assert!(e4.model.materials.values().any(|m| m.texture.is_some()));
    std::fs::remove_dir_all(&dir).ok();
}

// ---- R7-BRN-007 / R7-MBK-005: removing and replacing materials --------------------------------------------

#[test]
fn a_material_and_its_picture_can_be_removed_or_replaced_by_the_person_only() {
    let mut e = Engine::new();
    let f = eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}))["face"].as_u64().unwrap();
    let f2 = eng(&mut e, json!({"cmd": "rect", "from": [4, 0, 0], "to": [6, 2, 0]}))["face"].as_u64().unwrap();
    eng(&mut e, json!({"cmd": "texture", "name": "Brick", "data": url(&png(32, 32))}));
    eng(&mut e, json!({"cmd": "material", "name": "Paint", "color": [0.1, 0.2, 0.3]}));
    eng(&mut e, json!({"cmd": "paint", "ids": [f, f2], "material": "Brick"}));
    // the agent and the plugin don't have it
    for r in [as_agent(&mut e, json!({"cmd": "delete_material", "name": "Brick"})), as_plugin(&mut e, json!({"cmd": "delete_material", "name": "Brick"})), as_agent(&mut e, json!({"cmd": "purge_materials"})), as_plugin(&mut e, json!({"cmd": "purge_materials"}))] {
        assert_eq!(r["ok"], false, "{r}");
    }
    // replace: the faces move to the other material, the picture is gone
    let r = eng(&mut e, json!({"cmd": "delete_material", "name": "Brick", "replace_with": "Paint"}));
    assert_eq!(r["faces"], 2);
    assert!(!e.model.materials.contains_key("Brick"));
    assert!(e.model.faces.values().all(|f| f.material.as_deref() == Some("Paint")));
    assert!(e.model.broken_refs(true).is_empty());
    // one undo step brings it back
    eng(&mut e, json!({"cmd": "undo"}));
    assert!(e.model.materials["Brick"].texture.is_some());
    assert!(e.model.faces.values().all(|f| f.material.as_deref() == Some("Brick")));
    // no replacement: the faces go back to the plain look; a missing replacement is an error that changes nothing
    assert_eq!(e.exec_json(&json!({"cmd": "delete_material", "name": "Brick", "replace_with": "Nope"}).to_string())["ok"], false);
    assert!(e.model.materials.contains_key("Brick"));
    eng(&mut e, json!({"cmd": "delete_material", "name": "Brick"}));
    assert!(e.model.faces.values().all(|f| f.material.is_none()));
    assert_eq!(e.exec_json(&json!({"cmd": "delete_material", "name": "Brick"}).to_string())["ok"], false);
    // unused materials: "Paint" is unused now, a painted one stays
    eng(&mut e, json!({"cmd": "paint", "ids": [f], "color": [1.0, 0.0, 0.0]}));
    eng(&mut e, json!({"cmd": "texture", "name": "Spare", "data": url(&png(16, 16))}));
    let r = eng(&mut e, json!({"cmd": "purge_materials"}));
    assert_eq!(r["removed"], 2, "{r}");
    assert_eq!(e.model.materials.keys().collect::<Vec<_>>(), vec!["#ff0000"]);
}

#[test]
fn a_material_used_inside_a_component_is_replaced_there_too() {
    let mut e = Engine::new();
    let f = eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}))["face"].as_u64().unwrap();
    eng(&mut e, json!({"cmd": "texture", "name": "Wood", "data": url(&png(16, 16))}));
    eng(&mut e, json!({"cmd": "material", "name": "Plain", "color": [0.5, 0.5, 0.5]}));
    eng(&mut e, json!({"cmd": "paint", "ids": [f], "material": "Wood"}));
    let g = eng(&mut e, json!({"cmd": "group", "ids": [f]}))["group"].as_u64().unwrap();
    eng(&mut e, json!({"cmd": "make_component", "ids": [g], "name": "C"}));
    assert!(e.model.definitions.values().any(|d| d.model.material_uses("Wood") > 0));
    eng(&mut e, json!({"cmd": "delete_material", "name": "Wood", "replace_with": "Plain"}));
    assert_eq!(e.model.material_uses("Wood"), 0);
    assert!(e.model.definitions.values().all(|d| !d.model.materials.contains_key("Wood")));
    assert!(e.model.definitions.values().any(|d| d.model.material_uses("Plain") > 0));
}

#[test]
fn an_over_limit_model_can_be_cut_down_by_removing_its_texture() {
    let mut e = Engine::new();
    eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}));
    let mut big = Model::default();
    big.materials.insert("heavy".into(), serde_json::from_value(json!({"color": [1, 1, 1], "texture": {"data": "x".repeat(sketchcraft::model::MAX_MODEL_BYTES + 10), "width": 1, "height": 1}})).unwrap());
    e.replace_model(big).unwrap();
    assert!(e.oversize);
    assert_eq!(e.exec_json(&json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}).to_string())["ok"], false);
    eng(&mut e, json!({"cmd": "delete_material", "name": "heavy"}));
    eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}));
    assert!(!e.oversize);
}

// ---- R7-MBK-006 / R7-MBK-007 / R7-BRN-008 -----------------------------------------------------------------

#[test]
fn an_agents_open_and_the_library_have_their_own_smaller_limits() {
    let dir = scratch("caps");
    let p = dir.join("big.sketchcraft");
    let f = std::fs::File::create(&p).unwrap();
    f.set_len(sketchcraft::commands::MAX_OPEN_BYTES_UNTRUSTED + 1).unwrap(); // sparse
    drop(f);
    let mut e = Engine::new();
    let r = as_agent(&mut e, json!({"cmd": "open", "path": p}));
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["error"].as_str().unwrap().contains("agent"), "{r}");
    let q = dir.join("lib.sketchcraft");
    let f = std::fs::File::create(&q).unwrap();
    f.set_len(sketchcraft::library::MAX_LIBRARY_BYTES + 1).unwrap();
    drop(f);
    assert!(sketchcraft::library::load_file(q.to_str().unwrap()).is_err());
    assert!(sketchcraft::library::thumbnail_of_file(q.to_str().unwrap(), 64).is_err());
    // the person's own open still goes to 768 MB: this one fails only because it isn't JSON, not because of its size
    let r = e.exec_json(&json!({"cmd": "open", "path": p}).to_string());
    assert!(!r["error"].as_str().unwrap_or("").contains("opens models up to"), "{r}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_big_model_notice_appears_after_an_import_too() {
    let dir = scratch("bignotice");
    let mut l = Live::open(dir.join("b.sketchcraft"));
    let r = live(&mut l, json!({"cmd": "terrain_grid", "at": [0, 0, 0], "width": 420, "depth": 420, "spacing": 1}));
    assert!(r.get("save_error").is_none(), "{r}");
    assert!(l.engine.model.entity_count() > 1_000_000, "{}", l.engine.model.entity_count());
    assert!(l.notice.as_deref().unwrap_or("").contains("big model"), "{:?}", l.notice);
    std::fs::remove_dir_all(&dir).ok();
}
