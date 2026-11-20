#!/usr/bin/env python3
"""Generate the flat scale figures in core/assets (people, cyclists, cars).

Each figure is original art, drawn here from simple shapes: limbs are tapered
capsules between joints, heads are circles, cars are outlines. Every colour
is merged into one clean outline (holes included) by sampling a signed
distance field and tracing it with marching squares, then simplified.

Figures are drawn flat in the x-z plane facing -y (the side the camera sees),
origin on the ground at the middle of the footprint. Parts in front sit a
fraction of a millimetre nearer -y so they never fight for the same plane.

    python3 tools/scale_figures/scale_figures.py          # writes core/assets/*.jsonl
    python3 tools/scale_figures/scale_figures.py --check  # fails if a file is stale

Pure Python, no packages needed.
"""

import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ASSETS = ROOT / "core" / "assets"
CELL = 0.004  # sampling grid, meters
SIMPLIFY = 0.0025  # outline tolerance, meters
LAYER = 0.002  # gap between parts, meters

# ---------------------------------------------------------------- shapes (SDF)


def circle(c, r):
    return lambda x, z: math.hypot(x - c[0], z - c[1]) - r


def ring(c, r, t):
    """A ring (a tyre): radius r to the middle of the band, band width 2t."""
    return lambda x, z: abs(math.hypot(x - c[0], z - c[1]) - r) - t


def ellipse(c, rx, rz):
    m = min(rx, rz)
    return lambda x, z: (math.hypot((x - c[0]) / rx, (z - c[1]) / rz) - 1.0) * m


def capsule(a, b, ra, rb=None):
    """A limb: from a (radius ra) to b (radius rb)."""
    rb = ra if rb is None else rb
    dx, dz = b[0] - a[0], b[1] - a[1]
    ll = dx * dx + dz * dz or 1e-12

    def f(x, z):
        t = max(0.0, min(1.0, ((x - a[0]) * dx + (z - a[1]) * dz) / ll))
        return math.hypot(x - a[0] - dx * t, z - a[1] - dz * t) - (ra + (rb - ra) * t)

    return f


def chain(points, ra, rb=None):
    """Capsules along a polyline, tapering from ra to rb."""
    rb = ra if rb is None else rb
    n = len(points) - 1
    return [capsule(points[i], points[i + 1], ra + (rb - ra) * i / n, ra + (rb - ra) * (i + 1) / n) for i in range(n)]


def poly(pts, round_=0.0):
    """A filled polygon (any winding), optionally with rounded corners."""
    pts = list(pts)

    def f(x, z):
        d = float("inf")
        inside = False
        j = len(pts) - 1
        for i in range(len(pts)):
            (ax, az), (bx, bz) = pts[j], pts[i]
            ex, ez = bx - ax, bz - az
            ll = ex * ex + ez * ez or 1e-12
            t = max(0.0, min(1.0, ((x - ax) * ex + (z - az) * ez) / ll))
            d = min(d, math.hypot(x - ax - ex * t, z - az - ez * t))
            if (az > z) != (bz > z) and x < ax + (z - az) * ex / (bz - az):
                inside = not inside
            j = i
        return (-d if inside else d) - round_

    return f


def leg(hip, ankle, length, bend=1):
    """The knee between hip and ankle for a leg of total `length` (two equal
    bones); bend=+1 puts the knee forward (+x), -1 back."""
    (hx, hz), (ax, az) = hip, ankle
    d = math.hypot(ax - hx, az - hz)
    half = length / 2
    d = min(d, length * 0.999)
    h = math.sqrt(max(half * half - (d / 2) ** 2, 0))
    mx, mz = (hx + ax) / 2, (hz + az) / 2
    ux, uz = (ax - hx) / d, (az - hz) / d
    # perpendicular, pick the side by `bend`
    px, pz = -uz, ux
    if px * bend < 0:
        px, pz = -px, -pz
    return (mx + px * h, mz + pz * h)


# ------------------------------------------------------------ contour tracing


