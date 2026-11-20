//! Per-client undo (R3-BRN-011, owner decision): the person's own history survives another
//! program's save, and Undo never takes back the other program's work (R2-BRN-001).
use serde_json::{json, Value};
use sketchcraft::live::Live;
use sketchcraft::policy::Caller;
use sketchcraft::Engine;
use std::path::PathBuf;
use std::time::Duration;

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sc-undo-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn pause() {
    std::thread::sleep(Duration::from_millis(1100));
}

fn rect(l: &mut Live, x: f64) -> Value {
    let r = l.exec(&json!({"cmd": "rect", "from": [x, 0, 0], "to": [x + 4.0, 4, 0]}));
    assert_eq!(r["ok"], true, "{r}");
    r
}

/// the x of every face on disk (the left edge of each rectangle), sorted
fn xs_on_disk(doc: &std::path::Path) -> Vec<i64> {
    let mut e = Engine::new();
    let r = e.exec_json(&json!({"cmd": "open", "path": doc}).to_string());
    assert_eq!(r["ok"], true, "{r}");
    let mut xs: Vec<i64> = e.model.faces.values().map(|f| f.outer.iter().map(|v| e.model.p(*v).x).fold(f64::MAX, f64::min).round() as i64).collect();
    xs.sort();
    xs
}

fn xs_in_memory(l: &Live) -> Vec<i64> {
    let mut xs: Vec<i64> = l.engine.model.faces.values().map(|f| f.outer.iter().map(|v| l.engine.model.p(*v).x).fold(f64::MAX, f64::min).round() as i64).collect();
    xs.sort();
    xs
}

