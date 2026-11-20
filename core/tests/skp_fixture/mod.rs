//! A hand-built modern .skp with a component inside a component (shared by skp.rs and round7.rs).
#![allow(dead_code)]

pub struct W(pub Vec<u8>);

impl W {
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    pub fn f64(&mut self, v: f64) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    pub fn class(&mut self, name: &str) -> &mut Self {
        self.u16(0xFFFF).u16(0).u16(name.len() as u16);
        self.0.extend(name.as_bytes());
        self
    }
    pub fn ustr(&mut self, s: &str) -> &mut Self {
        self.0.extend([0xFF, 0xFE, 0xFF, s.encode_utf16().count() as u8]);
        for u in s.encode_utf16() {
            self.u16(u);
        }
        self
    }
    pub fn header(&mut self) -> &mut Self {
        self.0.extend([0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0]);
        self
    }
    pub fn vertex(&mut self, x: f64, y: f64, z: f64) -> &mut Self {
        self.u16(0).f64(x).f64(y).f64(z)
    }
    /// SketchUp 2017+: an entity's id as a mask of its non-zero bytes, then those bytes.
    pub fn id(&mut self, id: u32) -> &mut Self {
        let b = id.to_le_bytes();
        let mask = (0..4).fold(0u8, |m, i| if b[i] != 0 { m | 1 << i } else { m });
        self.u8(mask);
        for (i, v) in b.iter().enumerate() {
            if mask & 1 << i != 0 {
                self.u8(*v);
            }
        }
        self
    }
    /// The 10 bytes after an entity's id: material, flags, the 01 01 marker.
    pub fn rest(&mut self, material: u16) -> &mut Self {
        self.u16(material).u8(0).u8(1).u8(1).0.extend([0u8; 5]);
        self
    }
}

/// A SketchUp 2017-2020 file in the classic layout: every entity carries an
/// id after its (null) attribute slot, and the face has a dictionary hanging
/// off it, which takes object indices. One component definition holds a
/// right triangle (100 in legs); the model places it 50 inches along x. The
/// placement names the definition by its object index, which is two below
/// the list header's number because of the dictionary.
pub fn classic_2017_skp() -> Vec<u8> {
    let mut w = W(Vec::new());
    w.0.extend([0xFF, 0xFE, 0xFF, 0x0E]);
    for u in "SketchUp Model".encode_utf16() {
        w.u16(u);
    }
    w.ustr("{17.0.1}");
    w.0.extend([0u8; 64]);
    // the definition's list header: [x][0][count]; its first class takes x + 1
    w.u32(10).u16(0).u32(1);
    // face: class 11, object 12
    w.class("CFace");
    // its dictionary: container class 13, object 14; a named set, class 15, object 16
    w.class("CAttributeContainer").u16(0).u8(0);
    w.class("CAttributeNamed").u16(0).u8(0).u32(0);
    w.ustr("SU_Test").ustr("k").u8(10).ustr("v").ustr("").u32(0);
    w.u16(0); // no more attribute sets
    w.id(0x0135).rest(0);
    w.f64(0.0).f64(0.0).f64(1.0).f64(0.0).u32(1);
    // loop: class 17, object 18
    w.class("CLoop").u16(0).u8(0).u8(1).u8(1);
    // edge use (19, 20) with an edge (21, 22) from a vertex (23, 24) to a new one (25)
    w.class("CEdgeUse").u16(0).u8(0);
    w.class("CEdge").u16(0).id(7).rest(0);
    w.class("CVertex").u16(0).id(1).f64(0.0).f64(0.0).f64(0.0);
    w.u16(0x8000 | 23).u16(0).id(2).f64(100.0).f64(0.0).f64(0.0);
    w.u16(0).u8(0).u16(18);
    // edge use 2 (26) with an edge (27) from the vertex 25 to a new one (28)
    w.u16(0x8000 | 19).u16(0).u8(0);
    w.u16(0x8000 | 21).u16(0).id(8).rest(0);
    w.u16(25);
    w.u16(0x8000 | 23).u16(0).id(3).f64(0.0).f64(100.0).f64(0.0);
    w.u16(0).u8(0).u16(18);
    // edge use 3 (29) with an edge (30) closing the triangle
    w.u16(0x8000 | 19).u16(0).u8(0);
    w.u16(0x8000 | 21).u16(0).id(9).rest(0);
    w.u16(28).u16(24).u16(0);
    w.u8(0).u16(18);
    w.u16(0); // end of loop
    w.u16(0); // back material
    w.0.extend([0u8; 32]);
    // the model's own list: one instance (class 31)
    w.u32(1);
    w.class("CComponentInstance").u16(0).id(0x10).rest(0);
    w.u16(8);
    for v in [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 50.0, 0.0, 0.0, 1.0] {
        w.f64(v);
    }
    w.ustr("");
    w.0.extend([0x5au8; 16]); // its GUID
    w.0.extend([0u8; 32]);
    w.0
}