def trace(shapes, pad=0.02):
    """Outline loops (outer CCW, holes CW in x-z) of the union of shapes."""
    f = lambda x, z: min(s(x, z) for s in shapes)
    # bounds: sample coarsely to find where the shape is
    xs0, xs1, zs0, zs1 = -6.0, 6.0, -0.5, 4.0
    lo_x = lo_z = float("inf")
    hi_x = hi_z = float("-inf")
    step = 0.02
    z = zs0
    while z <= zs1:
        x = xs0
        while x <= xs1:
            if f(x, z) < step:
                lo_x, hi_x, lo_z, hi_z = min(lo_x, x), max(hi_x, x), min(lo_z, z), max(hi_z, z)
            x += step
        z += step
    x0, z0 = lo_x - pad, lo_z - pad
    nx = int((hi_x + pad - x0) / CELL) + 2
    nz = int((hi_z + pad - z0) / CELL) + 2
    g = [[f(x0 + i * CELL, z0 + k * CELL) for i in range(nx)] for k in range(nz)]

    def P(i, k):
        return (x0 + i * CELL, z0 + k * CELL)

    def cross(a, b, va, vb):
        t = va / (va - vb)
        return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t)

    # each crossing (a grid edge) joins exactly two cell segments: walk them
    adj = {}
    pos = {}
    for k in range(nz - 1):
        for i in range(nx - 1):
            # corners 0 (i,k) 1 (i+1,k) 2 (i+1,k+1) 3 (i,k+1); edge e joins corner e and e+1
            c = [(i, k), (i + 1, k), (i + 1, k + 1), (i, k + 1)]
            v = [g[k][i], g[k][i + 1], g[k + 1][i + 1], g[k + 1][i]]
            ins = [vv < 0 for vv in v]
            if all(ins) or not any(ins):
                continue

            def key(e):
                a, b = c[e], c[(e + 1) % 4]
                kk = (min(a, b), max(a, b))
                if kk not in pos:
                    pos[kk] = cross(P(*a), P(*b), v[e], v[(e + 1) % 4])
                return kk

            crossing = [e for e in range(4) if ins[e] != ins[(e + 1) % 4]]
            if len(crossing) == 2:
                pairs = [tuple(crossing)]
            else:  # saddle: cut off the corners on the side the centre isn't
                centre_in = sum(v) / 4 < 0
                cut = [q for q in range(4) if ins[q] != centre_in]
                pairs = [((q - 1) % 4, q) for q in cut]
            for e0, e1 in pairs:
                a, b = key(e0), key(e1)
                adj.setdefault(a, []).append(b)
                adj.setdefault(b, []).append(a)
    loops = []
    seen = set()
    for start in adj:
        if start in seen:
            continue
        loop, prev, cur = [], None, start
        while cur not in seen:
            seen.add(cur)
            loop.append(pos[cur])
            nb = adj[cur]
            nxt = nb[0] if nb[0] != prev or len(nb) == 1 else nb[1]
            prev, cur = cur, nxt
        if len(loop) >= 3:
            loops.append(loop)
    return loops


def area(loop):
    return sum(a[0] * b[1] - b[0] * a[1] for a, b in zip(loop, loop[1:] + loop[:1])) / 2


def rdp(pts, eps):
    if len(pts) < 3:
        return pts
    (ax, az), (bx, bz) = pts[0], pts[-1]
    dx, dz = bx - ax, bz - az
    ll = math.hypot(dx, dz) or 1e-12
    best, bi = -1.0, 0
    for i in range(1, len(pts) - 1):
        d = abs((pts[i][0] - ax) * dz - (pts[i][1] - az) * dx) / ll
        if d > best:
            best, bi = d, i
    if best <= eps:
        return [pts[0], pts[-1]]
    return rdp(pts[: bi + 1], eps)[:-1] + rdp(pts[bi:], eps)


def simplify(loop, eps):
    # split the closed loop at its two farthest-apart points
    i = 0
    j = max(range(len(loop)), key=lambda k: (loop[k][0] - loop[0][0]) ** 2 + (loop[k][1] - loop[0][1]) ** 2)
    a = rdp(loop[i : j + 1], eps)
    b = rdp(loop[j:] + [loop[0]], eps)
    return a[:-1] + b[:-1]


def inside(pt, loop):
    x, z = pt
    c = False
    for (ax, az), (bx, bz) in zip(loop, loop[1:] + loop[:1]):
        if (az > z) != (bz > z) and x < ax + (z - az) * (bx - ax) / (bz - az):
            c = not c
    return c


# ------------------------------------------------------------- figure → jsonl


def r4(v):
    v = round(v, 4)
    return 0.0 if v == 0 else v


