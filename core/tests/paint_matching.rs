#[allow(dead_code)]
mod skp_fixture;

use base64::Engine as _;
use serde_json::{json, Value};
use sketchcraft::Engine;

fn run(engine: &mut Engine, command: Value) -> Value {
    let result = engine.exec_json(&command.to_string());
    assert_eq!(result["ok"], true, "{command}: {result}");
    result
}

#[test]
fn paint_matching_isolates_a_nested_instance_and_undoes_it_as_one_step() {
    let mut engine = Engine::new();
    let data = base64::engine::general_purpose::STANDARD.encode(skp_fixture::modern_skp());
    run(&mut engine, json!({"cmd": "import_skp", "data": data}));
    let (&first, instance) = engine.model.instances.iter().next().unwrap();
    let original = instance.def;
    let outer = &engine.model.definitions[&original].model;
    let (&leaf, instance) = outer.instances.iter().next().unwrap();
    let leaf_model = &engine.model.definitions[&instance.def].model;
    let (&face, _) = leaf_model.faces.iter().next().unwrap();
    let second = run(&mut engine, json!({"cmd":"place", "component":original, "at":[10.,0.,0.]}))["instance"].as_u64().unwrap() as u32;
    let material = |engine: &Engine, owner| {
        engine.model.world_faces().into_iter().find(|f| f.owner == owner).unwrap().material
    };
    let source = material(&engine, first);
    assert_eq!(material(&engine, second), source);
    let scene = sketchcraft::scene::build(&engine.model, None);
    let paths = scene["paint"]["paths"].as_array().unwrap();
    assert!(paths.contains(&json!([first, leaf])) && paths.contains(&json!([second, leaf])));

    let changed = run(&mut engine, json!({"cmd":"paint_matching", "scope":"object", "face":face, "placement":[first,leaf], "color":[0.,0.,1.]}));
    assert_eq!(changed["faces"], 1);
    assert_eq!(material(&engine, first).as_deref(), Some("#0000ff"));
    assert_eq!(material(&engine, second), source);
    assert_ne!(engine.model.instances[&first].def, original);
    assert_eq!(engine.model.instances[&second].def, original);

    run(&mut engine, json!({"cmd":"undo"}));
    assert_eq!(engine.model.instances[&first].def, original);
    assert_eq!(material(&engine, first), source);
    let changed = run(&mut engine, json!({"cmd":"paint_matching", "scope":"all", "face":face, "placement":[first,leaf], "color":[0.,1.,0.]}));
    assert_eq!(changed["faces"], 1, "both placements share the same source definition");
    assert_eq!(material(&engine, first).as_deref(), Some("#00ff00"));
    assert_eq!(material(&engine, second).as_deref(), Some("#00ff00"));
}

#[test]
fn painted_placements_inherit_through_nested_skp_geometry_and_shift_replaces_that_material() {
    let mut engine = Engine::new();
    let data = base64::engine::general_purpose::STANDARD.encode(skp_fixture::modern_skp());
    run(&mut engine, json!({"cmd":"import_skp", "data":data}));
    let (&first, instance) = engine.model.instances.iter().next().unwrap();
    let original = instance.def;
    let (&leaf, nested) = engine.model.definitions[&original].model.instances.iter().next().unwrap();
    let nested = nested.def;
    let (&face, value) = std::sync::Arc::make_mut(&mut engine.model.definitions.get_mut(&nested).unwrap().model)
        .faces.iter_mut().next().unwrap();
    value.material = None;
    let second = run(&mut engine, json!({"cmd":"place","component":original,"at":[10.,0.,0.]}))["instance"].as_u64().unwrap() as u32;
    let loose = run(&mut engine, json!({"cmd":"rect","from":[20.,0.,0.],"to":[22.,2.,0.]}))["face"].as_u64().unwrap() as u32;
    run(&mut engine, json!({"cmd":"material","name":"Blue finish","color":[0.,0.,1.],"roughness":0.27}));
    run(&mut engine, json!({"cmd":"paint_objects","ids":[first],"material":"Blue finish"}));

    let pick = |model: &sketchcraft::model::Model, owner| {
        model.world_faces().into_iter().find(|f| f.owner == owner).unwrap().material
    };
    assert_eq!(pick(&engine.model, first).as_deref(), Some("Blue finish"));
    assert_eq!(pick(&engine.model, second), None);
    assert_eq!(engine.model.instances[&first].def, original, "painting a placement keeps its shared definition");
    assert_eq!(engine.model.definitions[&nested].model.faces[&face].material, None);
    let saved = serde_json::to_vec(&engine.model).unwrap();
    let reopened: sketchcraft::model::Model = serde_json::from_slice(&saved).unwrap();
    assert_eq!(pick(&reopened, first).as_deref(), Some("Blue finish"));
    let scene = sketchcraft::scene::build(&reopened, None);
    let triangle = scene["triFace"].as_array().unwrap().iter().position(|owner| *owner == first).unwrap();
    assert_eq!(scene["pbr"][triangle * 2], json!(0.27));
    assert!(reopened.broken_refs(true).is_empty());

    run(&mut engine, json!({"cmd":"paint_matching","face":face,"placement":[first,leaf],"scope":"all","color":[0.,1.,0.]}));
    assert_eq!(pick(&engine.model, first).as_deref(), Some("#00ff00"));
    assert_eq!(pick(&engine.model, second), None);
    assert_eq!(pick(&engine.model, loose), None);
    run(&mut engine, json!({"cmd":"undo"}));
    assert_eq!(pick(&engine.model, first).as_deref(), Some("Blue finish"));
    run(&mut engine, json!({"cmd":"undo"}));
    assert_eq!(pick(&engine.model, first), None);
}