fn rec(tag: u16, body: &[u8]) -> Vec<u8> {
    let mut v = tag.to_le_bytes().to_vec();
    v.extend((body.len() as u32).to_le_bytes());
    v.extend(body);
    v
}

fn recs(tag: u16, kids: &[Vec<u8>]) -> Vec<u8> {
    rec(tag, &kids.concat())
}

fn with_id(id: u8) -> Vec<u8> {
    recs(0x5dc, &[rec(0x5de, &[id])])
}

pub fn modern_skp() -> Vec<u8> {
    modern_skp_with(None)
}

pub fn modern_skp_with_views() -> Vec<u8> {
    build(None, false, Mode::Views)
}

pub fn modern_skp_with_inherited_finishes() -> Vec<u8> {
    build(Some(&tiny_png()), true, Mode::InheritedFinishes)
}

pub fn modern_skp_with_positioned_texture() -> Vec<u8> {
    build(Some(&tiny_png()), false, Mode::PositionedTexture)
}

/// The same file, with a picture (`bytes`) as the Red material's image.
pub fn modern_skp_with(texture: Option<&[u8]>) -> Vec<u8> {
    build(texture, false, Mode::Plain)
}

/// The same model laid out as SketchUp 27 saves it: placements listed in
/// 0x138c with an empty 0x1965, naming their definition by the id of its
/// entities record (0x1967), and 0x157e holding the definition's name.
pub fn su27_skp() -> Vec<u8> {
    build(None, true, Mode::Plain)
}

/// A named edge-only profile inside a placed assembly.
pub fn wire_profile_skp() -> Vec<u8> {
    build(None, true, Mode::WireProfile)
}

/// SketchUp 27 again, but the model's only placement is a picture (an image),
/// which lives in 0x1390/0x1f40 instead of 0x138c.
pub fn su27_picture_skp() -> Vec<u8> {
    build(None, true, Mode::Picture)
}

/// The model's own entities hold nothing but a loose edge, so nothing in it
/// draws; the definitions are all there is to show.
pub fn only_definitions_skp() -> Vec<u8> {
    build(None, true, Mode::OnlyDefinitions)
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Plain,
    Picture,
    OnlyDefinitions,
    WireProfile,
    Views,
    InheritedFinishes,
    PositionedTexture,
}