class Figure:
    def __init__(self, key, header):
        self.key = key
        self.header = header
        self.parts = []  # (name, color, shapes), back to front

    def part(self, name, color, *shapes):
        flat = []
        for s in shapes:
            flat.extend(s if isinstance(s, list) else [s])
        self.parts.append((name, color, flat))

    def jsonl(self):
        out = [f"# {line}" if line else "#" for line in self.header]
        traced = []
        for name, color, shapes in self.parts:
            loops = [simplify(l, SIMPLIFY) for l in trace(shapes)]
            traced.append([l for l in loops if len(l) >= 3 and abs(area(l)) > 1e-5])
        # stand it on the ground: the lowest point at z = 0
        ground = min(z for loops in traced for l in loops for _, z in l)
        traced = [[[(x, z - ground) for x, z in l] for l in loops] for loops in traced]
        for layer, ((name, color, _), loops) in enumerate(zip(self.parts, traced)):
            y = r4(-LAYER * layer)
            # nesting depth decides outer (even) or hole (odd), whatever the winding
            depth = [sum(1 for o in loops if o is not l and abs(area(o)) > abs(area(l)) and inside(l[0], o)) for l in loops]
            outers = sorted([l for l, d in zip(loops, depth) if d % 2 == 0], key=lambda l: -abs(area(l)))
            holes = [l for l, d in zip(loops, depth) if d % 2 == 1]
            names = []
            for n, o in enumerate(outers):
                if area(o) < 0:
                    o = o[::-1]
                nm = name if len(outers) == 1 else f"{name}{n + 1}"
                names.append(nm)
                pts = [[r4(x), y, r4(z)] for x, z in o]
                out.append(json.dumps({"cmd": "polygon", "points": pts, "as": nm}))
                mine = [h for h in holes if inside(h[0], o) and not any(inside(h[0], p) for p in outers if p is not o and abs(area(p)) < abs(area(o)) and inside(p[0], o))]
                for m, h in enumerate(mine):
                    hn = f"{nm}_hole{m + 1}"
                    pts = [[r4(x), y, r4(z)] for x, z in h]
                    out.append(json.dumps({"cmd": "polygon", "points": pts, "as": hn}))
                    out.append(json.dumps({"cmd": "delete", "ids": [f"@{hn}"]}))
            out.append(json.dumps({"cmd": "paint", "ids": [f"@{n}" for n in names], "color": list(color)}))
        return "\n".join(out) + "\n"


def shade(c, k):
    return tuple(round(v * k, 3) for v in c)


# ----------------------------------------------------------------- the people

SKIN = (0.45, 0.4, 0.38)  # head and hands: one neutral tone, a silhouette


def person(fig, pose, top, bottom, h=1.75, hair=None, extra_front=()):
    """A person in profile facing +x. `pose` gives joints in a 1.75 m body;
    everything scales by h / 1.75."""
    s = h / 1.75
    P = {k: (v[0] * s, v[1] * s) for k, v in pose.items()}
    T = lambda r: r * s
    far, near = shade(bottom, 0.82), bottom
    # far limbs first (behind the body)
    fig.part("far_arm", shade(top, 0.82), chain([P["shoulder"], P["far_elbow"], P["far_wrist"]], T(0.045), T(0.033)), circle(P["far_wrist"], T(0.04)))
    fig.part("far_leg", far, chain([P["hip"], P["far_knee"], P["far_ankle"]], T(0.075), T(0.045)), capsule(P["far_heel"], P["far_toe"], T(0.04), T(0.03)))
    fig.part("near_leg", near, capsule(P["hip"], P["hip_back"], T(0.11), T(0.11)), chain([P["hip"], P["near_knee"], P["near_ankle"]], T(0.08), T(0.045)), capsule(P["near_heel"], P["near_toe"], T(0.04), T(0.03)))
    fig.part("body", top, capsule(P["hip"], P["chest"], T(0.115), T(0.13)), capsule(P["chest"], P["neck"], T(0.13), T(0.05)))
    fig.part("head", hair or SKIN, circle(P["head"], T(0.105)), capsule(P["neck"], P["head"], T(0.05), T(0.05)))
    fig.part("near_arm", top, chain([P["shoulder"], P["near_elbow"], P["near_wrist"]], T(0.05), T(0.035)))
    fig.part("hand", SKIN, circle(P["near_wrist"], T(0.042)))
    for e in extra_front:
        fig.part(*e)


