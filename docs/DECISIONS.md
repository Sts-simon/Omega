# Workshop decisions

Short records for the choices made while rebuilding the missile workshop. Each one names the
simplest option that keeps the game playable and testable; revisit only with a measured reason.

## 2026-11-23 — The vehicle is an attachment tree, stages are derived

`Vehicle` stores a list of placed parts, each with an optional attachment (`parent`, node, angle)
and the stages are computed from the tree. A stage boundary is an explicit `separation` flag or a
decoupler between parent and child. The flat `Stage` list survives only as a view
(`Vehicle::stage_views()`) consumed by the program rules, the flight model and the dossier.

Why: radial boosters, multiple stages and free placement cannot be expressed by a list of stages,
while every existing rule and test already speaks in stages. Deriving the view keeps those rules
and their figures unchanged.

## 2026-11-23 — Migration from schema 1-2

An old design is chained bottom to top within each stage (engines first, as the old layout drew
them), and the link between two old stages gets a separation flag when there is no decoupler at the
boundary. This preserves the stage count and therefore every mass, thrust, TWR and delta-v figure.
The migration runs when `designs/vehicle.json` is deserialized; schema 3 is written back on the next
edit. A newer schema is refused with a message instead of being reinterpreted.

## 2026-11-23 — Node rules

`Top` and `Bottom` take one child each; `Radial(0..3)` exist on cylindrical bodies (motors, tanks,
mission modules) and take one child per slot. A node can never be used twice. Attaching a part into
its own subtree is refused as a cycle. A diameter difference between a child and its parent is a
**warning** (`Vehicle::warnings()`), not an error: drag is not modelled yet, so refusing the design
would add a rule the game cannot explain.

## 2026-11-23 — Adding a part from the palette

The palette adds above the selected part's `Top` node when that node is free; otherwise above the
highest part of the stack. An empty vehicle takes the part as its root. A second flight computer
replaces the existing one in place (same part id), so the "one computer, last stage" rule holds.
Deleting a part deletes its whole subtree and says how many parts went with it.

## Open decisions for the interaction lot

These are not implemented yet; the values below are the ones to use when the drag-and-drop layer
lands, unless a measurement says otherwise.

- **Drop outside the view** cancels the drag; dropping over the palette deletes nothing (explicit
  `Delete` plus a confirmation count is the only removal gesture).
- **Snap radius** 28 screen pixels around a node, nearest compatible node wins; no node in range
  cancels the drop.
- **Undo stack** bounded at 50 vehicle snapshots, in memory only, cleared when the preset changes.
- **Symmetry** cycles 1 → 2 → 3 → 4 with `X`, copies share a `symmetry` group id and are placed on
  consecutive radial slots.

## 2026-11-23 — Pointer interaction (delivered)

The interaction lot kept the values below; each is now covered by code and by a game check.

- **Picking** is a camera ray against each part's cylinder (`src/picking.rs`), not Bevy's mesh
  picking: identical results for these primitives, no extra plugin, and 5 pure unit tests.
- **Click versus drag** uses a 5 px threshold. A press selects immediately (so the inspector follows
  the pointer) but the part only starts moving past 5 px; without it, releasing a plain click over a
  free node moved the part under the cursor.
- **Drop outside a node cancels** and never deletes. Dropping a palette entry back over the palette
  is a plain click and adds above the selection, so the old click-to-add path still works.
- **Symmetry copies** share `sym-<undo-depth>` as their group id and occupy consecutive radial slots
  starting at the dropped slot; if any slot is taken the whole drop is refused before any mutation,
  so no orphan copies remain.
- **Undo** is a stack of `Vehicle` clones bounded at 50, memory only; a preset change clears both
  stacks. Every mutation (add, move, delete, adjust, computer swap) pushes one snapshot.
- **Keyboard shortcuts use logical keys** for `Ctrl+Z` / `Ctrl+Y`. On the French AZERTY layout in use
  on the test machine, `Ctrl+Z` arrives as the physical `KeyW`; a `KeyCode::KeyZ` binding silently
  never fired (found in game).

## 2026-11-24 — Wheel gestures, Escape and picking

- **One undo entry per wheel gesture.** Notches on the same part and parameter less than 500 ms apart
  share the snapshot of the first notch; the session closes the gesture on undo/redo, on any
  add/move/delete/computer swap, on a `+`/`-` button edit and on the 500 ms pause. Without this, a
  spin of the wheel filled all 50 history slots.
- **Escape cancels a drag before leaving.** `WorkshopInput::drag_active` is published at the end of
  the workshop system and read by `campus::handle_input`, which runs first: while a drag (or a press
  about to become one) is active, Escape only clears it. A second Escape behaves as before.
- **Picking shapes** stay in `picking.rs`. Bodies are ray-cast against their cylinder; wings, fins and
  control surfaces will add a box path in the aerodynamics lot, with the same nearest-hit rule.

## Open values for the aerodynamics lot

These are proposals, not decisions: settle them at the end of phase 1 and record the measured
figures in this file.

- Surface areal density (kg/m²) for wing, fin set and control surface, and whether mass scales with
  both span and chord or only area.
- Body drag coefficient, surface form factor and the sweep penalty: one value per Mach band or a
  single smooth factor.
- Lift model: the reduction of the air-breathing minimum speed and the pitch-over gravity loss as a
  function of `wing_area / reference_area`, with floors so a huge wing cannot make a design free.
- Stability and control effects: the dispersion reduction curve and the controller error improvement;
  whether a flight without fins can still be launched vertically.
- Whether mission families impose hard stability/control requirements or only make the offer
  unreachable in practice.
- Tech costs and prerequisites for the three new capabilities; whether wings also need the cruise
  family or only the aerodynamics node.