fn build(texture: Option<&[u8]>, su27: bool, mode: Mode) -> Vec<u8> {
    let place = |guid: &[u8], def_id: u8, xf: &[u8], material: u8| {
        let paint = if material == 0 { Vec::new() } else { recs(0x7d0, &[rec(0x7d1, &[material])]) };
        if su27 {
            recs(0x138c, &[recs(0x1964, &[paint, rec(0x1965, &[]), rec(0x1966, xf), rec(0x1967, &[def_id]), rec(0x1968, &[0xab; 16])])])
        } else {
            recs(0x138d, &[recs(0x1d4c, &[recs(0x1964, &[paint, rec(0x1965, guid), rec(0x1966, xf)])])])
        }
    };
    let header = |id: u8| if su27 { vec![recs(0x7d0, &[with_id(id)])] } else { vec![] };
    let f = |v: [f64; 3]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    let vert = |id: u8, p: [f64; 3]| recs(0x9c4, &[with_id(id), rec(0x9c5, &f(p))]);
    let edge = |id: u8, a: u8, b: u8| recs(0xbb8, &[recs(0x7d0, &[with_id(id), rec(0x7d3, &[6])]), rec(0xbb9, &[a]), rec(0xbba, &[b])]);
    let using = |e: u8| recs(0xfa0, &[rec(0xfa1, &[e]), rec(0xfa2, &[0])]);
    let face_id = if mode == Mode::PositionedTexture {
        // A 90-degree rotation and a 4- by 8-inch offset in the face plane.
        let matrix = [0.0_f64, 1.0, 0.0, -1.0, 0.0, 0.0, 4.0, 8.0, 1.0];
        let values: Vec<u8> = matrix.iter().flat_map(|v| v.to_le_bytes()).collect();
        let mut map = recs(0x2713, &[rec(0x2714, &[1]), rec(0x2715, &values), rec(0x2716, &[0; 24])]);
        for tag in [0x2711, 0x2710, 0x36b2, 0x36b1, 0x5dd] {
            map = rec(tag, &map);
        }
        recs(0x5dc, &[rec(0x5de, &[20]), map])
    } else {
        with_id(20)
    };
    let face = recs(
        0xdac,
        &[
            recs(0x7d0, &[face_id, rec(0x7d1, &[if mode == Mode::InheritedFinishes { 0 } else { 9 }])]),
            rec(0xdad, &[0u8; 32]),
            recs(0xdae, &[recs(0x1194, &[recs(0x1195, &[using(10), using(11), using(12)])])]),
        ],
    );
    let ents = recs(
        0x1388,
        &[
            header(40).concat(),
            recs(0x1389, &[vert(1, [0.0, 0.0, 0.0]), vert(2, [100.0, 0.0, 0.0]), vert(3, [0.0, 100.0, 0.0])]),
            recs(0x138a, &[edge(10, 1, 2), edge(11, 2, 3), edge(12, 3, 1)]),
            if mode == Mode::WireProfile { vec![] } else { recs(0x138b, &[face]) },
        ],
    );
    let guid = b"0000-def";
    let def = recs(0x157c, &[if su27 { rec(0x157d, &[0xab; 16]) } else { vec![] }, rec(0x157e, if mode == Mode::WireProfile { b"Door profile" } else if su27 { b"Triangle" } else { guid }), ents]);
    let mut m = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0f64];
    m[9] = 10.0; // placed 10 inches along x
    let xf: Vec<u8> = m.iter().flat_map(|x| x.to_le_bytes()).collect();
    // the triangle's component sits inside an outer component (like the
    // parts of a cabinet), which the model places 10 inches along x
    let ident: Vec<u8> = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0f64].iter().flat_map(|x| x.to_le_bytes()).collect();
    let outer_guid = b"0000-out";
    let outer = recs(
        0x157c,
        &[if su27 { rec(0x157d, &[0xcd; 16]) } else { vec![] }, rec(0x157e, if su27 { b"Outer..." } else { outer_guid }), recs(0x1388, &[header(41).concat(), place(guid, 40, &ident, if mode == Mode::InheritedFinishes { 9 } else { 0 })])],
    );
    let inst = match mode {
        Mode::Plain | Mode::WireProfile | Mode::Views | Mode::PositionedTexture => place(outer_guid, 41, &xf, 0),
        Mode::InheritedFinishes => place(outer_guid, 41, &xf, 8),
        Mode::Picture => recs(0x1390, &[recs(0x1f40, &[recs(0x1964, &[rec(0x1965, &[]), rec(0x1966, &xf), rec(0x1967, &[41]), rec(0x1968, &[0xab; 16])])])]),
        Mode::OnlyDefinitions => {
            let a = vert(1, [0.0, 0.0, 0.0]);
            let b = vert(2, [100.0, 0.0, 0.0]);
            [recs(0x1389, &[a, b]), recs(0x138a, &[edge(10, 1, 2)])].concat()
        }
    };
    let materials = recs(0x30d4, &[
        recs(0x32c8, &[with_id(9), rec(0x32cc, b"Red")]),
        if mode == Mode::InheritedFinishes { recs(0x32c8, &[with_id(8), rec(0x32cc, b"Copper")]) } else { Vec::new() },
    ]);
    let mut model_parts = vec![
        recs(0x1f7, &[materials]),
        recs(0x1f6, &[recs(0x1388, &[inst])]),
        recs(0x1f9, &[recs(0x1770, &[recs(0x1771, &[def, outer])])]),
    ];
    if mode == Mode::Views {
        let page = |name: &str, flags: u32, eye: [f64; 3], perspective: bool, fov_is_height: bool| {
            let camera = recs(0x34bc, &[
                rec(0x34bd, &f(eye)),
                rec(0x34be, &f([20.0, 10.0, 3.0])),
                rec(0x34bf, &f([0.3, 0.4, 0.8])),
                rec(0x34c2, &[u8::from(perspective)]),
                rec(0x34c3, &72.0_f64.to_le_bytes()),
                rec(0x34c4, &42.0_f64.to_le_bytes()),
                rec(0x34c5, &1.6_f64.to_le_bytes()),
                rec(0x34c6, &[u8::from(fov_is_height)]),
            ]);
            recs(0x7148, &[
                recs(0x6f54, &[rec(0x6f55, name.as_bytes())]),
                rec(0x7149, &flags.to_le_bytes()),
                recs(0x714a, &[camera]),
            ])
        };
        let pages = recs(0x6d61, &[
            page("Prospettiva", 1, [50.0, -70.0, 58.0], true, true),
            page("Sezione B–B", 3, [-75.0, -30.0, 64.0], false, false),
            page("Campo orizzontale", 1, [50.0, -70.0, 58.0], true, false),
            page("Style only", 2, [-75.0, -30.0, 64.0], false, true),
            page("Degenerate", 1, [20.0, 10.0, 3.0], true, true),
        ]);
        model_parts.push(recs(0x207, &[recs(0x6d60, &[pages])]));
    }
    let model = recs(0x1f4, &model_parts);

    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    use std::io::Write;
    zip.start_file("model.dat", opts).unwrap();
    zip.write_all(&model).unwrap();
    zip.start_file("materials/Red/material.xml", opts).unwrap();
    match texture {
        None => zip.write_all(br#"<mat:material name="Red" colorRed="255" colorGreen="0" colorBlue="0" hasTexture="0">"#).unwrap(),
        Some(png) => {
            let xml = if matches!(mode, Mode::InheritedFinishes | Mode::PositionedTexture) {
                r#"<mat:material name="Red" colorRed="255" colorGreen="0" colorBlue="0" hasTexture="1"><mat:texture xScale="31.496062992125985" yScale="82.677165354330711"><mat:images><mat:image path="./tex.png"/></mat:images></mat:texture></mat:material>"#
            } else {
                r#"<mat:material name="Red" colorRed="255" colorGreen="0" colorBlue="0" hasTexture="1"><mat:image path="./tex.png" width="10"/>"#
            };
            zip.write_all(xml.as_bytes()).unwrap();
            zip.start_file("materials/Red/tex.png", opts).unwrap();
            zip.write_all(png).unwrap();
        }
    }
    if mode == Mode::InheritedFinishes {
        zip.start_file("materials/Copper/material.xml", opts).unwrap();
        zip.write_all(br#"<mat:material name="Copper" colorRed="166" colorGreen="94" colorBlue="55" hasTexture="0"/>"#).unwrap();
    }
    let archive = zip.finish().unwrap().into_inner();

    let mut w = W(Vec::new());
    w.0.extend([0xFF, 0xFE, 0xFF, 0x0E]);
    for u in "SketchUp Model".encode_utf16() {
        w.u16(u);
    }
    w.ustr(if su27 { "{27.0.0}" } else { "{26.2.0}" });
    w.0.extend(b"VFF\x08\x00\x01\x00");
    w.0.extend(archive);
    w.0
}


/// A modern file made of definitions that place each other: `defs[i]` lists (definition index, copies) pairs
/// that definition i places; every definition also holds one triangle. The model's own list places `roots`.
/// Definitions are named by GUID "d<i>". Round 8 (cycles, depth, expansion).
pub fn graph_skp(defs: &[Vec<(usize, usize)>], roots: &[usize]) -> Vec<u8> {
    graph_skp_with(defs, roots, &[])
}

/// `graph_skp` plus extra archive entries (name, bytes), for the zip-limit tests.
pub fn graph_skp_with(defs: &[Vec<(usize, usize)>], roots: &[usize], extra: &[(&str, Vec<u8>)]) -> Vec<u8> {
    graph_full(defs, roots, &[], extra)
}

/// A file with one triangle definition and materials (id, name) that the archive's own entries describe.
pub fn materials_skp(mats: &[(u32, String)], extra: &[(&str, Vec<u8>)]) -> Vec<u8> {
    graph_full(&[vec![]], &[0], mats, extra)
}

/// One material "name" with the given material.xml and picture (`tex.png`).
pub fn with_material(name: &str, xml: Vec<u8>, picture: Vec<u8>) -> Vec<u8> {
    let (a, b) = (format!("materials/{name}/material.xml"), format!("materials/{name}/tex.png"));
    graph_full(&[vec![]], &[0], &[(1, name.to_string())], &[(a.as_str(), xml), (b.as_str(), picture)])
}

pub fn tiny_png() -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::RgbImage::new(2, 2).write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn graph_full(defs: &[Vec<(usize, usize)>], roots: &[usize], mats: &[(u32, String)], extra: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let f = |v: [f64; 3]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    let vert = |id: u8, p: [f64; 3]| recs(0x9c4, &[with_id(id), rec(0x9c5, &f(p))]);
    let edge = |id: u8, a: u8, b: u8| recs(0xbb8, &[recs(0x7d0, &[with_id(id), rec(0x7d3, &[6])]), rec(0xbb9, &[a]), rec(0xbba, &[b])]);
    let using = |e: u8| recs(0xfa0, &[rec(0xfa1, &[e]), rec(0xfa2, &[0])]);
    let ident: Vec<u8> = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0f64].iter().flat_map(|x| x.to_le_bytes()).collect();
    let place = |j: usize| recs(0x1964, &[rec(0x1965, format!("d{j}").as_bytes()), rec(0x1966, &ident)]);
    let places = |js: Vec<usize>| recs(0x138d, &[recs(0x1d4c, &js.into_iter().map(place).collect::<Vec<_>>())]);
    let tri = |extra: Vec<u8>| {
        let face = recs(0xdac, &[recs(0x7d0, &[with_id(20)]), rec(0xdad, &[0u8; 32]), recs(0xdae, &[recs(0x1194, &[recs(0x1195, &[using(10), using(11), using(12)])])])]);
        recs(
            0x1388,
            &[
                recs(0x1389, &[vert(1, [0.0, 0.0, 0.0]), vert(2, [100.0, 0.0, 0.0]), vert(3, [0.0, 100.0, 0.0])]),
                recs(0x138a, &[edge(10, 1, 2), edge(11, 2, 3), edge(12, 3, 1)]),
                recs(0x138b, &[face]),
                extra,
            ],
        )
    };
    let def_records: Vec<Vec<u8>> = defs
        .iter()
        .enumerate()
        .map(|(i, kids)| {
            let js: Vec<usize> = kids.iter().flat_map(|(j, n)| std::iter::repeat(*j).take(*n)).collect();
            recs(0x157c, &[rec(0x157e, format!("d{i}").as_bytes()), tri(if js.is_empty() { vec![] } else { places(js) })])
        })
        .collect();
    let root = recs(0x1388, &[if roots.is_empty() { vec![] } else { places(roots.to_vec()) }, recs(0x1389, &[vert(1, [0.0, 0.0, 0.0]), vert(2, [1.0, 0.0, 0.0])]), recs(0x138a, &[edge(10, 1, 2)])]);
    let material_list: Vec<Vec<u8>> = mats.iter().map(|(id, name)| recs(0x32c8, &[recs(0x5dc, &[rec(0x5de, &id.to_le_bytes())]), rec(0x32cc, name.as_bytes())])).collect();
    let model = recs(0x1f4, &[recs(0x1f7, &[recs(0x30d4, &material_list)]), recs(0x1f6, &[root]), recs(0x1f9, &[recs(0x1770, &[recs(0x1771, &def_records)])])]);
    vff(&model, extra)
}

/// A modern file around a given `model.dat` body, plus extra archive entries.
pub fn vff(model: &[u8], extra: &[(&str, Vec<u8>)]) -> Vec<u8> {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("model.dat", opts).unwrap();
    zip.write_all(model).unwrap();
    for (n, b) in extra {
        zip.start_file(*n, opts).unwrap();
        zip.write_all(b).unwrap();
    }
    let archive = zip.finish().unwrap().into_inner();
    let mut w = W(Vec::new());
    w.0.extend([0xFF, 0xFE, 0xFF, 0x0E]);
    for u in "SketchUp Model".encode_utf16() {
        w.u16(u);
    }
    w.ustr("{26.2.0}");
    w.0.extend(b"VFF\x08\x00\x01\x00");
    w.0.extend(archive);
    w.0
}

/// A record nested `depth` deep (6-byte records, each holding the next).
pub fn nested_records(depth: usize) -> Vec<u8> {
    let mut body: Vec<u8> = vec![0u8; 0];
    for _ in 0..depth {
        body = rec(0x32c9, &body);
        if body.len() < 6 {
            body.resize(6, 0);
        }
    }
    body
}