WALK = {
    "head": (0.06, 1.64),
    "neck": (0.035, 1.5),
    "chest": (0.02, 1.33),
    "shoulder": (0.02, 1.42),
    "hip": (0.0, 0.94),
    "hip_back": (-0.01, 0.97),
    "near_knee": (0.13, 0.52),
    "near_ankle": (0.22, 0.09),
    "near_heel": (0.18, 0.04),
    "near_toe": (0.42, 0.05),
    "far_knee": (-0.07, 0.51),
    "far_ankle": (-0.29, 0.13),
    "far_heel": (-0.32, 0.12),
    "far_toe": (-0.12, 0.035),
    "near_elbow": (-0.09, 1.16),
    "near_wrist": (-0.13, 0.92),
    "far_elbow": (0.11, 1.17),
    "far_wrist": (0.25, 1.0),
}


def walker():
    f = Figure("walker", ["Scale figure: a walker, 1.78 m, mid-stride in profile. Flat, drawn facing -y;", "the component turns to face the camera. Origin: on the ground under the hips.", "Generated by tools/scale_figures/scale_figures.py (original art)."])
    person(f, WALK, top=(0.24, 0.42, 0.58), bottom=(0.22, 0.23, 0.27), h=1.78)
    return f


def shopper():
    pose = dict(WALK)
    pose.update({"near_knee": (0.1, 0.5), "near_ankle": (0.17, 0.08), "near_heel": (0.13, 0.04), "near_toe": (0.37, 0.045),
                 "far_knee": (-0.05, 0.5), "far_ankle": (-0.22, 0.12), "far_heel": (-0.25, 0.11), "far_toe": (-0.05, 0.035),
                 "near_elbow": (-0.02, 1.15), "near_wrist": (-0.0, 0.9), "far_elbow": (0.08, 1.16), "far_wrist": (0.16, 0.97)})
    f = Figure("shopper", ["Scale figure: a shopper with a tote bag, 1.66 m, walking in profile. Flat, drawn", "facing -y; the component turns to face the camera. Origin: on the ground under the hips.", "Generated by tools/scale_figures/scale_figures.py (original art)."])
    s = 1.66 / 1.75
    w = (pose["near_wrist"][0] * s, pose["near_wrist"][1] * s)
    bag = [("bag", (0.78, 0.6, 0.3), poly([(w[0] - 0.17, w[1] - 0.14), (w[0] + 0.17, w[1] - 0.14), (w[0] + 0.15, w[1] - 0.5), (w[0] - 0.15, w[1] - 0.5)], 0.012)),
           ("straps", (0.62, 0.46, 0.22), capsule((w[0] - 0.1, w[1] - 0.14), (w[0], w[1] + 0.01), 0.012), capsule((w[0] + 0.1, w[1] - 0.14), (w[0], w[1] + 0.01), 0.012))]
    # a skirt over the hips
    skirt = ("skirt", (0.46, 0.27, 0.36), poly([(-0.13 * s, 0.98 * s), (0.13 * s, 0.98 * s), (0.2 * s, 0.6 * s), (-0.17 * s, 0.6 * s)], 0.02))
    person(f, pose, top=(0.58, 0.36, 0.42), bottom=(0.28, 0.26, 0.28), h=1.66, hair=(0.24, 0.18, 0.15), extra_front=())
    # put the skirt over the near leg, under the body
    i = [p[0] for p in f.parts].index("body")
    f.parts.insert(i, (skirt[0], skirt[1], [skirt[2]]))
    for b in bag:
        f.part(b[0], b[1], *b[2:])
    # the hand holds the straps: draw it last
    hand = next(p for p in f.parts if p[0] == "hand")
    f.parts.remove(hand)
    f.parts.append(hand)
    return f


def jogger():
    pose = {
        "head": (0.13, 1.63), "neck": (0.1, 1.49), "chest": (0.07, 1.33), "shoulder": (0.07, 1.41),
        "hip": (0.0, 0.96), "hip_back": (-0.01, 0.99),
        "near_knee": (0.3, 0.68), "near_ankle": (0.22, 0.28), "near_heel": (0.18, 0.25), "near_toe": (0.42, 0.24),
        "far_knee": (-0.05, 0.53), "far_ankle": (-0.32, 0.22), "far_heel": (-0.36, 0.22), "far_toe": (-0.19, 0.06),
        "near_elbow": (-0.19, 1.2), "near_wrist": (-0.27, 1.03), "far_elbow": (0.21, 1.2), "far_wrist": (0.33, 1.36),
    }
    f = Figure("jogger", ["Scale figure: a jogger, 1.70 m mid-stride, running in profile. Flat, drawn facing -y;", "the component turns to face the camera. Origin: on the ground under the hips.", "Generated by tools/scale_figures/scale_figures.py (original art)."])
    person(f, pose, top=(0.85, 0.42, 0.25), bottom=(0.2, 0.22, 0.28))
    return f


