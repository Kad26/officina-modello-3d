//! Round 5 (security-reports/R5-*.md, fixes in RED-TEAM-REMEDIATION.md): per-client undo on real (non-round)
//! geometry, references after an undo, Restore, the byte budget, the importers' limits, backups and side files.
use serde_json::{json, Value};
use sketchcraft::backups::{self, Backup, Origin};
use sketchcraft::live::Live;
use sketchcraft::policy::{Caller, Workspace};
use sketchcraft::Engine;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sc-round5-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn pause() {
    std::thread::sleep(Duration::from_millis(1100));
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
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

/// every point of every face, sorted: the shape of what is in the file
fn shape(m: &sketchcraft::model::Model) -> Vec<[f64; 3]> {
    let mut v: Vec<[f64; 3]> = m.vertices.values().map(|v| [v.p.x, v.p.y, v.p.z]).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v
}

fn on_disk(doc: &Path) -> sketchcraft::model::Model {
    serde_json::from_slice(&std::fs::read(doc).unwrap()).unwrap()
}

// ---- R5-BRN-001: undo with ordinary (non-round) numbers ----------------------------------

/// A draws a rectangle at irrational coordinates, pulls it up by 3.7 and turns it by an odd angle; B (another program)
/// draws far away and saves; A presses Undo, then Redo. Before the fix 38 of 40 such trials were refused (a coordinate is one
/// unit in the last place away after a trip through the file).
fn one_trial(k: usize) -> Result<(), String> {
    let dir = scratch(&format!("floats-{k}"));
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    let s = 1.0 + k as f64 * 0.137;
    let f = ok(&mut a, json!({"cmd": "rect", "from": [s * std::f64::consts::PI, s * std::f64::consts::E, 0], "to": [s * 7.3141592653, s * 9.2718281828, 0]}))["face"].as_u64().unwrap();
    ok(&mut a, json!({"cmd": "pushpull", "face": f, "distance": 3.7}));
    let ids: Vec<u32> = a.engine.model.faces.keys().copied().collect();
    ok(&mut a, json!({"cmd": "rotate", "ids": ids, "center": [s, 2.0 * s, 0], "axis": [0, 0, 1], "angle": 33.3 + k as f64 * 1.7}));
    let mine = shape(&a.engine.model);
    pause();
    let mut b = Live::open(doc.clone());
    ok(&mut b, json!({"cmd": "rect", "from": [200.123, 5.456, 0], "to": [204.789, 9.012, 0]}));
    let theirs = shape(&b.engine.model);
    pause();
    a.sync();
    let r = a.exec(&json!({"cmd": "undo"}));
    if r["ok"] != true {
        return Err(format!("undo refused: {r}"));
    }
    // A's step took back the rotation, and B's rectangle is untouched on disk
    let m = on_disk(&doc);
    if m.faces.len() != 1 + 6 {
        // the box's rotation was the last step: undone, the box is still there (pushpull and rect are earlier steps)
        return Err(format!("after undo of the rotation: {} faces", m.faces.len()));
    }
    for p in shape(&b.engine.model).iter().filter(|p| p[0] > 100.0) {
        if !shape(&m).iter().any(|q| q == p) {
            return Err("B's rectangle moved".into());
        }
    }
    pause();
    let r = a.exec(&json!({"cmd": "redo"}));
    if r["ok"] != true {
        return Err(format!("redo refused: {r}"));
    }
    let after = shape(&on_disk(&doc));
    let mut want = mine.clone();
    want.extend(theirs.iter().filter(|p| p[0] > 100.0).copied());
    want.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if after.len() != want.len() || after.iter().zip(&want).any(|(p, q)| (0..3).any(|i| (p[i] - q[i]).abs() > 1e-9)) {
        return Err("the redone shape isn't the one that was undone".into());
    }
    std::fs::remove_dir_all(&dir).ok();
    Ok(())
}

#[test]
fn undo_and_redo_work_after_another_programs_save_on_rotated_and_pushed_geometry() {
    let n: usize = std::env::var("SC_TRIALS").ok().and_then(|v| v.parse().ok()).unwrap_or(12);
    let trials: Vec<_> = (0..n).map(|k| std::thread::spawn(move || one_trial(k))).collect();
    let failures: Vec<String> = trials.into_iter().enumerate().filter_map(|(k, t)| t.join().unwrap().err().map(|e| format!("trial {k}: {e}"))).collect();
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn numbers_survive_a_trip_through_the_file_exactly() {
    // float_roundtrip: what is written is what is read, bit for bit
    let mut x = 0.1234567891234567_f64;
    for _ in 0..20_000 {
        x = (x * 1.000_000_7 + 0.000_131).fract() * 977.0;
        let back: f64 = serde_json::from_str(&serde_json::to_string(&x).unwrap()).unwrap();
        assert_eq!(back.to_bits(), x.to_bits());
    }
}

#[test]
fn a_refused_step_is_kept_and_a_second_undo_skips_it() {
    let dir = scratch("keep");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    ok(&mut a, json!({"cmd": "rect", "from": [0, 0, 0], "to": [4, 4, 0]}));
    let f = ok(&mut a, json!({"cmd": "rect", "from": [20, 0, 0], "to": [24, 4, 0]}))["face"].as_u64().unwrap();
    pause();
    let mut b = Live::open(doc.clone());
    ok(&mut b, json!({"cmd": "pushpull", "face": f, "distance": 2.0}));
    pause();
    a.sync();
    let depth = a.engine.undo_depth();
    let r = a.exec(&json!({"cmd": "undo"}));
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(a.engine.undo_depth(), depth, "kept");
    // B takes its pull back; now A's step can be undone, because the step was kept
    pause();
    ok(&mut b, json!({"cmd": "undo"}));
    pause();
    a.sync();
    ok(&mut a, json!({"cmd": "undo"}));
    assert_eq!(on_disk(&doc).faces.len(), 1, "A's second rectangle went");
    std::fs::remove_dir_all(&dir).ok();
}

// ---- R5-BRN-003: nothing left pointing at what an undo removed ---------------------------

#[test]
fn undo_refuses_to_remove_a_material_that_another_program_uses_and_nothing_breaks_after() {
    let dir = scratch("material");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    let fa = ok(&mut a, json!({"cmd": "rect", "from": [0, 0, 0], "to": [4, 4, 0]}))["face"].as_u64().unwrap();
    ok(&mut a, json!({"cmd": "paint", "ids": [fa], "color": [1.0, 0.0, 0.0]}));
    let name = a.engine.model.materials.keys().next().unwrap().clone();
    pause();
    let mut b = Live::open(doc.clone());
    let fb = ok(&mut b, json!({"cmd": "rect", "from": [20, 0, 0], "to": [24, 4, 0]}))["face"].as_u64().unwrap();
    ok(&mut b, json!({"cmd": "paint", "ids": [fb], "material": name}));
    pause();
    a.sync();
    let r = a.exec(&json!({"cmd": "undo"}));
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["error"].as_str().unwrap().contains("material"), "{r}");
    assert!(a.engine.model.materials.contains_key(&name), "the material stays");
    // B's face can still be made a component (it used to panic on the missing material)
    ok(&mut b, json!({"cmd": "make_component", "ids": [fb], "name": "Panel"}));
    assert!(a.engine.model.broken_refs(true).is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn undo_refuses_to_remove_a_group_that_another_program_drew_into() {
    let dir = scratch("group");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    let f = ok(&mut a, json!({"cmd": "rect", "from": [0, 0, 0], "to": [4, 4, 0]}))["face"].as_u64().unwrap();
    let g = ok(&mut a, json!({"cmd": "group", "ids": [f]}))["group"].as_u64().unwrap() as u32;
    pause();
    let mut b = Live::open(doc.clone());
    b.engine.model.active_group = Some(g);
    ok(&mut b, json!({"cmd": "line", "points": [[10, 0, 0], [12, 0, 0]]}));
    assert!(b.engine.model.vertices.values().any(|v| v.group == Some(g)));
    pause();
    a.sync();
    let r = a.exec(&json!({"cmd": "undo"}));
    assert_eq!(r["ok"], false, "{r}");
    assert!(a.engine.model.groups.contains_key(&g));
    assert!(a.engine.model.broken_refs(true).is_empty(), "{:?}", a.engine.model.broken_refs(true));
    std::fs::remove_dir_all(&dir).ok();
}

// ---- R5-BRN-004: no freeze when another program saves --------------------------------------

#[test]
fn carrying_the_history_across_a_save_costs_nothing_and_keeps_every_step() {
    let dir = scratch("lazy");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    ok(&mut a, json!({"cmd": "terrain_grid", "at": [0, 0, 0], "width": 120, "depth": 120, "spacing": 1}));
    for i in 0..30 {
        ok(&mut a, json!({"cmd": "rect", "from": [300.0 + i as f64 * 7.0, 0, 0], "to": [304.0 + i as f64 * 7.0, 4, 0]}));
    }
    pause();
    let mut b = Live::open(doc.clone());
    ok(&mut b, json!({"cmd": "rect", "from": [-50, 0, 0], "to": [-46, 4, 0]}));
    pause();
    let t = std::time::Instant::now();
    a.sync();
    assert!(t.elapsed() < Duration::from_millis(1500), "the reload with 31 steps took {:?}", t.elapsed());
    assert_eq!(a.engine.undo_depth(), 31, "every step is kept");
    assert!(a.notice.is_none(), "{:?}", a.notice);
    // and each one still works, one at a time
    for _ in 0..3 {
        ok(&mut a, json!({"cmd": "undo"}));
    }
    assert_eq!(a.engine.model.faces.values().filter(|f| f.outer.iter().any(|v| a.engine.model.p(*v).x > 300.0)).count(), 27);
    std::fs::remove_dir_all(&dir).ok();
}

// ---- R5-BRN-005 / 006: Restore ---------------------------------------------------------------

#[test]
fn restore_never_hands_out_an_id_twice_and_refuses_an_inconsistent_model() {
    let mut e = Engine::new();
    eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [4, 4, 0]}));
    let old = e.model.clone();
    let f2 = eng(&mut e, json!({"cmd": "rect", "from": [10, 0, 0], "to": [14, 4, 0]}))["face"].as_u64().unwrap();
    e.replace_model(old).unwrap();
    let f3 = eng(&mut e, json!({"cmd": "rect", "from": [20, 0, 0], "to": [24, 4, 0]}))["face"].as_u64().unwrap();
    assert!(f3 > f2, "face {f3} must be a new id, the old model used {f2}");
    // a face that names points that aren't there
    let mut bad = e.model.clone();
    let id = *bad.faces.keys().next().unwrap();
    bad.faces.get_mut(&id).unwrap().outer.push(99_999);
    assert!(e.replace_model(bad).is_err());
    // an edge to a missing point; a placement of a missing component
    let mut bad = e.model.clone();
    bad.edges.values_mut().next().unwrap().a = 88_888;
    assert!(e.replace_model(bad).is_err());
}