#[test]
fn two_clients_interleave_edits_and_each_undoes_only_its_own() {
    let dir = scratch("interleave");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    rect(&mut a, 0.0);
    rect(&mut a, 10.0);
    pause();
    let mut b = Live::open(doc.clone());
    rect(&mut b, 20.0);
    pause();
    rect(&mut a, 30.0); // A picks up B's rectangle first, then adds its own
    assert_eq!(xs_in_memory(&a), vec![0, 10, 20, 30]);
    pause();
    rect(&mut b, 40.0);
    pause();
    // A undoes twice: its 30 and its 10; never B's 20 or 40
    assert_eq!(a.exec(&json!({"cmd": "undo"}))["ok"], true);
    assert_eq!(xs_on_disk(&doc), vec![0, 10, 20, 40], "A's last step went, B's 40 (picked up first) stayed");
    pause();
    assert_eq!(a.exec(&json!({"cmd": "undo"}))["ok"], true);
    assert_eq!(xs_on_disk(&doc), vec![0, 20, 40]);
    // A redoes: its 10 is back, and nothing of B's moved
    pause();
    assert_eq!(a.exec(&json!({"cmd": "redo"}))["ok"], true);
    assert_eq!(xs_on_disk(&doc), vec![0, 10, 20, 40]);
    pause();
    // B undoes: its own last step (40); B's earlier 20 is a step of B's too
    assert_eq!(b.exec(&json!({"cmd": "undo"}))["ok"], true);
    assert_eq!(xs_on_disk(&doc), vec![0, 10, 20], "only B's rectangle at 40 went");
    pause();
    assert_eq!(b.exec(&json!({"cmd": "undo"}))["ok"], true);
    assert_eq!(xs_on_disk(&doc), vec![0, 10], "and then B's 20");
    // A's first step (the 0) is still A's to undo, but A has 10 redone on top of it
    pause();
    assert_eq!(a.exec(&json!({"cmd": "undo"}))["ok"], true);
    pause();
    let r = a.exec(&json!({"cmd": "undo"}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(xs_on_disk(&doc), Vec::<i64>::new());
    assert_eq!(a.exec(&json!({"cmd": "undo"}))["ok"], false, "nothing left of A's");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_step_that_the_other_client_built_on_is_refused_and_nothing_of_theirs_is_lost() {
    let dir = scratch("conflict");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    let f = rect(&mut a, 0.0)["face"].as_u64().unwrap();
    pause();
    let mut b = Live::open(doc.clone());
    // B pulls A's rectangle up into a box and draws another
    assert_eq!(b.exec(&json!({"cmd": "pushpull", "face": f, "distance": 3.0}))["ok"], true);
    rect(&mut b, 20.0);
    pause();
    let before = std::fs::read(&doc).unwrap();
    // A picks up B's work, then tries to undo its rectangle: that rectangle is now the base of B's box
    a.sync();
    assert_eq!(a.engine.undo_depth(), 1, "A's step is kept");
    let r = a.exec(&json!({"cmd": "undo"}));
    assert_eq!(r["ok"], false, "{r}");
    let msg = r["error"].as_str().unwrap();
    assert!(msg.contains("Undo stopped") && msg.contains("changed"), "{msg}");
    assert_eq!(std::fs::read(&doc).unwrap(), before, "the file is untouched");
    assert_eq!(a.engine.undo_depth(), 1, "the refused step is kept: the other program may take its change back");
    assert!(msg.contains("press Undo again to skip"), "{msg}");
    // asking again skips it (nothing further back: said so, and the list is empty)
    let again = a.exec(&json!({"cmd": "undo"}));
    assert_eq!(again["ok"], false, "{again}");
    assert_eq!(a.engine.undo_depth(), 0, "the second Undo let the step go, so Undo isn't stuck behind it");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_neighbour_drawn_against_a_step_blocks_its_undo() {
    let dir = scratch("neighbour");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    rect(&mut a, 0.0);
    pause();
    let mut b = Live::open(doc.clone());
    // B draws a rectangle that shares an edge with A's: it uses A's points and edge without changing them
    b.exec(&json!({"cmd": "rect", "from": [4, 0, 0], "to": [8, 4, 0]}));
    pause();
    a.sync();
    let r = a.exec(&json!({"cmd": "undo"}));
    assert_eq!(r["ok"], false, "undoing A's rectangle would leave B's hanging on its points: {r}");
    assert_eq!(xs_on_disk(&doc), vec![0, 4], "both rectangles are still there");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_plugin_run_is_one_step_across_a_reload() {
    let dir = scratch("batch");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    let mut plugin = |l: &mut Live, v: Value| {
        let r = l.exec_as(&Caller::Plugin, &v);
        assert_eq!(r["ok"], true, "{r}");
    };
    a.exec(&json!({"cmd": "begin_batch"}));
    for i in 0..5 {
        plugin(&mut a, json!({"cmd": "rect", "from": [i * 10, 0, 0], "to": [i * 10 + 4, 4, 0]}));
    }
    a.exec(&json!({"cmd": "end_batch"}));
    pause();
    let mut b = Live::open(doc.clone());
    rect(&mut b, 100.0);
    pause();
    a.sync();
    assert_eq!(a.engine.undo_depth(), 1, "the five commands are one step");
    assert_eq!(a.exec(&json!({"cmd": "undo"}))["ok"], true);
    assert_eq!(xs_on_disk(&doc), vec![100], "the whole run went, B's rectangle stayed");
    pause();
    assert_eq!(a.exec(&json!({"cmd": "redo"}))["ok"], true);
    assert_eq!(xs_on_disk(&doc), vec![0, 10, 20, 30, 40, 100]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn single_client_undo_is_unchanged_and_fast_on_a_big_model() {
    let dir = scratch("single");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    let t = a.exec(&json!({"cmd": "terrain_grid", "at": [0, 0, 0], "width": 150, "depth": 150, "spacing": 1}));
    assert_eq!(t["ok"], true, "{t}");
    let started = std::time::Instant::now();
    for i in 0..6 {
        rect(&mut a, 200.0 + i as f64 * 10.0);
    }
    for _ in 0..6 {
        assert_eq!(a.exec(&json!({"cmd": "undo"}))["ok"], true);
    }
    for _ in 0..6 {
        assert_eq!(a.exec(&json!({"cmd": "redo"}))["ok"], true);
    }
    // 18 steps on a 22,000-face model, each one saving the file: seconds, not a minute
    assert!(started.elapsed().as_secs() < 30, "{:?}", started.elapsed());
    // the history is plain copies until another program saves: depth counts steps
    assert_eq!(a.engine.undo_depth(), 7);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn patches_count_against_the_undo_memory_budget() {
    let dir = scratch("budget");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    a.engine.undo_budget = 6_000;
    for i in 0..12 {
        rect(&mut a, i as f64 * 10.0);
    }
    pause();
    let mut b = Live::open(doc.clone());
    rect(&mut b, 500.0);
    pause();
    a.sync();
    let depth = a.engine.undo_depth();
    assert!(depth >= 1 && depth <= 12, "{depth}");
    // more steps after the reload still respect the budget
    for i in 0..30 {
        rect(&mut a, 600.0 + i as f64 * 10.0);
    }
    assert!(a.engine.undo_depth() < 42, "{}", a.engine.undo_depth());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn redo_survives_a_reload_too() {
    let dir = scratch("redo");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    rect(&mut a, 0.0);
    rect(&mut a, 10.0);
    assert_eq!(a.exec(&json!({"cmd": "undo"}))["ok"], true); // 10 is now redoable
    pause();
    let mut b = Live::open(doc.clone());
    rect(&mut b, 50.0);
    pause();
    a.sync();
    let r = a.exec(&json!({"cmd": "redo"}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(xs_on_disk(&doc), vec![0, 10, 50]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn restore_from_a_backup_is_one_undoable_step_and_keeps_what_it_replaced() {
    let dir = scratch("restore");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    rect(&mut a, 0.0);
    rect(&mut a, 10.0);
    rect(&mut a, 20.0);
    // a destructive step makes a "before New" backup
    assert_eq!(a.exec(&json!({"cmd": "new"}))["ok"], true);
    rect(&mut a, 77.0);
    let l = a.exec(&json!({"cmd": "list_backups"}));
    assert_eq!(l["ok"], true, "{l}");
    let items = l["items"].as_array().unwrap();
    let pre = items.iter().find(|i| i["label"].as_str().unwrap().contains("before New")).unwrap_or_else(|| panic!("{l}"));
    assert!(pre["size"].as_u64().unwrap() > 0 && pre["secs"].as_u64().unwrap() > 0);
    let r = a.exec(&json!({"cmd": "restore_backup", "kind": "backup", "name": pre["name"], "mode": "replace"}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(xs_on_disk(&doc), vec![0, 10, 20], "the model is back");
    // what it replaced (the 77) is now in the backups
    let l = a.exec(&json!({"cmd": "list_backups"}));
    assert!(l["items"].as_array().unwrap().iter().any(|i| i["label"].as_str().unwrap().contains("before a restore")), "{l}");
    // and Undo takes the restore back
    pause();
    assert_eq!(a.exec(&json!({"cmd": "undo"}))["ok"], true);
    assert_eq!(xs_on_disk(&doc), vec![77]);
    // a copy goes beside the model and changes nothing
    let r = a.exec(&json!({"cmd": "restore_backup", "kind": "backup", "name": pre["name"], "mode": "copy"}));
    assert_eq!(r["ok"], true, "{r}");
    let copy = PathBuf::from(r["path"].as_str().unwrap());
    assert!(copy.starts_with(dir.join("Restored")) && copy.exists());
    assert_eq!(xs_on_disk(&doc), vec![77]);
    // names are checked: no paths, no other files, no other callers
    for bad in ["../live.sketchcraft", "live.sketchcraft", "Backups/x", "live-1.sketchcraft", ".hidden", ""] {
        let r = a.exec(&json!({"cmd": "restore_backup", "kind": "backup", "name": bad}));
        assert_eq!(r["ok"], false, "{bad:?} -> {r}");
    }
    let r = a.exec_as(&Caller::Plugin, &json!({"cmd": "restore_backup", "kind": "backup", "name": pre["name"]}));
    assert_eq!(r["ok"], false);
    let r = a.exec_as(&Caller::Plugin, &json!({"cmd": "list_backups"}));
    assert_eq!(r["ok"], false);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_command_whose_result_is_thrown_away_leaves_no_step_and_no_trace() {
    let mut e = Engine::new();
    let run = |e: &mut Engine, v: Value| {
        let r = e.exec_json(&v.to_string());
        assert_eq!(r["ok"], true, "{r}");
    };
    run(&mut e, json!({"cmd": "rect", "from": [0, 0, 0], "to": [4, 4, 0]}));
    let depth = e.undo_depth();
    run(&mut e, json!({"cmd": "rect", "from": [10, 0, 0], "to": [14, 4, 0]}));
    assert_eq!(e.model.faces.len(), 2);
    e.drop_steps_after(depth);
    assert_eq!((e.model.faces.len(), e.undo_depth()), (1, depth), "back to before that command, with no step for it");
}

/// R8-BRN-004: the group a program is editing is that program's own state. Another program that opens the file while a
/// group is open draws outside it, so the first program can still take back making that group (it was refused as
/// "another program changed it" because the other program's far-away rectangle had landed inside the group).
#[test]
fn another_program_does_not_draw_into_the_group_being_edited() {
    let dir = scratch("editing");
    let doc = dir.join("live.sketchcraft");
    let mut a = Live::open(doc.clone());
    let f = rect(&mut a, 0.0)["face"].as_u64().unwrap();
    assert_eq!(a.exec(&json!({"cmd": "pushpull", "face": f, "distance": 2.0}))["ok"], true);
    let g = a.exec(&json!({"cmd": "group", "ids": [f]}))["group"].as_u64().unwrap();
    assert_eq!(a.exec(&json!({"cmd": "edit", "id": g}))["ok"], true);
    pause();
    let mut b = Live::open(doc.clone());
    assert_eq!(b.engine.model.active_group, None, "the other program does not start inside A's open group");
    rect(&mut b, 400.0);
    pause();
    // A picks up B's save and is still editing its own group
    assert_eq!(a.exec(&json!({"cmd": "rect", "from": [20, 20, 0], "to": [22, 22, 0]}))["ok"], true);
    assert_eq!(a.engine.model.active_group, Some(g as _), "A stays in its group across the other program's save");
    assert_eq!(a.exec(&json!({"cmd": "close"}))["ok"], true);
    // B's rectangle belongs to no group
    let theirs: Vec<_> = a.engine.model.faces.values().filter(|fc| a.engine.model.p(fc.outer[0]).x >= 400.0).collect();
    assert_eq!(theirs.len(), 1);
    assert!(theirs[0].group.is_none(), "B's rectangle is not inside A's group");
    // A takes back every one of its steps; none is refused, and B's rectangle stays
    for n in 0..20 {
        if a.engine.undo_depth() == 0 {
            break;
        }
        let r = a.exec(&json!({"cmd": "undo"}));
        assert_eq!(r["ok"], true, "undo {n}: {r}");
        pause_short();
    }
    assert_eq!(xs_on_disk(&doc), vec![400], "only the other program's rectangle is left");
    std::fs::remove_dir_all(&dir).ok();
}

fn pause_short() {
    std::thread::sleep(Duration::from_millis(30));
}