def child():
    pose = dict(WALK)
    pose.update({"head": (0.06, 1.6), "neck": (0.035, 1.47),
                 "near_knee": (0.1, 0.52), "near_ankle": (0.17, 0.09), "near_heel": (0.13, 0.045), "near_toe": (0.36, 0.05),
                 "far_knee": (-0.05, 0.51), "far_ankle": (-0.22, 0.13), "far_heel": (-0.25, 0.12), "far_toe": (-0.06, 0.04),
                 "near_elbow": (-0.08, 1.15), "near_wrist": (-0.12, 0.94), "far_elbow": (0.13, 1.18), "far_wrist": (0.26, 1.1)})
    f = Figure("child", ["Scale figure: a child, 1.15 m, walking in profile. Flat, drawn facing -y;", "the component turns to face the camera. Origin: on the ground under the hips.", "Generated by tools/scale_figures/scale_figures.py (original art)."])
    # children have bigger heads for their height: draw at 1.15 m, then enlarge the head
    person(f, pose, top=(0.36, 0.62, 0.42), bottom=(0.25, 0.3, 0.45), h=1.15)
    s = 1.15 / 1.75
    for n, (name, col, shapes) in enumerate(f.parts):
        if name == "head":
            hx, hz = pose["head"][0] * s, pose["head"][1] * s - 0.02
            f.parts[n] = (name, (0.3, 0.22, 0.17), [circle((hx, hz), 0.1), capsule((pose["neck"][0] * s, pose["neck"][1] * s), (hx, hz), 0.035)])
    return f


def dog(fig, x, color=(0.55, 0.42, 0.3)):
    """A medium dog in profile facing +x, 0.55 m at the head; back legs at x."""
    far = shade(color, 0.8)
    fig.part("dog_far_legs", far, capsule((x + 0.05, 0.3), (x + 0.02, 0.02), 0.03, 0.022), capsule((x + 0.42, 0.3), (x + 0.45, 0.02), 0.03, 0.022))
    fig.part("dog", color,
             ellipse((x + 0.25, 0.36), 0.28, 0.1),
             capsule((x + 0.0, 0.32), (x + 0.07, 0.03), 0.04, 0.022), capsule((x + 0.06, 0.03), (x + 0.11, 0.025), 0.022),
             capsule((x + 0.47, 0.32), (x + 0.5, 0.03), 0.035, 0.022), capsule((x + 0.5, 0.03), (x + 0.55, 0.025), 0.022),
             capsule((x + 0.47, 0.4), (x + 0.56, 0.5), 0.05, 0.045),
             ellipse((x + 0.6, 0.53), 0.08, 0.06), capsule((x + 0.62, 0.52), (x + 0.71, 0.5), 0.035, 0.028),
             capsule((x + 0.56, 0.56), (x + 0.54, 0.62), 0.03, 0.015),
             capsule((x - 0.02, 0.4), (x - 0.16, 0.52), 0.02, 0.012))
    fig.part("dog_eye", (0.1, 0.09, 0.11), circle((x + 0.63, 0.55), 0.011))


def dog_walker():
    pose = dict(WALK)
    pose.update({"near_elbow": (0.07, 1.15), "near_wrist": (0.24, 1.0)})
    f = Figure("dog_walker", ["Scale figure: a person walking a dog, 1.75 m, in profile. Flat, drawn facing -y;", "the component turns to face the camera. Origin: on the ground under the person's hips.", "Generated by tools/scale_figures/scale_figures.py (original art)."])
    person(f, pose, top=(0.3, 0.45, 0.32), bottom=(0.3, 0.27, 0.24))
    dog(f, 0.55)
    f.part("lead", (0.75, 0.2, 0.18), capsule((0.25, 0.99), (0.7, 0.62), 0.008), capsule((0.7, 0.62), (1.13, 0.47), 0.008))
    hand = next(p for p in f.parts if p[0] == "hand")
    f.parts.remove(hand)
    f.parts.append(hand)
    return f