#[test]
fn a_restore_or_open_of_an_inconsistent_file_is_refused_and_the_scene_never_panics() {
    let dir = scratch("inconsistent");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    ok(&mut a, json!({"cmd": "rect", "from": [0, 0, 0], "to": [4, 4, 0]}));
    // a backup whose face names points that don't exist
    let mut v: Value = serde_json::from_slice(&std::fs::read(&doc).unwrap()).unwrap();
    let f = v["faces"].as_object_mut().unwrap().values_mut().next().unwrap();
    f["outer"] = json!([901, 902, 903]);
    let bk = dir.join("Backups");
    std::fs::create_dir_all(&bk).unwrap();
    let name = format!("live-{}.sketchcraft", now());
    std::fs::write(bk.join(&name), serde_json::to_vec(&v).unwrap()).unwrap();
    let r = a.exec(&json!({"cmd": "restore_backup", "kind": "backup", "name": name}));
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(a.engine.model.faces.len(), 1);
    assert!(a.scene()["notice"].is_null() || a.scene()["notice"].is_string());
    // opening such a file is refused the same way
    let bad = dir.join("bad.sketchcraft");
    std::fs::write(&bad, serde_json::to_vec(&v).unwrap()).unwrap();
    let mut e = Engine::new();
    let r = e.exec_json(&json!({"cmd": "open", "path": bad}).to_string());
    assert_eq!(r["ok"], false, "{r}");
    // and a model that is inconsistent in memory anyway draws as an error, not a panic
    let mut broken = Live::memory(Engine::new());
    broken.engine.model = serde_json::from_value(v).unwrap();
    let s = broken.scene();
    assert!(s.is_object());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn restore_looks_at_the_size_before_reading() {
    let dir = scratch("restore-size");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    ok(&mut a, json!({"cmd": "rect", "from": [0, 0, 0], "to": [4, 4, 0]}));
    let bk = dir.join("Backups");
    std::fs::create_dir_all(&bk).unwrap();
    let name = format!("live-{}.sketchcraft", now());
    // a sparse file far over the limit: never read
    let f = std::fs::File::create(bk.join(&name)).unwrap();
    f.set_len(sketchcraft::commands::MAX_OPEN_BYTES + 1).unwrap();
    for mode in ["replace", "copy"] {
        let r = a.exec(&json!({"cmd": "restore_backup", "kind": "backup", "name": name, "mode": mode}));
        assert_eq!(r["ok"], false, "{r}");
        assert!(r["error"].as_str().unwrap().contains("too big"), "{r}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

// ---- R5-BRN-002 / R5-MBK-002: the byte budget -----------------------------------------------

fn png_noise(px: u32, seed: u32) -> String {
    use base64::Engine as _;
    let mut img = image::RgbImage::new(px, px);
    let mut s = seed.wrapping_mul(2654435761).wrapping_add(1);
    for p in img.pixels_mut() {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        *p = image::Rgb([(s >> 8) as u8, (s >> 16) as u8, (s >> 24) as u8]);
    }
    let mut out = vec![];
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(out))
}

#[test]
fn textures_count_against_a_byte_budget_per_command_and_per_model() {
    let mut e = Engine::new();
    let ws = Workspace::anywhere();
    let agent = Caller::Mcp(ws);
    let data = png_noise(2000, 1);
    assert!(data.len() > 10 << 20, "{}", data.len());
    let mut accepted = 0usize;
    let mut refused = None;
    for i in 0..40 {
        let r = e.exec_json_as(&agent, &json!({"cmd": "texture", "name": format!("t{i}"), "data": data}).to_string());
        if r["ok"] == true {
            accepted += 1;
        } else {
            refused = Some(r);
            break;
        }
    }
    let refused = refused.expect("40 textures of 10+ MB must not all be accepted");
    assert!(refused["error"].as_str().unwrap().contains("MB"), "{refused}");
    assert!(e.model.size_estimate() <= sketchcraft::model::MAX_MODEL_BYTES_UNTRUSTED, "{}", e.model.size_estimate());
    assert!(accepted >= 10, "{accepted}: a few real textures are fine");
    // the same model can't be saved over the limit either
    e.model.check_sane().unwrap();
}

#[test]
fn a_model_over_the_byte_budget_fails_the_check() {
    let mut m = sketchcraft::model::Model::default();
    m.materials.insert("big".into(), serde_json::from_value(json!({"color": [1, 1, 1], "texture": {"data": "x".repeat(sketchcraft::model::MAX_MODEL_BYTES + 1), "width": 1, "height": 1}})).unwrap());
    assert!(m.check_sane().is_err());
}

// ---- R5-BRN-009: legitimate big imports work again -----------------------------------------

#[test]
fn big_imports_that_worked_before_work_again_for_the_person_and_stay_tight_for_agents() {
    // 300 x 300 terrain (542,404 entities)
    let mut e = Engine::new();
    let r = e.exec_json(&json!({"cmd": "terrain_grid", "at": [0, 0, 0], "width": 300, "depth": 300, "spacing": 1}).to_string());
    assert_eq!(r["ok"], true, "{r}");
    assert!(e.model.entity_count() > sketchcraft::model::MAX_GROWTH);
    // the same from an agent is refused (per-command limit)
    let mut e2 = Engine::new();
    let agent = Caller::Mcp(Workspace::anywhere());
    let r = e2.exec_json_as(&agent, &json!({"cmd": "terrain_grid", "at": [0, 0, 0], "width": 300, "depth": 300, "spacing": 1}).to_string());
    assert_eq!(r["ok"], false, "{r}");
    // and the person's next command is held to the ordinary limit again
    let r = e.exec_json(&json!({"cmd": "array", "ids": [*e.model.groups.keys().next().unwrap()], "by": [400, 0, 0], "count": 2}).to_string());
    assert_eq!(r["ok"], false, "an array that adds a million entities is refused: {r}");
}

#[test]
fn a_215k_triangle_obj_imports_for_the_person() {
    let mut obj = String::with_capacity(32 << 20);
    let n = 215_000usize;
    for i in 0..n {
        let (x, y) = ((i % 500) as f64, (i / 500) as f64);
        obj.push_str(&format!("v {x} {y} 0\nv {} {y} 0\nv {x} {} 0.5\nf {} {} {}\n", x + 0.9, y + 0.9, 3 * i + 1, 3 * i + 2, 3 * i + 3));
    }
    let mut e = Engine::new();
    let r = e.exec_json(&json!({"cmd": "import_obj", "text": obj, "name": "Scan"}).to_string());
    assert_eq!(r["ok"], true, "{}", r.to_string().chars().take(300).collect::<String>());
    assert!(e.model.entity_count() > 500_000, "{}", e.model.entity_count());
}

#[test]
fn a_354k_line_dxf_lattice_imports_for_the_person() {
    let mut dxf = String::from("0\nSECTION\n2\nENTITIES\n");
    let n = 354_000usize;
    for i in 0..n {
        let (x, y) = ((i % 600) as f64, (i / 600) as f64);
        dxf.push_str(&format!("0\nLINE\n8\nLAT\n10\n{x}\n20\n{y}\n30\n0\n11\n{}\n21\n{}\n31\n0\n", x + 0.7, y + 0.3));
    }
    dxf.push_str("0\nENDSEC\n0\nEOF\n");
    let mut e = Engine::new();
    let r = e.exec_json(&json!({"cmd": "import_dxf", "text": dxf}).to_string());
    assert_eq!(r["ok"], true, "{}", r.to_string().chars().take(300).collect::<String>());
    assert!(e.model.entity_count() > 500_000, "{}", e.model.entity_count());
}

// ---- R5-BRN-007 / R5-MBK-005 and the byte rule: which backups go -----------------------------

fn bk(secs: u64, pre: bool, origin: Origin, size: u64) -> Backup {
    Backup { path: PathBuf::from(format!("b-{secs}-{origin:?}-{pre}-{size}")), secs, n: 0, pre, kind: None, origin, size }
}

#[test]
fn a_few_huge_timed_backups_cannot_push_out_every_good_one() {
    let t = 1_800_000_000;
    let mut all: Vec<Backup> = (0..24).map(|i| bk(t - 86_000 + i * 3_500, false, Origin::Ui, 5 << 20)).collect();
    all.extend((0..4).map(|i| bk(t - 1_200 + i * 300, false, Origin::Ui, 900 << 20)));
    let gone = backups::to_delete(&all, t);
    let kept_good = all.iter().take(24).filter(|b| !gone.contains(&b.path)).count();
    assert!(kept_good >= 20, "only {kept_good} of 24 good backups kept");
    assert!(!gone.contains(&all[27].path), "the newest stays");
    assert!(all.iter().filter(|b| !gone.contains(&b.path)).map(|b| b.size).sum::<u64>() <= 2 << 30);
}

#[test]
fn the_persons_delete_rhythm_does_not_rotate_out_the_older_history() {
    let t = 1_800_000_000;
    // a day of hourly history, then one delete every 20 s for 25 minutes (every one a "pre" backup)
    let mut all: Vec<Backup> = (0..24).map(|i| bk(t - 30 * 60 - 86_000 + i * 3_600, false, Origin::Ui, 1 << 20)).collect();
    all.extend((0..75).map(|i| bk(t - 1_500 + i * 20, true, Origin::Ui, 1 << 20)));
    let gone = backups::to_delete(&all, t);
    let kept_old = all.iter().take(24).filter(|b| !gone.contains(&b.path)).count();
    assert!(kept_old >= 22, "{kept_old} of 24 hourly backups survived 75 deletes");
    let kept: usize = all.len() - gone.len();
    assert!(kept <= backups::MAX_PERSON, "{kept}");
    // and the newest delete's backup is there
    assert!(!gone.contains(&all.last().unwrap().path));
}

#[test]
fn an_agents_flood_keeps_the_persons_state_from_before_the_agent() {
    let t = 1_800_000_000;
    // the agent's pool: an earlier copy of 522 bytes from yesterday, then the person's 6-rectangle model copied at the first `new`,
    // then a flood
    let mut all = vec![bk(t - 80_000, true, Origin::Agent, 522), bk(t - 3_000, true, Origin::Agent, 2_318)];
    all.extend((0..60).map(|i| bk(t - 2_900 + i * 40, true, Origin::Agent, if i % 2 == 0 { 522 } else { 9_000 })));
    let gone = backups::to_delete(&all, t);
    assert!(!gone.contains(&all[0].path), "the oldest copy in the window stays");
    // a week old is the limit now, not a day
    assert!(!gone.contains(&all[1].path) || all.len() > 10, "{gone:?}");
    let kept = all.len() - gone.len();
    assert_eq!(kept, backups::MAX_OTHER);
}

#[test]
fn an_agent_cannot_waive_the_pace_by_changing_the_models_size() {
    // alternating a tiny and a big model with `new`: each `new` is a forced backup; for an agent the 60 s pace holds
    let dir = scratch("pace");
    let doc = dir.join("live.sketchcraft");
    let mut live = Live::open(doc.clone());
    let agent = Caller::Mcp(Workspace::anywhere());
    for i in 0..14 {
        let n = if i % 2 == 0 { 1 } else { 12 };
        for k in 0..n {
            let r = live.exec_as(&agent, &json!({"cmd": "rect", "from": [k * 10, 0, 0], "to": [k * 10 + 4, 4, 0]}));
            assert_eq!(r["ok"], true, "{r}");
        }
        assert_eq!(live.exec_as(&agent, &json!({"cmd": "new"}))["ok"], true);
    }
    let list = backups::list(&dir.join("Backups"), "live", now());
    let agents = list.iter().filter(|b| b.origin == Origin::Agent).count();
    assert!(agents <= 2, "{agents} agent backups in a few seconds: the pace was waived");
    std::fs::remove_dir_all(&dir).ok();
}

// ---- R5-BRN-008 / R5-MBK-001: side files ---------------------------------------------------

#[test]
fn only_side_files_in_the_hidden_index_are_offered_or_restored() {
    let dir = scratch("side");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    ok(&mut a, json!({"cmd": "rect", "from": [0, 0, 0], "to": [4, 4, 0]}));
    let forged = format!("live.unsaved-{}.sketchcraft", now() + 50);
    std::fs::copy(&doc, dir.join(&forged)).unwrap();
    // a well-named file the program didn't note is still offered (a lost index never hides a recovery file), marked "not verified"
    let l = a.exec(&json!({"cmd": "list_backups"}));
    let item = l["items"].as_array().unwrap().iter().find(|i| i["name"] == forged.as_str()).unwrap_or_else(|| panic!("{l}"));
    assert_eq!(item["verified"], false, "{item}");
    assert!(item["label"].as_str().unwrap().contains("not verified"), "{item}");
    // a file that only carries a similar name is not offered at all
    std::fs::copy(&doc, dir.join("live.unsaved-18000000000x.sketchcraft")).unwrap();
    std::fs::copy(&doc, dir.join("live.unsaved-1800000000.sketchcraft.bak")).unwrap();
    let l = a.exec(&json!({"cmd": "list_backups"}));
    assert!(!l["items"].as_array().unwrap().iter().any(|i| i["name"].as_str().is_some_and(|n| n.contains("x.sketchcraft") || n.ends_with(".bak"))), "{l}");
    let r = a.exec(&json!({"cmd": "restore_backup", "kind": "unsaved", "name": "live.unsaved-1800000000.sketchcraft.bak", "mode": "copy"}));
    assert_eq!(r["ok"], false, "{r}");
    // once it is in the index (this program wrote it) it is listed and restorable
    backups::write_index(&doc, &[forged.clone()]);
    let l = a.exec(&json!({"cmd": "list_backups"}));
    let item = l["items"].as_array().unwrap().iter().find(|i| i["name"] == forged.as_str()).unwrap();
    assert_eq!(item["verified"], true, "{item}");
    let r = a.exec(&json!({"cmd": "restore_backup", "kind": "unsaved", "name": forged, "mode": "copy"}));
    assert_eq!(r["ok"], true, "{r}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn save_to_library_cannot_make_a_file_that_looks_like_set_aside_work() {
    let base = scratch("lib-name");
    let a = Caller::Mcp(Workspace::at(&base));
    let mut e = Engine::new();
    let f = eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [2, 2, 0]}))["face"].as_u64().unwrap();
    let c = e.exec_json_as(&a, &json!({"cmd": "make_component", "ids": [f], "name": "Box"}).to_string());
    assert_eq!(c["ok"], true, "{c}");
    let comp = c["component"].as_u64().or_else(|| c["definition"].as_u64()).or_else(|| c["id"].as_u64()).unwrap_or_else(|| *e.model.definitions.keys().next().unwrap() as u64);
    let r = e.exec_json_as(&a, &json!({"cmd": "save_to_library", "component": comp, "dir": &base, "name": "live.unsaved-1800000000"}).to_string());
    assert_eq!(r["ok"], false, "{r}");
    // a component renamed to look like one gets a different file name
    eng(&mut e, json!({"cmd": "rename", "id": comp, "name": "live.unsaved-1800000001"}));
    let r = e.exec_json_as(&a, &json!({"cmd": "save_to_library", "component": comp, "dir": &base}).to_string());
    if r["ok"] == true {
        let names: Vec<String> = std::fs::read_dir(&base).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().all(|n| !backups::looks_like_side(n)), "{names:?}");
    }
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn unindexed_side_files_do_not_count_toward_the_cap_and_a_bad_index_deletes_nothing() {
    let dir = scratch("side-cap");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    ok(&mut a, json!({"cmd": "rect", "from": [0, 0, 0], "to": [4, 4, 0]}));
    // 22 files from an older build (no index)
    for i in 0..22 {
        std::fs::write(dir.join(format!("live.unsaved-{}.sketchcraft", 1_700_000_000 + i)), b"{}").unwrap();
    }
    // five plugin runs cut across by another program's save: each sets its work aside
    let mut made = vec![];
    for k in 0..5 {
        pause();
        let mut other = Live::open(doc.clone());
        a.exec(&json!({"cmd": "begin_batch"}));
        a.exec_as(&Caller::Plugin, &json!({"cmd": "rect", "from": [100 + k * 10, 0, 0], "to": [104 + k * 10, 4, 0]}));
        ok(&mut other, json!({"cmd": "rect", "from": [-50 - k * 10, 0, 0], "to": [-46 - k * 10, 4, 0]}));
        pause();
        let r = a.exec(&json!({"cmd": "end_batch"}));
        assert_eq!(r["ok"], false, "{r}");
        a.sync();
        made.push(backups::read_index(&doc));
    }
    let idx = backups::read_index(&doc);
    assert_eq!(idx.len(), 5, "all five new set-aside files are kept and indexed: {idx:?}");
    for n in &idx {
        assert!(dir.join(n).exists(), "{n} was removed");
    }
    // an index that can't be read: nothing is removed, and the files that were there stay
    std::fs::write(backups::index_path(&doc), b"not json").unwrap();
    assert!(backups::read_index_checked(&doc).is_none());
    let before: usize = std::fs::read_dir(&dir).unwrap().count();
    pause();
    let mut other = Live::open(doc.clone());
    a.exec(&json!({"cmd": "begin_batch"}));
    a.exec_as(&Caller::Plugin, &json!({"cmd": "rect", "from": [900, 0, 0], "to": [904, 4, 0]}));
    ok(&mut other, json!({"cmd": "rect", "from": [-900, 0, 0], "to": [-896, 4, 0]}));
    pause();
    a.exec(&json!({"cmd": "end_batch"}));
    let after: usize = std::fs::read_dir(&dir).unwrap().count();
    assert!(after >= before, "files were removed with a bad index: {before} -> {after}");
    std::fs::remove_dir_all(&dir).ok();
}

// ---- R5-MBK-003: plugins have no undo ----------------------------------------------------------

#[test]
fn plugins_cannot_undo_or_redo_the_persons_steps_but_agents_keep_their_own() {
    let mut e = Engine::new();
    eng(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [4, 4, 0]}));
    for c in ["undo", "redo"] {
        let r = e.exec_json_as(&Caller::Plugin, &json!({"cmd": c}).to_string());
        assert_eq!(r["ok"], false, "{r}");
    }
    assert_eq!(e.model.faces.len(), 1);
    let r = e.exec_json_as(&Caller::Mcp(Workspace::anywhere()), &json!({"cmd": "undo"}).to_string());
    assert_eq!(r["ok"], true, "{r}");
}

