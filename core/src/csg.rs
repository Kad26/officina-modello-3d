//! Solid tools: union, subtract, intersect, trim, split and outer shell of
//! closed groups or components.
//!
//! The booleans run on a BSP tree of convex polygons (the classic approach
//! of csg.js, written fresh here): each solid's faces become triangles, the
//! trees clip each other, and the surviving pieces are merged back into
//! clean faces, one plane and material at a time, with a 2D polygon union.

use crate::geom;
use crate::mesh::Mesh;
use crate::model::{Id, Material, Model};
use anyhow::{bail, Result};
use glam::DVec3;
use std::collections::{BTreeMap, HashMap};

const EPS: f64 = 1e-7;
/// A boolean that splits more pieces than this stops with an error instead
/// of filling memory.
const MAX_PIECES: usize = 2_000_000;
const TOO_COMPLEX: &str = "these solids are too complex to combine (over 2 million pieces); simplify them first";
thread_local! {
    static PIECES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
/// Points closer than this are the same point in a result.
const WELD_TOL: f64 = 2.5e-5;

#[derive(Clone, Debug)]
pub struct Poly {
    pub pts: Vec<DVec3>,
    n: DVec3,
    w: f64,
    pub material: Option<String>,
}

impl Poly {
    pub fn new(pts: Vec<DVec3>, material: Option<String>) -> Option<Poly> {
        let n = geom::newell(&pts);
        if n == DVec3::ZERO {
            return None;
        }
        let n = n.normalize();
        Some(Poly { w: n.dot(pts[0]), n, pts, material })
    }
    fn flip(&mut self) {
        self.pts.reverse();
        self.n = -self.n;
        self.w = -self.w;
    }
}

#[derive(Clone, Copy)]
struct Plane {
    n: DVec3,
    w: f64,
}

impl Plane {
    /// Sort `p` into the four lists, splitting it if it spans the plane.
    fn split(&self, p: &Poly, cof: &mut Vec<Poly>, cob: &mut Vec<Poly>, front: &mut Vec<Poly>, back: &mut Vec<Poly>) {
        const CO: u8 = 0;
        const FR: u8 = 1;
        const BK: u8 = 2;
        let mut kind = 0u8;
        let types: Vec<u8> = p
            .pts
            .iter()
            .map(|v| {
                let t = self.n.dot(*v) - self.w;
                let k = if t < -EPS { BK } else if t > EPS { FR } else { CO };
                kind |= k;
                k
            })
            .collect();
        match kind {
            CO => {
                if self.n.dot(p.n) > 0.0 { cof } else { cob }.push(p.clone());
            }
            FR => front.push(p.clone()),
            BK => back.push(p.clone()),
            _ => {
                let (mut f, mut b) = (Vec::new(), Vec::new());
                let k = p.pts.len();
                for i in 0..k {
                    let j = (i + 1) % k;
                    let (ti, tj) = (types[i], types[j]);
                    let (vi, vj) = (p.pts[i], p.pts[j]);
                    if ti != BK {
                        f.push(vi);
                    }
                    if ti != FR {
                        b.push(vi);
                    }
                    if (ti | tj) == 3 {
                        let t = (self.w - self.n.dot(vi)) / self.n.dot(vj - vi);
                        let v = vi.lerp(vj, t);
                        f.push(v);
                        b.push(v);
                    }
                }
                // keep the parent's plane: tiny pieces would compute a noisy one.
                // Slivers under a square micrometer are dropped: splitting them
                // further is how a boolean's work runs away.
                let budget = |n: usize| PIECES.with(|c| {
                    let v = c.get() + n;
                    c.set(v);
                    if v > MAX_PIECES {
                        panic!("{TOO_COMPLEX}");
                    }
                });
                if f.len() >= 3 && geom::newell_raw(&f).length() > 2e-12 {
                    budget(1);
                    front.push(Poly { pts: f, ..p.clone() });
                }
                if b.len() >= 3 && geom::newell_raw(&b).length() > 2e-12 {
                    budget(1);
                    back.push(Poly { pts: b, ..p.clone() });
                }
            }
        }
    }
}

#[derive(Default)]
struct Node {
    plane: Option<Plane>,
    front: Option<Box<Node>>,
    back: Option<Box<Node>>,
    polys: Vec<Poly>,
}

impl Node {
    fn new(polys: Vec<Poly>) -> Node {
        let mut n = Node::default();
        n.build(polys);
        n
    }

    fn invert(&mut self) {
        for p in &mut self.polys {
            p.flip();
        }
        if let Some(pl) = &mut self.plane {
            pl.n = -pl.n;
            pl.w = -pl.w;
        }
        if let Some(f) = &mut self.front {
            f.invert();
        }
        if let Some(b) = &mut self.back {
            b.invert();
        }
        std::mem::swap(&mut self.front, &mut self.back);
    }

    /// The parts of `polys` outside this solid.
    fn clip_polys(&self, polys: Vec<Poly>) -> Vec<Poly> {
        let Some(pl) = self.plane else { return polys };
        let (mut f, mut b) = (Vec::new(), Vec::new());
        for p in &polys {
            let (mut cf, mut cb) = (Vec::new(), Vec::new());
            pl.split(p, &mut cf, &mut cb, &mut f, &mut b);
            f.extend(cf);
            b.extend(cb);
        }
        let f = match &self.front {
            Some(n) => n.clip_polys(f),
            None => f,
        };
        let b = match &self.back {
            Some(n) => n.clip_polys(b),
            None => Vec::new(),
        };
        f.into_iter().chain(b).collect()
    }

    fn clip_to(&mut self, other: &Node) {
        self.polys = other.clip_polys(std::mem::take(&mut self.polys));
        if let Some(f) = &mut self.front {
            f.clip_to(other);
        }
        if let Some(b) = &mut self.back {
            b.clip_to(other);
        }
    }

    fn all(&self) -> Vec<Poly> {
        let mut out = self.polys.clone();
        if let Some(f) = &self.front {
            out.extend(f.all());
        }
        if let Some(b) = &self.back {
            out.extend(b.all());
        }
        out
    }

    fn build(&mut self, polys: Vec<Poly>) {
        if polys.is_empty() {
            return;
        }
        let pl = *self.plane.get_or_insert(Plane { n: polys[0].n, w: polys[0].w });
        let (mut f, mut b) = (Vec::new(), Vec::new());
        for p in &polys {
            let (mut cf, mut cb) = (Vec::new(), Vec::new());
            pl.split(p, &mut cf, &mut cb, &mut f, &mut b);
            self.polys.extend(cf);
            self.polys.extend(cb);
        }
        if !f.is_empty() {
            self.front.get_or_insert_with(Default::default).build(f);
        }
        if !b.is_empty() {
            self.back.get_or_insert_with(Default::default).build(b);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Op {
    Union,
    Subtract,
    Intersect,
}

/// `a op b` on two closed polygon soups.
pub fn boolean(a: Vec<Poly>, b: Vec<Poly>, op: Op) -> Result<Vec<Poly>> {
    // deep trees recurse deeply: give the work a big stack
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || {
            PIECES.with(|c| c.set(0));
            let (mut a, mut b) = (Node::new(a), Node::new(b));
            match op {
                Op::Union => {
                    a.clip_to(&b);
                    b.clip_to(&a);
                    b.invert();
                    b.clip_to(&a);
                    b.invert();
                    a.build(b.all());
                }
                Op::Subtract => {
                    a.invert();
                    a.clip_to(&b);
                    b.clip_to(&a);
                    b.invert();
                    b.clip_to(&a);
                    b.invert();
                    a.build(b.all());
                    a.invert();
                }
                Op::Intersect => {
                    a.invert();
                    b.clip_to(&a);
                    b.invert();
                    a.clip_to(&b);
                    b.clip_to(&a);
                    a.build(b.all());
                    a.invert();
                }
            }
            a.all()
        })
        .map_err(|e| anyhow::anyhow!("couldn't start the boolean: {e}"))?
        .join()
        .map_err(|_| anyhow::anyhow!("{TOO_COMPLEX}"))
}

// ----- solids in the model ---------------------------------------------------

/// The faces of a top-level group or a component instance, in world
/// coordinates, as triangles. Errors if `id` is neither, or isn't closed.
pub fn solid_polys(m: &mut Model, id: Id) -> Result<Vec<Poly>> {
    build_solid_polys(m, id, false)
}

fn operand_faces(m: &Model, id: Id) -> Result<Vec<crate::components::WFace>> {
    let is_group = m.groups.contains_key(&id);
    if !is_group && !m.instances.contains_key(&id) {
        bail!("{id} is not a group or component");
    }
    Ok(m.world_faces().into_iter().filter(|f| if is_group { f.group == Some(id) && f.face.is_some() } else { f.owner == id }).collect())
}

fn build_solid_polys(m: &mut Model, id: Id, require_clean_shell: bool) -> Result<Vec<Poly>> {
    let faces = operand_faces(m, id)?;
    if faces.is_empty() {
        bail!("{id} has no faces");
    }
    let mut face_shells: Vec<usize> = if require_clean_shell { (0..faces.len()).collect() } else { Vec::new() };
    let uses = edge_uses(faces.iter().map(|f| (&f.outer, &f.holes)), &mut face_shells);
    let (open, inconsistent) = inspect_edges(&uses);
    if !open.is_empty() {
        let at = open[0];
        bail!(
            "{} isn't a solid: {} edge{} not shared by exactly two faces (close the holes first; one is near [{:.3}, {:.3}, {:.3}])",
            name_of(m, id),
            open.len(),
            if open.len() == 1 { " is" } else { "s are" },
            at.x,
            at.y,
            at.z
        );
    }
    if require_clean_shell {
        if let Some(at) = inconsistent {
            bail!(
                "{} has inconsistently oriented faces near [{:.3}, {:.3}, {:.3}]: orient the faces and try again",
                name_of(m, id), at.x, at.y, at.z
            );
        }
        let loose = loose_edge_samples(m, id, &uses);
        if loose.count != 0 {
            let suffix = if loose.count == 1 { "" } else { "s" };
            bail!("{} has {} loose edge{suffix}; move or remove the loose geometry and try again", name_of(m, id), loose.count);
        }
    }
    drop(uses);
    let mut out = Vec::new();
    let mut shells: BTreeMap<usize, Vec<Poly>> = BTreeMap::new();
    for (face, f) in faces.into_iter().enumerate() {
        let material = material_key(m, &f);
        // straight runs don't matter to the boolean (the result gets its
        // shared corners back), and they'd triangulate into slivers
        let outer = simplify(&f.outer);
        let holes: Vec<Vec<DVec3>> = f.holes.iter().map(|h| simplify(h)).filter(|h| h.len() >= 3).collect();
        if outer.len() < 3 {
            continue;
        }
        let polygons = if require_clean_shell { shells.entry(face_shells[face]).or_default() } else { &mut out };
        for t in geom::triangulate(&outer, &holes, f.normal) {
            if let Some(p) = Poly::new(t.to_vec(), material.clone()) {
                polygons.push(p);
            }
        }
    }
    if require_clean_shell {
        let shells: Vec<_> = shells.into_values().collect();
        if !shells_are_oriented(&shells) {
            bail!("{} has inward-facing faces or no enclosed volume: orient the faces and try again", name_of(m, id));
        }
        out = shells.into_iter().flatten().collect();
    }
    Ok(out)
}

/// A loop without its corners on straight runs.
fn simplify(l: &[DVec3]) -> Vec<DVec3> {
    let mut c = l.to_vec();
    let mut i = 0;
    while c.len() > 3 && i < c.len() {
        let (a, b, d) = (c[(i + c.len() - 1) % c.len()], c[i], c[(i + 1) % c.len()]);
        let off = (b - a).cross(d - a).length() / (d - a).length().max(1e-12);
        if off < 1e-6 && (b - a).dot(d - b) > 0.0 {
            c.remove(i);
            i = i.saturating_sub(1);
        } else {
            i += 1;
        }
    }
    c
}

/// A face's look as a document material name: its named material if the
/// document has it, else its color (registered as a paint).
pub fn material_key(m: &mut Model, f: &crate::components::WFace) -> Option<String> {
    match (&f.material, f.color) {
        (Some(k), _) if m.materials.contains_key(k) => Some(k.clone()),
        (_, Some(c)) => {
            let key = format!("#{:02x}{:02x}{:02x}", (c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8);
            m.materials.entry(key.clone()).or_insert(Material { color: c, texture: None, opacity: 1.0, ..Default::default() });
            Some(key)
        }
        _ => None,
    }
}

/// A closed prism: `profile` (a flat loop) swept along `along`.
pub fn prism(profile: &[DVec3], along: DVec3, material: Option<String>) -> Vec<Poly> {
    let mut p = profile.to_vec();
    if geom::newell(&p).dot(along) < 0.0 {
        p.reverse(); // counter-clockwise seen from the far end
    }
    let mut out = Vec::new();
    let start: Vec<DVec3> = p.iter().rev().copied().collect();
    let end: Vec<DVec3> = p.iter().map(|q| *q + along).collect();
    out.extend(Poly::new(start, material.clone()));
    out.extend(Poly::new(end, material.clone()));
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        out.extend(Poly::new(vec![a, b, b + along, a + along], material.clone()));
    }
    out
}

pub fn name_of(m: &Model, id: Id) -> String {
    m.groups.get(&id).map(|g| g.name.clone()).or_else(|| m.instances.get(&id).map(|i| i.name.clone())).filter(|n| !n.is_empty()).unwrap_or_else(|| format!("#{id}"))
}

type PointKey = (i64, i64, i64);
type EdgeKey = (PointKey, PointKey);

fn key(p: DVec3) -> PointKey {
    ((p.x * 1e5).round() as i64, (p.y * 1e5).round() as i64, (p.z * 1e5).round() as i64)
}

fn edge_key([a, b]: [DVec3; 2]) -> EdgeKey {
    let (a, b) = (key(a), key(b));
    (a.min(b), a.max(b))
}

struct FaceEdge {
    points: [DVec3; 2],
    count: usize,
    direction: i32,
    first_face: usize,
}

impl FaceEdge {
    fn midpoint(&self) -> DVec3 {
        (self.points[0] + self.points[1]) / 2.0
    }
}

/// Edges without exactly two uses and the first shared edge with the same
/// boundary direction on both faces. Inner loops count as face boundaries.
fn inspect_edges(uses: &HashMap<EdgeKey, FaceEdge>) -> (Vec<DVec3>, Option<DVec3>) {
    let mut open: Vec<_> = uses.values().filter(|e| e.count != 2).map(FaceEdge::midpoint).collect();
    open.sort_by_key(|p| key(*p));
    if std::env::var("CSG_DEBUG").is_ok() {
        for (k, edge) in uses.iter().filter(|(_, e)| e.count != 2) {
            eprintln!("open edge {k:?} used {}", edge.count);
        }
    }
    let inconsistent = uses.values().filter(|e| e.count == 2 && e.direction != 0)
        .map(FaceEdge::midpoint).min_by_key(|p| key(*p));
    (open, inconsistent)
}

fn edge_uses<'a>(
    faces: impl Iterator<Item = (&'a Vec<DVec3>, &'a Vec<Vec<DVec3>>)>,
    face_shells: &mut [usize],
) -> HashMap<EdgeKey, FaceEdge> {
    let mut uses = HashMap::new();
    for (face, (outer, holes)) in faces.enumerate() {
        for l in std::iter::once(outer).chain(holes.iter()) {
            for i in 0..l.len() {
                let points = [l[i], l[(i + 1) % l.len()]];
                let [a, b] = points.map(key);
                if a != b {
                    let edge = uses.entry((a.min(b), a.max(b))).or_insert(FaceEdge {
                        points, count: 0, direction: 0, first_face: face,
                    });
                    if !face_shells.is_empty() && edge.count == 1 {
                        let a = shell_root(face_shells, edge.first_face);
                        let b = shell_root(face_shells, face);
                        face_shells[a.max(b)] = a.min(b);
                    }
                    edge.count += 1;
                    edge.direction += if a < b { 1 } else { -1 };
                }
            }
        }
    }
    for face in 0..face_shells.len() {
        face_shells[face] = shell_root(face_shells, face);
    }
    uses
}

fn shell_root(parents: &mut [usize], mut face: usize) -> usize {
    while parents[face] != face {
        parents[face] = parents[parents[face]];
        face = parents[face];
    }
    face
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum FindingKind {
    NoFaces,
    OpenEdges,
    NonmanifoldEdges,
    LooseEdges,
    InconsistentFaces,
    InvalidShellOrientation,
    AttachedObjects,
    NestedComponent,
}

#[derive(Default)]
struct FindingSamples {
    count: usize,
    edges: BTreeMap<EdgeKey, [[f64; 3]; 2]>,
}

impl FindingSamples {
    fn add_edge(&mut self, points: [DVec3; 2]) {
        self.count += 1;
        self.edges.insert(edge_key(points), points.map(|p| p.to_array()));
        if self.edges.len() > 8 {
            self.edges.pop_last();
        }
    }
}

fn loose_edge_samples(m: &Model, id: Id, uses: &HashMap<EdgeKey, FaceEdge>) -> FindingSamples {
    let mut samples = FindingSamples::default();
    let mut inspect = |points| {
        let key = edge_key(points);
        if key.0 != key.1 && !uses.contains_key(&key) {
            samples.add_edge(points);
        }
    };
    if m.groups.contains_key(&id) {
        for e in m.edges.values().filter(|e| e.group == Some(id)) {
            inspect([m.p(e.a), m.p(e.b)]);
        }
    } else {
        for e in m.world_edges().into_iter().filter(|e| e.owner == id) {
            inspect([e.a, e.b]);
        }
    }
    samples
}

/// Inspect the exact visible shell solid tools use, without repairing or
/// changing it. Keep only a few examples per finding for large imports.
pub fn inspect_solid(m: &Model, id: Id) -> Result<serde_json::Value> {
    let faces = operand_faces(m, id)?;
    let mut findings: BTreeMap<FindingKind, FindingSamples> = BTreeMap::new();
    if faces.is_empty() {
        findings.entry(FindingKind::NoFaces).or_default().count = 1;
    }
    let uses = edge_uses(faces.iter().map(|f| (&f.outer, &f.holes)), &mut []);
    for edge in uses.values() {
        let kind = match edge.count {
            1 => FindingKind::OpenEdges,
            2 if edge.direction != 0 => FindingKind::InconsistentFaces,
            2 => continue,
            _ => FindingKind::NonmanifoldEdges,
        };
        findings.entry(kind).or_default().add_edge(edge.points);
    }
    let loose = loose_edge_samples(m, id, &uses);
    if loose.count != 0 {
        findings.insert(FindingKind::LooseEdges, loose);
    }
    drop(uses);
    drop(faces);

    let volume = if findings.is_empty() {
        // Material registration as part of triangulation must not touch the
        // live model. Use the same closed-shell checks as the solid tools.
        match solid_info(&mut m.clone(), id) {
            Some(volume) => Some(volume),
            None => {
                findings.entry(FindingKind::InvalidShellOrientation).or_default().count = 1;
                None
            }
        }
    } else {
        None
    };

    let nested = m.groups.values().filter(|g| g.parent == Some(id)).count()
        + m.instances.values().filter(|i| i.parent == Some(id)).count();
    if nested > 0 {
        findings.entry(FindingKind::AttachedObjects).or_default().count = nested;
    }
    if let Some(d) = m.instances.get(&id).and_then(|i| m.definitions.get(&i.def)) {
        let nested = d.model.groups.len() + d.model.instances.len();
        if nested > 0 {
            findings.entry(FindingKind::NestedComponent).or_default().count = nested;
        }
    }
    let findings: Vec<_> = findings.into_iter().map(|(kind, samples)| {
        serde_json::json!({"kind": kind, "count": samples.count, "edges": samples.edges.into_values().collect::<Vec<_>>()})
    }).collect();
    Ok(serde_json::json!({"solid": volume.is_some(), "volume": volume, "findings": findings}))
}

/// Volume enclosed by a closed surface (m³).
pub fn volume(polys: &[Poly]) -> f64 {
    signed_volume(polys, DVec3::ZERO).abs()
}

fn signed_volume(polys: &[Poly], origin: DVec3) -> f64 {
    polys
        .iter()
        .flat_map(|p| {
            let first = p.pts[0] - origin;
            (1..p.pts.len() - 1).map(move |k| {
                first.dot((p.pts[k] - origin).cross(p.pts[k + 1] - origin)) / 6.0
            })
        })
        .sum::<f64>()
}

/// For a closed triangle shell, the sum of signed solid angles is ±4π
/// inside and 0 outside. This avoids ray hits on shared triangle edges.
fn shell_contains(shell: &[Poly], point: DVec3) -> bool {
    let angle: f64 = shell.iter().map(|p| {
        let (a, b, c) = (p.pts[0] - point, p.pts[1] - point, p.pts[2] - point);
        let (la, lb, lc) = (a.length(), b.length(), c.length());
        let denominator = la * lb * lc + a.dot(b) * lc + b.dot(c) * la + c.dot(a) * lb;
        2.0 * a.dot(b.cross(c)).atan2(denominator)
    }).sum();
    angle.abs() > 2.0 * std::f64::consts::PI
}

fn shells_are_oriented(shells: &[Vec<Poly>]) -> bool {
    if shells.is_empty() || shells.iter().any(Vec::is_empty) {
        return false;
    }
    let volumes: Vec<_> = shells.iter().map(|s| signed_volume(s, s[0].pts[0])).collect();
    if volumes.iter().any(|v| !v.is_finite() || *v == 0.0) || volumes.iter().sum::<f64>() <= 0.0 {
        return false;
    }
    if shells.len() == 1 {
        return true;
    }
    let bounds: Vec<_> = shells.iter().map(|shell| {
        let first = shell[0].pts[0];
        shell.iter().flat_map(|p| &p.pts).fold((first, first), |(lo, hi), &p| (lo.min(p), hi.max(p)))
    }).collect();
    // Inner boundaries must face into the cavity. A disconnected inward
    // shell cannot pass just because a separate outward shell is larger.
    volumes.iter().enumerate().all(|(i, volume)| {
        let (lo, hi) = bounds[i];
        let point = shells[i][0].pts[0];
        let surrounding = bounds.iter().enumerate().filter(|(j, (outer_lo, outer_hi))| {
            outer_lo.cmplt(lo).all() && outer_hi.cmpgt(hi).all() && shell_contains(&shells[*j], point)
        }).count();
        (surrounding % 2 == 1) == (*volume < 0.0)
    })
}

/// Turn a polygon soup into faces of a new top-level group: pieces on the
/// same plane with the same material are merged back into whole faces.
///
/// Done on one shared set of points so neighboring faces always agree:
/// 1. every corner is welded (25 µm, about SketchUp's 1/1000 inch) to a
///    numbered point, in a fixed order;
/// 2. where a point sits on another piece's edge, that edge gets the point
///    too (no T-junctions);
/// 3. on each plane, edges walked once each way are inside seams and drop
///    out; the rest chain into outlines (counter-clockwise) and holes;
/// 4. a corner on a straight run is dropped only if no face on another
///    plane uses it.
pub fn to_group(m: &mut Model, polys: &[Poly], name: &str) -> Result<Id> {
    if polys.is_empty() {
        bail!("nothing left: the solids don't overlap that way");
    }
    // 1. weld
    let mut pts: Vec<DVec3> = Vec::new();
    let mut grid: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    let cell = |p: DVec3| ((p.x / WELD_TOL).floor() as i64, (p.y / WELD_TOL).floor() as i64, (p.z / WELD_TOL).floor() as i64);
    let mut weld = |p: DVec3| -> usize {
        let c = cell(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(&i) = grid.get(&(c.0 + dx, c.1 + dy, c.2 + dz)).and_then(|v| v.iter().find(|&&i| pts[i].distance(p) <= WELD_TOL)) {
                        return i;
                    }
                }
            }
        }
        pts.push(p);
        grid.entry(c).or_default().push(pts.len() - 1);
        pts.len() - 1
    };
    let mut pieces: Vec<(Vec<usize>, usize)> = Vec::new(); // (loop, plane group)
    let mut planes: Vec<(DVec3, f64, Option<String>)> = Vec::new();
    let mut plane_of: std::collections::BTreeMap<(i64, i64, i64, i64, Option<String>), usize> = Default::default();
    for p in polys {
        let q = |x: f64| (x * 1e4).round() as i64;
        let key = (q(p.n.x), q(p.n.y), q(p.n.z), q(p.w), p.material.clone());
        let g = *plane_of.entry(key).or_insert_with(|| {
            planes.push((p.n, p.w, p.material.clone()));
            planes.len() - 1
        });
        let mut l: Vec<usize> = p.pts.iter().map(|v| weld(*v)).collect();
        l.dedup();
        while l.len() > 1 && l.first() == l.last() {
            l.pop();
        }
        if l.len() >= 3 {
            pieces.push((l, g));
        }
    }
    // 2. T-junctions, against points sorted by x
    let mut by_x: Vec<usize> = (0..pts.len()).collect();
    by_x.sort_by(|a, b| pts[*a].x.total_cmp(&pts[*b].x));
    let xs: Vec<f64> = by_x.iter().map(|i| pts[*i].x).collect();
    for (l, _) in pieces.iter_mut() {
        let mut out = Vec::with_capacity(l.len());
        for i in 0..l.len() {
            let (ia, ib) = (l[i], l[(i + 1) % l.len()]);
            out.push(ia);
            let (a, b) = (pts[ia], pts[ib]);
            let d = b - a;
            let len2 = d.length_squared();
            let (lo, hi) = (a.min(b) - DVec3::splat(1e-5), a.max(b) + DVec3::splat(1e-5));
            let start = xs.partition_point(|x| *x < lo.x);
            let mut on: Vec<(f64, usize)> = Vec::new();
            for &pi in by_x[start..].iter().take_while(|pi| pts[**pi].x <= hi.x) {
                if pi == ia || pi == ib {
                    continue;
                }
                let p = pts[pi];
                if !(p.cmpge(lo).all() && p.cmple(hi).all()) {
                    continue;
                }
                let t = (p - a).dot(d) / len2;
                if t > 0.0 && t < 1.0 && (a + d * t).distance(p) < 1e-5 {
                    on.push((t, pi));
                }
            }
            on.sort_by(|x, y| x.0.total_cmp(&y.0));
            out.extend(on.into_iter().map(|(_, pi)| pi));
        }
        *l = out;
    }
    // which planes use each point (for step 4)
    let mut planes_at: Vec<std::collections::BTreeSet<usize>> = vec![Default::default(); pts.len()];
    for (l, g) in &pieces {
        for &v in l {
            planes_at[v].insert(*g);
        }
    }
    // 3. per plane: cancel seams, chain loops, sort outlines and holes
    let mut faces: Vec<(Vec<DVec3>, Vec<Vec<DVec3>>, Option<String>)> = Vec::new();
    for (g, (n, _, mat)) in planes.iter().enumerate() {
        let (u, v) = geom::basis(*n);
        let p2 = |i: usize| glam::DVec2::new(pts[i].dot(u), pts[i].dot(v));
        let mut count: std::collections::BTreeMap<(usize, usize), i32> = Default::default();
        for (l, _) in pieces.iter().filter(|(_, pg)| *pg == g) {
            for i in 0..l.len() {
                let (a, b) = (l[i], l[(i + 1) % l.len()]);
                if let Some(c) = count.get_mut(&(b, a)).filter(|c| **c > 0) {
                    *c -= 1;
                } else {
                    *count.entry((a, b)).or_default() += 1;
                }
            }
        }
        let mut next: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
        for ((a, b), c) in &count {
            for _ in 0..*c {
                next.entry(*a).or_default().push(*b);
            }
        }
        let turn = |a: usize, b: usize, c: usize| -> f64 {
            let (d1, d2) = (p2(b) - p2(a), p2(c) - p2(b));
            d1.perp_dot(d2).atan2(d1.dot(d2))
        };
        let mut chains: Vec<Vec<usize>> = Vec::new();
        let starts: Vec<usize> = next.keys().copied().collect();
        for s0 in starts {
            while next.get(&s0).map_or(false, |v| !v.is_empty()) {
                let mut chain = vec![s0];
                let (mut prev, mut cur) = (s0, next.get_mut(&s0).unwrap().pop().unwrap());
                let mut guard = 0;
                while cur != s0 && guard < 10_000_000 {
                    guard += 1;
                    chain.push(cur);
                    let Some(outs) = next.get_mut(&cur).filter(|o| !o.is_empty()) else { break };
                    // at a pinch, turn most to the left: keeps each loop simple
                    let k = (0..outs.len()).max_by(|&i, &j| turn(prev, cur, outs[i]).total_cmp(&turn(prev, cur, outs[j]))).unwrap();
                    prev = cur;
                    cur = outs.swap_remove(k);
                }
                if cur == s0 && chain.len() >= 3 {
                    chains.push(chain);
                }
            }
        }
        // 4. straight runs, where only this plane cares about the corner
        for c in &mut chains {
            let mut i = 0;
            while c.len() > 3 && i < c.len() {
                let (a, b, d) = (c[(i + c.len() - 1) % c.len()], c[i], c[(i + 1) % c.len()]);
                let (pa, pb, pd) = (pts[a], pts[b], pts[d]);
                let off_line = (pb - pa).cross(pd - pa).length() / (pd - pa).length().max(1e-12);
                if planes_at[b].len() == 1 && off_line < 1e-5 && (pb - pa).dot(pd - pb) > 0.0 {
                    c.remove(i);
                    i = i.saturating_sub(1);
                } else {
                    i += 1;
                }
            }
        }
        let area = |c: &[usize]| -> f64 { (0..c.len()).map(|i| p2(c[i]).perp_dot(p2(c[(i + 1) % c.len()]))).sum::<f64>() / 2.0 };
        let (mut outers, mut holes): (Vec<(f64, Vec<usize>)>, Vec<Vec<usize>>) = (Vec::new(), Vec::new());
        for c in chains {
            let a = area(&c);
            if a.abs() < 1e-12 {
                continue;
            }
            if a > 0.0 { outers.push((a, c)) } else { holes.push(c) }
        }
        let first = faces.len();
        for (_, o) in &outers {
            faces.push((o.iter().map(|i| pts[*i]).collect(), Vec::new(), mat.clone()));
        }
        for h in holes {
            // the smallest outline around the hole owns it
            let probe = (p2(h[0]) + p2(h[1])) / 2.0;
            let owner = outers
                .iter()
                .enumerate()
                .filter(|(_, (_, o))| {
                    let poly: Vec<glam::DVec2> = o.iter().map(|i| p2(*i)).collect();
                    geom::inside(probe, &poly) || geom::inside(p2(h[0]), &poly)
                })
                .min_by(|a, b| a.1 .0.total_cmp(&b.1 .0))
                .map(|(i, _)| i);
            if let Some(i) = owner {
                faces[first + i].1.push(h.iter().map(|i| pts[*i]).collect());
            }
        }
    }
    let prev = m.active_group;
    let gid = m.begin_group(name.to_string());
    let mut mesh = Mesh::new(m, Some(gid));
    for (outer, holes, mat) in &faces {
        mesh.face(outer, holes, mat.clone());
    }
    mesh.finish();
    m.active_group = prev;
    Ok(gid)
}

/// Run a solid tool on groups / component instances (SketchUp's order:
/// for subtract and trim the first one cuts the second). Returns the
/// resulting group(s).
pub fn solid(m: &mut Model, op: &str, ids: &[Id]) -> Result<Vec<Id>> {
    if matches!(op, "trim" | "subtract") {
        return cut(m, op, ids).map(|group| vec![group]);
    }
    if ids.len() < 2 {
        bail!("pick at least two solids");
    }
    let mut polys = Vec::new();
    for &id in ids {
        check_individual_solid(m, id)?;
        polys.push(build_solid_polys(m, id, true)?);
        if std::env::var("CSG_DEBUG").is_ok() { eprintln!("operand {id}: {} triangles", polys.last().unwrap().len()); }
    }
    let name = |m: &Model, id: Id| name_of(m, id);
    // results land where the shape being worked on sat in the hierarchy
    let keep = ids[0];
    let parent = m.groups.get(&keep).and_then(|g| g.parent).or_else(|| m.instances.get(&keep).and_then(|i| i.parent));
    let out = match op {
        "union" | "outer_shell" => {
            let nm = name(m, ids[0]);
            let mut acc = polys[0].clone();
            for p in &polys[1..] {
                acc = boolean(acc, p.clone(), Op::Union)?;
            }
            m.delete(ids)?;
            vec![to_group(m, &acc, &nm)?]
        }
        "intersect" => {
            let nm = name(m, ids[0]);
            let mut acc = polys[0].clone();
            for p in &polys[1..] {
                acc = boolean(acc, p.clone(), Op::Intersect)?;
            }
            m.delete(ids)?;
            vec![to_group(m, &acc, &nm)?]
        }
        "split" => {
            if ids.len() != 2 {
                bail!("split takes two solids");
            }
            let (na, nb) = (name(m, ids[0]), name(m, ids[1]));
            let a_only = boolean(polys[0].clone(), polys[1].clone(), Op::Subtract)?;
            let b_only = boolean(polys[1].clone(), polys[0].clone(), Op::Subtract)?;
            let both = boolean(polys[0].clone(), polys[1].clone(), Op::Intersect)?;
            m.delete(ids)?;
            let mut out = Vec::new();
            for (p, n) in [(a_only, na.clone()), (b_only, nb), (both, format!("{na} overlap"))] {
                if !p.is_empty() {
                    out.push(to_group(m, &p, &n)?);
                }
            }
            out
        }
        _ => bail!("unknown solid tool {op:?} (union, subtract, intersect, trim, split, outer_shell)"),
    };
    for g in &out {
        if let Some(x) = m.groups.get_mut(g) {
            x.parent = parent;
        }
    }
    Ok(out)
}

/// Replacing an assembly with one mesh would discard children of a group or
/// flatten the contents of a component placement.
fn check_individual_solid(m: &Model, id: Id) -> Result<()> {
    let nested = if let Some(def) = m.instances.get(&id).and_then(|i| m.definitions.get(&i.def)) {
        !def.model.groups.is_empty() || !def.model.instances.is_empty()
    } else {
        m.groups.values().any(|g| g.parent == Some(id)) || m.instances.values().any(|i| i.parent == Some(id))
    };
    if nested {
        bail!("{} contains nested objects: open it and select an individual solid", name_of(m, id));
    }
    Ok(())
}

/// Both cutters work on the target's shell, leaving attached hardware editable.
/// Trim keeps the cutter and Subtract consumes it.
fn cut(m: &mut Model, op: &str, ids: &[Id]) -> Result<Id> {
    let &[cutter, target] = ids else {
        bail!("{op} takes two different solids: the cutter, then the one to cut");
    };
    if cutter == target {
        bail!("{op} takes two different solids: the cutter, then the one to cut");
    }
    let removed = if op == "trim" { &ids[1..] } else { ids };
    m.check_unlocked(removed)?;
    check_individual_solid(m, cutter)?;
    // Group targets keep their attached objects on the replacement group.
    if m.instances.contains_key(&target) {
        check_individual_solid(m, target)?;
    }
    let cutter_polys = build_solid_polys(m, cutter, true)?;
    let target_polys = build_solid_polys(m, target, true)?;
    let origin = target_polys[0].pts[0];
    let before = signed_volume(&target_polys, origin);
    let result = boolean(target_polys, cutter_polys, Op::Subtract)?;
    // A disjoint or merely touching cutter leaves the target's volume intact.
    // Refuse that no-op before Subtract can consume the user's cutter.
    let after = signed_volume(&result, origin);
    let tolerance = before.abs() * 1e-9;
    if !after.is_finite() || after < -tolerance {
        bail!("{op} could not produce a valid result");
    }
    if after > before + tolerance {
        bail!("{op} would increase the target volume; check the two solids");
    }
    if before - after <= tolerance {
        bail!("the cutter and the target do not overlap in volume");
    }
    let name = name_of(m, target);
    let replacement = to_group(m, &result, &name)?;
    if let Some(group) = m.groups.get(&target).cloned() {
        m.groups.insert(replacement, group);
        for child in m.groups.values_mut().filter(|g| g.parent == Some(target)) {
            child.parent = Some(replacement);
        }
        for child in m.instances.values_mut().filter(|i| i.parent == Some(target)) {
            child.parent = Some(replacement);
        }
    } else if let Some(instance) = m.instances.get(&target) {
        let group = m.groups.get_mut(&replacement).unwrap();
        group.parent = instance.parent;
        group.tag = instance.tag.clone();
    }
    if m.active_group == Some(target) {
        m.active_group = Some(replacement);
    }
    let class = m.classes.remove(&target);
    m.delete(removed)?;
    if op == "subtract" {
        m.classes.remove(&cutter);
    }
    if let Some(class) = class {
        m.classes.insert(replacement, class);
    }
    Ok(replacement)
}

/// Is this group / instance a closed solid, and how big is it?
pub fn solid_info(m: &mut Model, id: Id) -> Option<f64> {
    let polys = build_solid_polys(m, id, true).ok()?;
    polys.first().map(|p| signed_volume(&polys, p.pts[0]))
}