def stroller():
    pose = dict(WALK)
    pose.update({"chest": (0.04, 1.33), "shoulder": (0.05, 1.41), "neck": (0.06, 1.5), "head": (0.09, 1.64),
                 "near_elbow": (0.22, 1.16), "near_wrist": (0.4, 1.02), "far_elbow": (0.24, 1.18), "far_wrist": (0.41, 1.04),
                 "near_knee": (0.1, 0.51), "near_ankle": (0.16, 0.08), "near_heel": (0.12, 0.04), "near_toe": (0.36, 0.045),
                 "far_knee": (-0.06, 0.5), "far_ankle": (-0.24, 0.12), "far_heel": (-0.27, 0.11), "far_toe": (-0.07, 0.035)})
    f = Figure("stroller", ["Scale figure: a person pushing a stroller, 1.70 m, in profile. Flat, drawn facing -y;", "the component turns to face the camera. Origin: on the ground under the person's hips.", "Generated by tools/scale_figures/scale_figures.py (original art)."])
    s = 1.7 / 1.75
    person(f, pose, top=(0.46, 0.4, 0.6), bottom=(0.24, 0.24, 0.3), h=1.7)
    hx, hz = 0.41 * s, 1.02 * s
    frame = (0.2, 0.2, 0.23)
    f.parts.insert(0, ("stroller_frame", frame, [capsule((hx, hz), (0.68, 0.38), 0.014), capsule((0.68, 0.38), (0.55, 0.11), 0.012), capsule((0.68, 0.38), (0.98, 0.11), 0.012), capsule((hx - 0.03, hz), (hx + 0.02, hz + 0.01), 0.018)]))
    f.part("stroller", (0.32, 0.5, 0.55), poly([(0.55, 0.62), (1.02, 0.62), (0.98, 0.42), (0.6, 0.4)], 0.03), poly([(0.55, 0.62), (0.56, 0.86), (0.68, 0.84), (0.83, 0.62)], 0.03))
    f.part("stroller_wheels", frame, ring((0.55, 0.11), 0.09, 0.016), ring((0.98, 0.11), 0.09, 0.016), circle((0.55, 0.11), 0.018), circle((0.98, 0.11), 0.018))
    hand = next(p for p in f.parts if p[0] == "hand")
    f.parts.remove(hand)
    f.parts.append(hand)
    return f


def wheelchair():
    f = Figure("wheelchair", ["Scale figure: a wheelchair user, 1.30 m, in profile. Flat, drawn facing -y;", "the component turns to face the camera. Origin: on the ground under the big wheel.", "Generated by tools/scale_figures/scale_figures.py (original art)."])
    metal = (0.22, 0.23, 0.26)
    top, bottom = (0.6, 0.48, 0.3), (0.25, 0.28, 0.38)
    f.part("chair_far", shade(metal, 0.8), ring((0.0, 0.3), 0.28, 0.02), circle((0.0, 0.3), 0.03))
    f.part("chair", metal,
           capsule((-0.17, 0.5), (0.27, 0.5), 0.018), capsule((-0.17, 0.5), (-0.2, 0.98), 0.016), capsule((-0.2, 0.98), (-0.3, 0.98), 0.016),
           capsule((0.27, 0.5), (0.36, 0.1), 0.014), capsule((0.36, 0.1), (0.46, 0.1), 0.014),
           capsule((0.27, 0.5), (0.38, 0.16), 0.012), capsule((0.0, 0.3), (0.0, 0.5), 0.014), capsule((-0.17, 0.5), (0.38, 0.16), 0.01))
    f.part("thigh_far", shade(bottom, 0.82), capsule((-0.08, 0.6), (0.28, 0.6), 0.075, 0.065), capsule((0.28, 0.6), (0.36, 0.17), 0.055, 0.045), capsule((0.33, 0.13), (0.5, 0.13), 0.035, 0.03))
    f.part("legs", bottom, capsule((-0.08, 0.62), (0.32, 0.6), 0.08, 0.065), capsule((0.32, 0.6), (0.4, 0.18), 0.058, 0.045), capsule((0.37, 0.14), (0.55, 0.14), 0.038, 0.03))
    f.part("body", top, capsule((-0.08, 0.66), (-0.07, 1.0), 0.13, 0.13), capsule((-0.07, 1.0), (-0.04, 1.12), 0.13, 0.05))
    f.part("head", SKIN, circle((0.0, 1.2), 0.105), capsule((-0.04, 1.1), (0.0, 1.2), 0.05))
    f.part("wheel", metal, ring((0.0, 0.3), 0.3, 0.022), ring((0.0, 0.3), 0.255, 0.008), circle((0.0, 0.3), 0.035), circle((0.42, 0.06), 0.06))
    f.part("arm", top, chain([(-0.04, 1.02), (0.02, 0.78), (0.12, 0.56)], 0.05, 0.035))
    f.part("hand", SKIN, circle((0.12, 0.56), 0.042))
    return f