#[test]
fn object_paint_is_atomic_and_closed_groups_keep_explicit_face_materials() {
    let mut engine = Engine::new();
    let a = run(&mut engine, json!({"cmd":"rect","from":[0.,0.,0.],"to":[1.,1.,0.]}))["face"].as_u64().unwrap() as u32;
    let b = run(&mut engine, json!({"cmd":"rect","from":[2.,0.,0.],"to":[3.,1.,0.]}))["face"].as_u64().unwrap() as u32;
    run(&mut engine, json!({"cmd":"paint","ids":[a],"color":[1.,0.,0.]}));
    let group = run(&mut engine, json!({"cmd":"group","ids":[a,b]}))["group"].as_u64().unwrap() as u32;
    let reply = engine.exec_json(&json!({"cmd":"paint_objects","ids":[group,9999],"color":[0.,0.,1.]}).to_string());
    assert_eq!(reply["ok"], false, "{reply}");
    assert!(engine.model.groups[&group].material.is_none());

    run(&mut engine, json!({"cmd":"paint_objects","ids":[group],"color":[0.,0.,1.]}));
    assert_eq!(engine.model.faces[&a].material.as_deref(), Some("#ff0000"));
    assert_eq!(engine.model.faces[&b].material, None);
    let scene = engine.model.world_faces();
    assert_eq!(scene.iter().find(|f| f.owner == b).unwrap().material.as_deref(), Some("#0000ff"));
    let purge = run(&mut engine, json!({"cmd":"purge_materials"}));
    assert_eq!(purge["removed"], 0);
    run(&mut engine, json!({"cmd":"ungroup","id":group}));
    assert_eq!(engine.model.faces[&b].material.as_deref(), Some("#0000ff"));
}

