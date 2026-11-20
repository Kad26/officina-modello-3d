# SketchUp Ruby scripts in SketchCraft

SketchCraft runs SketchUp Ruby scripts (`.rb`) and extensions (`.rbz`) that
build geometry: generators for stairs, framing, shelves, fences, houses and the
like. It isn't all of SketchUp's Ruby API: anything that isn't here stops with a
clear message ("UI::HtmlDialog isn't supported in SketchCraft yet") rather than
doing something wrong.

## Installing

**File › Plugins ▸ Manage plugins… › Install from file…** and pick the `.rbz`
or `.rb`. Its menu items (`UI.menu("Plugins").add_item …`, `UI::Command`s) show
up in **File › Plugins** and in Shift+S.

## How it works

1. The script is compiled to JavaScript with [Opal](https://opalrb.com) (MIT,
   vendored in `app/public/ruby`), in a sandboxed worker. The result is kept,
   so after the first time a script loads in milliseconds.
2. It runs, in another sandboxed worker (no network, no files), against the API
   in [`app/src/ruby/sketchup.rb`](../../app/src/ruby/sketchup.rb). That API
   keeps a copy of what the script builds, so `face.normal`, `edge.length` and
   the like answer straight away, and records each change as a SketchCraft
   command.
3. SketchCraft replays the commands. Each is an ordinary undo step, and if the
   script fails, nothing has changed.

Lengths are inches inside the script, as in SketchUp (`10.m`, `2.feet`,
`300.mm` convert). A face drawn flat on the ground faces down, as in SketchUp,
so the usual `face.pushpull(-height)` pulls up.

`UI.inputbox` shows a SketchCraft form. Because SketchUp scripts expect the
answer straight away, the script runs once to find the question, then again
with your answers (nothing is applied until the run that finishes).

## What's there

| | |
| --- | --- |
| Units | `Numeric#inch`, `feet`, `m`, `cm`, `mm`, `yard`, `km`, `degrees`, `radians`, `to_m`…; `String#to_l` |
| Geometry | `Geom::Point3d`, `Geom::Vector3d` (dot, cross, normalize, angle…), `Geom::Transformation` (translation, rotation, scaling, combined, inverse), `Geom::BoundingBox`, arrays as points, `X_AXIS`, `Y_AXIS`, `Z_AXIS`, `ORIGIN` |
| Model | `Sketchup.active_model`: `entities`, `active_entities`, `selection` (what you selected), `materials`, `definitions`, `layers`/`tags`, `start_operation`/`commit_operation` |
| Entities | `add_face` (points or edges), `add_line`, `add_edges`, `add_curve`, `add_circle`, `add_ngon`, `add_arc`, `add_group`, `add_instance`, `add_cpoint`, `add_cline`, `add_text`, `add_dimension_linear`, `erase_entities`, `clear!`, `each` |
| Faces and edges | `pushpull`, `reverse!`, `normal`, `area`, `vertices`, `edges`, `plane`, `material=`, `erase!`; edge `length`, `start`, `end`, `soft=`/`smooth=` |
| Groups and components | `entities`, `name=`, `transform!`/`move!`, `material=`, `layer=`, `hidden=`, `explode`, `to_component`, `locked=`; `definitions.add` with `add_instance` copies |
| Materials | `materials.add`, `color=` (a `Sketchup::Color`, `[r, g, b]`, `"#rrggbb"` or a color name), `alpha=` |
| UI | `UI.menu(...).add_item` (and submenus), `UI::Command`, `UI::Toolbar`, `UI.messagebox`, `UI.inputbox` (with lists) |
| Loading | `SketchupExtension`, `Sketchup.register_extension`, `Sketchup.require`/`require` of the extension's own files, `file_loaded?`, `LanguageHandler`, `puts`/`print` (shown when the script ends) |

## What isn't (yet)

Tools that respond to the mouse (`Sketchup::Tool`), observers, `UI::HtmlDialog`
and `WebDialog`, timers, `followme`, textures, reading the existing model's
geometry (beyond the selection), files, and Ruby's `eval` of strings.
SketchCraft's own [plugins](../PLUGINS.md) can do mouse tools and forms today.

## Testing

`node tools/ruby-tests/run.mjs` installs a set of scripts written the way real
extensions are (stairs with an input box and nested groups, fence posts from a
component, an `.rbz` extension with a loader and a toolbar, the selection, an
unsupported dialog), runs them and checks what they build. It needs the bridge
and dev page running (see [building.md](building.md)).
