//! Replace effective face finishes without painting other occurrences of shared components.

use super::{apply_faces, Id, Material, Model, Result};
use crate::model::{Definition, Instance};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Dynamic component RGB fills are not the model's Default or a stored material.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Finish<'a> {
    Default,
    Material(&'a str),
    DynamicColor,
}

impl<'a> Finish<'a> {
    pub(super) fn apply(self, material: Option<&'a str>) -> Self {
        material.map_or(self, Self::Material)
    }

    pub(super) fn placement(self, root: &'a Model, model: &'a Model, i: &'a Instance, d: &'a Definition) -> Self {
        if let Some(material) = i.material.as_deref() {
            Self::Material(material)
        } else if crate::dynamic::has_material_fill(root, i, d) {
            Self::DynamicColor
        } else {
            self.apply(model.group_material(i.parent))
        }
    }
}

// For one command, a definition can inherit either a matching finish or an
// unrelated one. Its protected occurrences keep the original definition.
type Case = (Id, bool);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Reference {
    Original(Id),
    Changed(Case),
}

#[derive(Default, PartialEq, Eq)]
struct Changes {
    faces: Vec<Id>,
    placements: Vec<(Id, Case)>,
}

struct Plan<'a> {
    root: &'a Model,
    source: Finish<'a>,
    cases: BTreeMap<Case, Reference>,
    changes: BTreeMap<Case, Changes>,
    preserve: BTreeSet<Id>,
}

impl Plan<'_> {
    fn keep(&mut self, id: Id) {
        let mut pending = vec![id];
        while let Some(id) = pending.pop() {
            if !self.preserve.insert(id) {
                continue;
            }
            if let Some(def) = self.root.definitions.get(&id) {
                pending.extend(def.model.instances.values().map(|i| i.def));
            }
        }
    }

    fn definition(&mut self, id: Id, matches: bool, depth: usize) -> Result<Reference> {
        let key = (id, matches);
        if let Some(&existing) = self.cases.get(&key) {
            return Ok(existing);
        }
        if depth > crate::nesting::MAX_NEST {
            anyhow::bail!("component placement path is too deep");
        }
        let def = self.root.definitions.get(&id).ok_or_else(|| anyhow::anyhow!("no component definition {id}"))?;
        let changes = self.model(&def.model, matches, depth)?;
        let result = if changes == Changes::default() {
            self.preserve.insert(id);
            Reference::Original(id)
        } else {
            let other = (id, !matches);
            if self.changes.get(&other) == Some(&changes) {
                Reference::Changed(other)
            } else {
                self.changes.insert(key, changes);
                Reference::Changed(key)
            }
        };
        self.cases.insert(key, result);
        Ok(result)
    }

    fn model(&mut self, model: &Model, inherited_matches: bool, depth: usize) -> Result<Changes> {
        self.root.check_deadline()?;
        // Cache group paint and locks; a large group may have many faces.
        let mut groups = BTreeMap::new();
        let source = self.source;
        let mut group_matches = |group| {
            *groups.entry(group).or_insert_with(|| {
                let matching = model.group_material(group).map_or(inherited_matches, |name| source == Finish::Material(name));
                (matching, model.locked_by(group).is_some())
            })
        };
        let mut faces = Vec::new();
        for (&id, f) in &model.faces {
            let (matches, locked) = group_matches(f.group);
            if !locked && f.material.as_deref().map_or(matches, |name| self.source == Finish::Material(name)) {
                faces.push(id);
            }
        }
        let mut placements = Vec::new();
        for (&id, i) in &model.instances {
            let (matches, locked) = group_matches(i.parent);
            if locked {
                self.keep(i.def);
                continue;
            }
            let def = self.root.definitions.get(&i.def).ok_or_else(|| anyhow::anyhow!("no component definition {}", i.def))?;
            let matches = match i.material.as_deref() {
                Some(name) => self.source == Finish::Material(name),
                None if crate::dynamic::has_material_fill(self.root, i, def) => false,
                None => matches,
            };
            if let Reference::Changed(key) = self.definition(i.def, matches, depth + 1)? {
                placements.push((id, key));
            }
        }
        Ok(Changes { faces, placements })
    }
}

/// The named fast path preserves container slots when no locked descendant
/// could inherit a changed finish. Otherwise paint the matching faces only.
pub(super) fn replace(root: &mut Model, source: Finish<'_>, name: &str, material: &Material) -> Result<usize> {
    let (top, changes, preserve) = {
        let mut plan = Plan {
            root,
            source,
            cases: BTreeMap::new(),
            changes: BTreeMap::new(),
            preserve: BTreeSet::new(),
        };
        let top = plan.model(root, source == Finish::Default, 0)?;
        // Leave unplaced library components and everything they reference intact.
        let visited: BTreeSet<_> = plan.cases.keys().map(|&(id, _)| id).chain(plan.preserve.iter().copied()).collect();
        for &id in root.definitions.keys().filter(|id| !visited.contains(id)) {
            plan.keep(id);
        }
        (top, plan.changes, plan.preserve)
    };

    let source_defs: BTreeMap<_, _> = changes.keys().map(|&(id, _)| (id, root.definitions[&id].clone())).collect();
    let mut target_ids = BTreeMap::new();
    let mut keep = preserve;
    for &key in changes.keys() {
        let id = if keep.insert(key.0) { key.0 } else { root.new_id() };
        target_ids.insert(key, id);
    }

    let update = |model: &mut Model, edits: Changes| {
        let count = edits.faces.len();
        apply_faces(model, &edits.faces, name, material);
        for (id, child) in edits.placements {
            model.instances.get_mut(&id).unwrap().def = target_ids[&child];
        }
        count
    };
    let mut count = update(root, top);
    for (key, edits) in changes {
        let target = target_ids[&key];
        let mut def = source_defs[&key.0].clone();
        count += update(Arc::make_mut(&mut def.model), edits);
        if target != key.0 {
            def.source = None;
        }
        root.definitions.insert(target, def);
    }
    Ok(count)
}
