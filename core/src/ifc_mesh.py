"""IFC to triangle meshes, for SketchCraft's IFC import (run by the app with
a Python that has IfcOpenShell). Usage: python -I ifc_mesh.py in.ifc out.json

Writes {"storeys": [...], "elements": [...]}: each element its IFC type, name,
GlobalId, storey, and triangles in meters, world coordinates, Z up, with one
color per triangle. Openings and spaces are left out (the openings are
already cut from their walls).
"""

import json
import multiprocessing
import sys

import ifcopenshell
import ifcopenshell.geom

try:
    import ifcopenshell.util.element as uel
except Exception:  # older builds
    uel = None

SKIP = {"IfcOpeningElement", "IfcSpace", "IfcVirtualElement", "IfcAnnotation", "IfcGrid"}


def color_of(mat):
    """(r, g, b, alpha) of an IfcOpenShell style, defensively."""
    try:
        d = mat.diffuse
        rgb = [d.r(), d.g(), d.b()] if callable(getattr(d, "r", None)) else list(d)[:3]
    except Exception:
        rgb = [0.8, 0.8, 0.8]
    try:
        t = float(mat.transparency)
        alpha = 1.0 - t if t == t else 1.0
    except Exception:
        alpha = 1.0
    return [float(x) for x in rgb] + [alpha]


def storey_of(el):
    if uel is None:
        return None
    seen = 0
    while el is not None and seen < 8:
        c = uel.get_container(el)
        if c is not None:
            return c.id() if c.is_a("IfcBuildingStorey") else None
        el = uel.get_aggregate(el)
        seen += 1
    return None


def main(src, out):
    f = ifcopenshell.open(src)
    s = ifcopenshell.geom.settings()
    for key in ("use-world-coords", "USE_WORLD_COORDS"):
        try:
            s.set(key, True)
            break
        except Exception:
            try:
                s.set(getattr(s, key), True)
                break
            except Exception:
                pass
    storeys = [{"id": st.id(), "name": st.Name or f"Level {i}", "elevation": float(st.Elevation or 0.0)} for i, st in enumerate(f.by_type("IfcBuildingStorey"))]
    elements = []
    it = ifcopenshell.geom.iterator(s, f, max(1, multiprocessing.cpu_count() - 1))
    if it.initialize():
        while True:
            shape = it.get()
            el = f.by_id(shape.id)
            kind = el.is_a()
            if kind not in SKIP:
                g = shape.geometry
                mats = [color_of(m) for m in getattr(g, "materials", [])]
                ids = list(getattr(g, "material_ids", []))
                faces = list(g.faces)
                elements.append({
                    "guid": getattr(el, "GlobalId", "") or "",
                    "type": kind,
                    "name": getattr(el, "Name", None) or "",
                    "storey": storey_of(el),
                    "verts": [round(v, 6) for v in g.verts],
                    "faces": faces,
                    "mats": mats,
                    "mat": ids if len(ids) * 3 == len(faces) else [],
                })
            if not it.next():
                break
    with open(out, "w") as fh:
        json.dump({"schema": f.schema, "storeys": storeys, "elements": elements}, fh)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