def cyclist():
    f = Figure("cyclist", ["Scale figure: a cyclist, 1.70 m tall and 1.75 m long, in profile. Flat, drawn", "facing -y; the component turns to face the camera. Origin: on the ground under the pedals.", "Generated by tools/scale_figures/scale_figures.py (original art)."])
    metal = (0.2, 0.21, 0.24)
    paint = (0.8, 0.25, 0.22)
    top, bottom = (0.3, 0.5, 0.62), (0.2, 0.21, 0.26)
    rear, front, bb = (-0.5, 0.34), (0.5, 0.34), (-0.04, 0.29)
    seat, head_t, head_b = (-0.2, 0.83), (0.36, 0.82), (0.4, 0.69)
    crank = 0.17
    a = math.radians(-35)
    near_pedal = (bb[0] + crank * math.cos(a), bb[1] + crank * math.sin(a))
    far_pedal = (bb[0] - crank * math.cos(a), bb[1] - crank * math.sin(a))
    hip = (-0.23, 1.0)
    leg_len = 0.9
    near_ankle = (near_pedal[0] - 0.02, near_pedal[1] + 0.06)
    far_ankle = (far_pedal[0] - 0.02, far_pedal[1] + 0.06)
    f.part("far_leg", shade(bottom, 0.82), chain([hip, leg(hip, far_ankle, leg_len), far_ankle], 0.075, 0.045), capsule((far_ankle[0] - 0.06, far_pedal[1] + 0.02), (far_ankle[0] + 0.15, far_pedal[1] + 0.02), 0.035, 0.028))
    f.part("far_arm", shade(top, 0.82), chain([(0.08, 1.38), (0.25, 1.2), (0.42, 1.02)], 0.045, 0.033))
    f.part("wheels", metal, ring(rear, 0.32, 0.022), ring(front, 0.32, 0.022), circle(rear, 0.03), circle(front, 0.03))
    f.part("spokes", shade(metal, 1.6),
           *[capsule(c, (c[0] + 0.3 * math.cos(t), c[1] + 0.3 * math.sin(t)), 0.0035) for c in (rear, front) for t in [k * math.pi / 6 for k in range(12)]])
    f.part("frame", paint,
           capsule(bb, seat, 0.02), capsule(seat, head_t, 0.018), capsule(bb, head_b, 0.022), capsule(head_t, head_b, 0.024),
           capsule(bb, rear, 0.014), capsule(seat, rear, 0.012), capsule(head_b, front, 0.015))
    f.part("parts", metal,
           capsule(seat, (-0.22, 0.92), 0.012), capsule((-0.31, 0.94), (-0.13, 0.95), 0.022, 0.015),
           capsule(head_t, (0.35, 0.95), 0.014), capsule((0.35, 0.95), (0.45, 0.97), 0.016), capsule((0.45, 0.97), (0.48, 0.9), 0.014),
           circle(bb, 0.06), capsule(far_pedal, near_pedal, 0.014), capsule((near_pedal[0] - 0.04, near_pedal[1]), (near_pedal[0] + 0.04, near_pedal[1]), 0.012))
    knee = leg(hip, near_ankle, leg_len)
    f.part("near_leg", bottom, capsule(hip, (-0.27, 1.02), 0.1), chain([hip, knee, near_ankle], 0.08, 0.045), capsule((near_ankle[0] - 0.06, near_pedal[1] + 0.02), (near_ankle[0] + 0.15, near_pedal[1] + 0.02), 0.04, 0.03))
    f.part("body", top, capsule(hip, (0.0, 1.32), 0.115, 0.13), capsule((0.0, 1.32), (0.12, 1.42), 0.13, 0.06))
    f.part("head", SKIN, circle((0.2, 1.54), 0.1), capsule((0.12, 1.42), (0.2, 1.54), 0.05))
    f.part("helmet", (0.92, 0.78, 0.2), poly([(0.08, 1.57), (0.13, 1.66), (0.23, 1.68), (0.31, 1.6), (0.3, 1.56), (0.1, 1.55)], 0.015))
    f.part("near_arm", top, chain([(0.06, 1.38), (0.24, 1.2), (0.41, 1.0)], 0.05, 0.035))
    f.part("hand", SKIN, circle((0.42, 0.99), 0.04))
    return f