#[test]
fn shift_default_rewrites_only_matching_nested_placements_and_keeps_editing_bound() {
    let mut engine = Engine::new();
    let data = base64::engine::general_purpose::STANDARD.encode(skp_fixture::modern_skp());
    run(&mut engine, json!({"cmd":"import_skp","data":data}));
    let (&first, instance) = engine.model.instances.iter().next().unwrap();
    let old = instance.def;
    let (&leaf, item) = engine.model.definitions[&old].model.instances.iter().next().unwrap();
    let leaf_def = item.def;
    let (&face, f) = std::sync::Arc::make_mut(&mut engine.model.definitions.get_mut(&leaf_def).unwrap().model)
        .faces.iter_mut().next().unwrap();
    f.material = None;
    let place = |engine: &mut Engine, x| {
        run(engine, json!({"cmd":"place","component":old,"at":[x,0.,0.]}))["instance"].as_u64().unwrap() as u32
    };
    let matching = place(&mut engine, 5.0);
    let painted = place(&mut engine, 10.0);
    run(&mut engine, json!({"cmd":"paint_objects","ids":[painted],"color":[1.,0.,0.]}));
    let group = run(&mut engine, json!({"cmd":"begin_group","name":"Keep protected"}))["group"].as_u64().unwrap() as u32;
    let locked = place(&mut engine, 15.0);
    run(&mut engine, json!({"cmd":"end_group"}));
    run(&mut engine, json!({"cmd":"lock","ids":[group],"locked":true}));
    let loose = run(&mut engine, json!({"cmd":"rect","from":[20.,0.,0.],"to":[22.,2.,0.]}))["face"].as_u64().unwrap() as u32;
    let paint = |engine: &Engine, owner| engine.model.world_faces().into_iter().find(|f| f.owner == owner).unwrap().material;
    let originals = engine.model.definitions.len();

    run(&mut engine, json!({"cmd":"edit","id":first}));
    run(&mut engine, json!({"cmd":"paint_matching","face":face,"placement":[first,leaf],"scope":"all","color":[0.,0.,1.]}));
    for owner in [first, matching, loose] {
        assert_eq!(paint(&engine, owner).as_deref(), Some("#0000ff"));
    }
    assert_eq!(paint(&engine, painted).as_deref(), Some("#ff0000"));
    assert_eq!(paint(&engine, locked), None);
    assert_eq!(engine.model.instances[&painted].def, old);
    assert_eq!(engine.model.instances[&locked].def, old);
    let variant = engine.model.instances[&first].def;
    assert_ne!(variant, old);
    assert_eq!(engine.model.instances[&matching].def, variant);
    assert_eq!(engine.model.definitions.len(), originals + 2, "reuse one new outer and leaf definition");
    assert_eq!(engine.ctx, Some(variant));

    run(&mut engine, json!({"cmd":"undo"}));
    assert_eq!(engine.ctx, Some(old));
    assert_eq!(engine.model.definitions.len(), originals);
    for owner in [first, matching, loose, locked] {
        assert_eq!(paint(&engine, owner), None);
    }
    assert_eq!(paint(&engine, painted).as_deref(), Some("#ff0000"));
    run(&mut engine, json!({"cmd":"redo"}));
    assert_eq!(engine.ctx, Some(variant));
    assert_eq!(paint(&engine, first).as_deref(), Some("#0000ff"));
}

#[test]
fn shift_named_preserves_locked_inherited_faces_and_explicit_overrides() {
    let mut engine = Engine::new();
    let group = run(&mut engine, json!({"cmd":"begin_group","name":"Blue owner"}))["group"].as_u64().unwrap() as u32;
    let rect = |engine: &mut Engine, x: f64| {
        run(engine, json!({"cmd":"rect","from":[x,0.,0.],"to":[x+1.,1.,0.]}))["face"].as_u64().unwrap() as u32
    };
    let inherited = rect(&mut engine, 0.);
    let red_override = rect(&mut engine, 2.);
    run(&mut engine, json!({"cmd":"paint_objects","ids":[red_override],"color":[1.,0.,0.]}));
    let protected = run(&mut engine, json!({"cmd":"begin_group","name":"Protected"}))["group"].as_u64().unwrap() as u32;
    let locked = rect(&mut engine, 4.);
    run(&mut engine, json!({"cmd":"end_group"}));
    run(&mut engine, json!({"cmd":"end_group"}));
    run(&mut engine, json!({"cmd":"paint_objects","ids":[group],"color":[0.,0.,1.]}));
    run(&mut engine, json!({"cmd":"lock","ids":[protected],"locked":true}));
    let explicit = rect(&mut engine, 6.);
    run(&mut engine, json!({"cmd":"paint_objects","ids":[explicit],"color":[0.,0.,1.]}));
    let untouched = rect(&mut engine, 8.);

    run(&mut engine, json!({"cmd":"paint_matching","face":inherited,"scope":"all","color":[0.,1.,0.]}));
    let faces: std::collections::BTreeMap<_,_> = engine.model.world_faces().into_iter().map(|f| (f.owner, f.material)).collect();
    assert_eq!(faces[&inherited].as_deref(), Some("#00ff00"));
    assert_eq!(faces[&explicit].as_deref(), Some("#00ff00"));
    assert_eq!(faces[&locked].as_deref(), Some("#0000ff"));
    assert_eq!(faces[&red_override].as_deref(), Some("#ff0000"));
    assert_eq!(faces[&untouched], None);
}