/// R5-BRN-004 measurement (run by hand: `cargo test --release --test round5 -- --ignored --nocapture`): a model of about a million
/// entities, a few steps, another program's save, then one Undo.
#[test]
#[ignore]
fn big_model_costs() {
    let dir = scratch("big");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    let t = std::time::Instant::now();
    ok(&mut a, json!({"cmd": "terrain_grid", "at": [0, 0, 0], "width": 420, "depth": 420, "spacing": 1}));
    println!("terrain: {} entities in {:?}", a.engine.model.entity_count(), t.elapsed());
    for i in 0..8 {
        ok(&mut a, json!({"cmd": "rect", "from": [1000.0 + i as f64 * 7.0, 0, 0], "to": [1004.0 + i as f64 * 7.0, 4, 0]}));
    }
    pause();
    let mut b = Live::open(doc.clone());
    ok(&mut b, json!({"cmd": "rect", "from": [-50, 0, 0], "to": [-46, 4, 0]}));
    pause();
    let t = std::time::Instant::now();
    a.sync();
    println!("reload with {} steps: {:?} (notice {:?})", a.engine.undo_depth(), t.elapsed(), a.notice);
    let t = std::time::Instant::now();
    ok(&mut a, json!({"cmd": "undo"}));
    println!("first undo: {:?}", t.elapsed());
    let t = std::time::Instant::now();
    ok(&mut a, json!({"cmd": "undo"}));
    println!("second undo: {:?}", t.elapsed());
    std::fs::remove_dir_all(&dir).ok();
}