# --------------------------------------------------------------------- cars

def car_profile(key, header, body, windows, wheel_x, wheel_r, color):
    f = Figure(key, header)
    dark, glass, metal = (0.12, 0.12, 0.14), (0.62, 0.72, 0.8), (0.72, 0.73, 0.76)
    f.part("body", color, poly(body, 0.03))
    f.part("windows", glass, *[poly(w, 0.015) for w in windows])
    f.part("trim", shade(color, 0.7),
           *[capsule((w[0][0], w[0][1] - 0.04), (w[0][0], 0.42), 0.006) for w in windows[1:]])
    f.part("tyres", dark, *[circle((x, wheel_r), wheel_r) for x in wheel_x])
    f.part("rims", metal, *[ring((x, wheel_r), wheel_r * 0.5, wheel_r * 0.1) for x in wheel_x], *[circle((x, wheel_r), wheel_r * 0.14) for x in wheel_x])
    f.part("lights", (0.98, 0.9, 0.6), poly([(body[0][0] - 0.02, 0.72), (body[0][0] - 0.16, 0.74), (body[0][0] - 0.16, 0.67), (body[0][0] - 0.03, 0.64)], 0.01))
    return f


def car_flat():
    # a hatchback, 4.3 m, facing +x; body outline from the front bumper round
    body = [(2.13, 0.3), (2.15, 0.62), (2.05, 0.8), (1.0, 0.92), (0.45, 1.42), (-1.25, 1.45), (-1.95, 1.05), (-2.15, 0.85), (-2.15, 0.3)]
    windows = [[(0.92, 0.94), (0.43, 1.36), (-0.25, 1.38), (-0.25, 0.96)], [(-0.33, 0.96), (-0.33, 1.38), (-1.2, 1.39), (-1.75, 1.0)]]
    f = car_profile("car_flat", ["Scale figure: a flat car, a 4.3 m hatchback in profile, for elevations and", "sections. Drawn facing -y; it stays put (rotate it to line up with the road).", "Origin: on the ground at the middle. Generated by tools/scale_figures/scale_figures.py."],
                    body, windows, (1.35, -1.35), 0.32, (0.32, 0.45, 0.62))
    return f


def suv_flat():
    body = [(2.35, 0.36), (2.38, 0.75), (2.25, 0.95), (1.25, 1.07), (0.85, 1.68), (-1.95, 1.72), (-2.3, 1.45), (-2.35, 0.36)]
    windows = [[(1.17, 1.09), (0.82, 1.62), (0.25, 1.64), (0.25, 1.1)], [(0.17, 1.1), (0.17, 1.64), (-0.95, 1.65), (-0.95, 1.1)], [(-1.03, 1.1), (-1.03, 1.65), (-1.88, 1.66), (-2.12, 1.4), (-2.12, 1.1)]]
    return car_profile("suv_flat", ["Scale figure: a flat SUV, 4.7 m, in profile, for elevations and sections.", "Drawn facing -y; it stays put (rotate it to line up with the road).", "Origin: on the ground at the middle. Generated by tools/scale_figures/scale_figures.py."],
                       body, windows, (1.5, -1.45), 0.36, (0.68, 0.7, 0.72))


FIGURES = [walker, shopper, jogger, child, dog_walker, stroller, wheelchair, cyclist, car_flat, suv_flat]


def main():
    check = "--check" in sys.argv
    only = [a for a in sys.argv[1:] if not a.startswith("--")]
    stale = []
    for make in FIGURES:
        if only and make.__name__ not in only:
            continue
        fig = make()
        text = fig.jsonl()
        path = ASSETS / f"{fig.key}.jsonl"
        if check:
            if not path.exists() or path.read_text() != text:
                stale.append(path.name)
        else:
            path.write_text(text)
            print(f"{path.relative_to(ROOT)}: {text.count(chr(10))} lines")
    if stale:
        sys.exit(f"stale: {', '.join(stale)} (run tools/scale_figures/scale_figures.py)")


if __name__ == "__main__":
    main()
