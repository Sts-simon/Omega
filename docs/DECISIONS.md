# Workshop decisions

Short records for the choices made while rebuilding the missile workshop. Each one names the
simplest option that keeps the game playable and testable; revisit only with a measured reason.

## 2026-11-26 — Contracts use explicit integer goals; revisions stay append-only

The 0.45 offer keeps the existing score thresholds and adds separate integer goals for mass,
reliability and complexity. Integers avoid introducing `f64` into a type that derives `Eq` and is
compared as a persisted board value; the application converts `DesignMetrics` to the same scale at
display and delivery time. Budget and goals are checked before the dossier, but a refusal never
removes the active contract. Secondary goals are all-or-nothing because a partial bonus would make
the reward table depend on an arbitrary weighting; meeting every optional goal pays
`secondary_bonus_cr` once.

Revision restore copies the old vehicle and commits it as a new revision instead of moving `head`
backwards. This keeps every parent chain, submitted id and trial association valid, and makes
"restore" visible in the same append-only history used by the comparison. The submitted revision
is persisted separately from `head`, because a player may record a newer revision after delivery
while the contract still refers to the earlier one.

The delivery PDF is generated from a pure list builder. It repeats the offer context and the
requirement matrix rather than embedding game state, and it filters trial campaigns by both stored
hashes so a dossier cannot cite a campaign flown on a different design or program.

## 2026-11-26 — Workshop ergonomics stay in the session, locks cover subtrees

Multiple selection, collapse, hide, lock, group names, favourites, recents and the palette query
are editor state, not vehicle data. They live in `MissileSession`, are not serialized, and are
summarised by `ui_signature()` so a board rebuilds only when one of them changes; schema 7 and old
saves are untouched. Hover lives in a resource outside the session so moving the pointer can never
mark the design dirty.

Highlights are gizmo wire boxes rather than material or mesh rebuilds: hovering a 45-part design
would otherwise respawn meshes every frame. Hidden parts are skipped by the mesh rebuild, picking
and attachment markers, but `Vehicle` and every rule still see them. A lock covers the whole
subtree: deleting or moving an ancestor that contains a locked part is refused, because otherwise
the protected child would be removed or moved by an edit aimed at its parent.

Camera framing goes through an optional interpolated `CameraGoal` in the rig. `F`, `Home`, the five
view keys and the palette buttons all build the same goal from the selected bounds or the whole
vehicle, which is what makes multi-selection framing and a smooth transition share one path. Free
mode only changes how drag and wheel motion are interpreted; it does not create a second camera.

## 2026-11-26 — Fixed-section parts carry a mounting diameter, and the nose is a part

Sensors, computers, decouplers, payloads and mission electronics have no length/diameter fields of
their own, so fitting them to a wider airframe forced an adapter or left a diameter-step warning.
They now carry a per-placement `size_scale` (default 1, schema 7 unchanged) and the inspector
exposes it as an actual mounting diameter in 5 cm steps. The scale multiplies only the cross-section:
the part's length and mass stay its own, because the complaint was a radial step, not a request for
larger versions of the electronics. Keeping the factor on `PlacedPart` avoids adding a scale field
to every fixed `PartSpec` variant and keeps existing saves valid; the warning, poses, picking and
mesh all read the same scaled dimensions.

The automatic ogive cap is removed. A `NoseCone` part owns its length, base diameter and profile
(conical, ogive, parabolic), contributes its shell mass and is the only thing that closes the top of
the stack. It is a terminal part: adding above it is refused until the player removes it, so a
palette drop can never place a tank through the tip. Presets do not add one implicitly, which keeps
the 0.40 behaviour of an open stack until the designer chooses a cap.

## 2026-11-26 — The inspector edits the existing attachment transform

The numeric inspector reads and writes the six `Attach` fields already stored by schema 7
(`offset_*_m` and `rotation_*_deg`), so direct entry needs no new save version. The precision
selector only controls the `-`/`+` step and its rounding; direct entries are rounded to the field's
displayed decimals (four for metres, three for degrees) so the stored value and the visible value
cannot disagree. All six values are validated before the vehicle is mutated, and the whole field
edit goes through `set_transform_values`, which records one undo entry. A gizmo drag still previews
in `Vehicle` and commits once on release. The initial transform means the default zero mounting,
not a saved per-part baseline, because the schema has no room for one and zero is the mounting the
rules already use.

## 2026-11-25 — Contracts are mission profiles, not only family thresholds

Each mission family now draws from a small set of profiles (deep strike, veil raid, precision
strike, air superiority, long-range intercept, point defence, area defence, theatre strike, sea
skimmer). A profile sets the vehicle family, charge mass and yield, and either a low-altitude
discretion demand or a high-apogee demand. The board takes at most two profiles per family so the
cards still fit on screen. This is deliberately a rules-layer concept: `contracts.rs` owns the
profile table and requirement construction, while the office only stores offers and evidence.

Discretion is derived from the trial's peak altitude, not stored on the design: a low flight is
discreet, a high ballistic apogee is not. The curve is a single gameplay exponential
(`100 * exp(-altitude / 7000 m)`); it is not a sensor or radar-cross-section model. A profile-bearing
contract only records evidence from its own vehicle family, and the same structural design cannot
be delivered twice. The delivery fingerprint serializes the attachment tree with part ids, part
names and child order removed, so renaming the vehicle or every part cannot create a second craft.
The family check prevents cross-profile evidence; the delivery check makes the
one-craft-one-contract rule explicit rather than relying on every profile being numerically
incompatible.

## 2026-11-25 — An explosive charge is an abstract payload grade

A charge is a real workshop part with a charge mass, a casing mass and a game yield. Three grades
(standard, improved, advanced) are separate palette entries because the research gate is already a
per-part capability; the improved and advanced grades are two Materials projects. The grade raises
yield per kilogram, never models chemistry, blast, fragmentation or a real warhead. Contracts ask
for a minimum charge mass and a minimum yield, so the player can trade mass for research or accept
a heavier airframe. High-altitude contracts also need a barometer and a low-pressure hold block;
their apogee requirement is what makes a ballistic flight useful rather than a universal design.

## 2026-11-25 — Cruise vehicles are edited horizontally

The workshop rotates the whole vehicle presentation when it carries an air-breathing engine:
`orientation()` maps the local nose axis to +X and the radial mounts into the horizontal plane.
Picking, free nodes, gizmos and the camera go through the same `world_point()` / `part_rotation()`
helpers, so the layout stays editable instead of being a baked transform. Why an air-breathing
engine: it is the rules' existing cruise marker; a new save field would alter the schema for a
presentation choice. Body sections are rounded rectangles generated in code for the same reason:
no external mesh or texture asset enters the save or the release archive.

## 2026-11-25 — An adapter owns the diameter transition

An adapter is a real airframe part, not a scaling flag on the neighbouring bodies: it carries an
inlet diameter, an outlet diameter, a length and a profile (conical, ogival, stepped). The two
axial faces are what the diameter warning compares, so the adapter absorbs the step instead of the
two bodies warning around it. The mesh is one rounded body whose radius follows the profile; the
nose uses the outlet radius. The profile changes casing mass (1.12 for the ogive, 0.92 for the
stepped form) but not the joint geometry, which keeps the rule explainable and the save small. The
schema moves to 7; older graphs still load because the new variant is additive.

The camera resize path and the gizmo ring point were folded into one helper each while adding the
part. Both keep their previous formulas: only the duplicated expressions moved.

## 2026-11-25 — The workshop camera keeps the model in the free strip

The workshop aims a cruise vehicle above the central inspector with a vertical target bias derived
from the viewport height (2.4 m at 960 px, 3.2 m at 768 px). The same bias is reapplied when the
window is resized, while yaw, pitch, zoom and pan are kept, so opening or resizing at either width
keeps the model readable instead of sliding it behind the panel. The opening view is a light rear
3/4 (yaw -0.4, pitch 0.72): the older -1.05 yaw looked almost down the fuselage and hid the nose
behind the radar. Transform handles are 0.7 m and rings 0.5 m: the previous 1.5 m arrows dwarfed a
2.6 m missile, and 0.9 m put the nose handle under the header at 1280. The nose
is 2.2 top radii long, the turbofan intake faces the nose, and a solid motor ends in a dark nozzle
skirt. These are presentation-only changes; the save schema and design figures are unchanged.

## 2026-11-24 — 0.36–0.42: chains plus shared state before the canvas

For 0.36–0.42 the editor kept the thread model and drew connections between consecutive blocks
instead of a free 2D canvas. Conditions and modes were gates on the current block, with a timeout,
and four flags plus one mode register were the only shared state. Why at the time: the flight already
executed parallel threads and the chain was easy to read without node placement or port hit-testing.
The connection rules stayed real: a terminal block has no successor and an ignition cannot follow
the separation of its stage. This choice is superseded by 0.43 for the presentation only; the
execution model and validation rules described here still hold.

## 2026-11-26 — 0.44 stores whole snapshots so a campaign can be replayed

`TrialHistoryEntry` stores the complete `Vehicle` and `Program` that flew, not only their hashes.
Why: the history must answer "what exactly produced this result?" after the workshop moved on, and
a hash cannot be flown again. The entry also keeps the seed range, launch profile, revision id and
program version so the laboratory can associate a campaign with the design log and identify stale
proofs. The cost is a larger `trials/history.json`; the history is capped and remains separate from
the vehicle, program, contracts and revision files. Relaunch deliberately ignores the current
workshop state: a replay is evidence of the stored snapshot, not of what the player has since built.

## 2026-11-26 — 0.43 adds a layout canvas without changing the language

The editor now stores optional positions, zoom, pan and groups in `ProgramLayout`, separate from the
executable `threads` and `routines`. The canvas is a view over the same blocks and the same
`program_issues()` validation; dragging never changes execution order, and a program saved before
0.43 loads with a neutral layout. Why: free placement, branches and routines make a long chain hard
to scan, while keeping order in the vectors preserves determinism and the old JSON contract. Control
bodies render as rows inside their card, so nesting stays readable without inventing a second
coordinate system for every block. The debugger remains a replay of `FlightReport::trace`, not a
second interpreter; the playback cursor only selects which trace entry colours the cards.

## 2026-11-24 — The debugger reuses the seeded flight

`FlightConfig::trace` records a `BlockTrace` per executed block, and the editor shows that report
instead of running a separate interpreter. A second interpreter would drift from the flight the
trial actually uses, and a trace is cheap: it is off for batch runs and cleared on every edit so the
panel cannot show a stale timeline. The saved library is a separate file
(`programs/library.json`, 12 entries) so the current program keeps its own file and its hash.

## 2026-11-24 — Mission electronics share one shape; only they draw power

Every electronic device answers the same `AvionicsSpec` questions, including the legacy computers
and sensors, so the workshop, the validation panel and the trial cannot disagree about cost, power
or category. The mission bus is drawn by the five new modules only: the computer and the legacy
sensors stay on the vehicle bus. Otherwise every existing save would need a power cell to validate,
which is a rule the player never chose.

## 2026-11-24 — Effects are capped, and duplicates are refused

Authority, lock and fault bonuses are summed and clamped (+20 %, +15 points, −65 %), so stacking
modules is a trade, not a free win. A module may be fitted once (the power cell is the exception,
because capacity is its whole purpose), which keeps the rule explainable and the part list short.
Effects are applied in `flight::simulate` once per run and quoted on the report; no new guidance or
sensor model is introduced.

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
  control surfaces use an oriented box, with the same nearest-hit rule.

## 2026-11-24 — Aerodynamics: surfaces, figures and balance (delivered)

- **Three leaf parts on radial mounts.** `Wing`, `FinSet` and `ControlSurface` are `PartSpec`
  variants; they mount on a body's radial slots, cannot carry children and are excluded from the
  body-diameter reference area. Placing them through the tree, symmetry, undo and picking needed no
  second model.
- **One effect table per choice family.** Planform, profile, stabiliser role, fin layout and control
  family each map to a single `Effect { area, lift, drag, stability, authority, complexity,
  reliability, mass }` in `omega-rules::aero`; the aggregation only multiplies and sums those. No
  coefficient is repeated in the app or in the flight model.
- **Scores are normalised against 0.30 m².** Fin effectiveness and moving-control area divide by
  that reference, clamped to 0..1; lift divides by 0.90 m² so one default wing reads about 50 and a
  pair reaches the top of the scale; the drag indicator divides surface drag area by 0.60 m². This
  keeps a small four-fin set useful without letting one huge wing break the scale.
- **Mass is span × chord.** Areal densities: wing 3.2, fin 4.8, control 5.6 kg/m²; a planform mass
  factor applies on top. The light build multiplies by 0.70 and adds complexity and reliability
  cost. Aerodynamic mass is ordinary dry mass, so stage reports, TWR and delta-v already include it.
- **CG and CA are fictional but consistent.** CG is the mass-weighted mean part position; CA is the
  area-weighted mean of the surfaces only (wing 1.0, fin 0.55, control 0.25). The margin is `CG−CA`,
  i.e. positive when the CA is behind the CG. The acceptable band is 2 %–10 % of vehicle length;
  outside it is a warning, and ahead of the CG is reported as unstable. No real static-margin or
  centre-of-pressure formula is used.
- **Flight effects.** Surface drag area is added to the body term (`DRAG_COEFFICIENT` still folded in,
  so a surface-less design is unchanged); lift lowers the air-breathing minimum from 90 m/s by up to
  45 % (floor 55 %) and the pre-pitch-over gravity by up to 35 %; stability damps the seeded drag
  spread by up to 60 %; stability and authority raise the altitude-hold rate by up to 50 %. The
  figures are computed once per run, in `AeroSummary` on the report, so seeds stay deterministic.
- **Compatibility is a rule, not a guess.** Integrated surfaces need a standard computer, combined
  surfaces an advanced one; fixed and classic surfaces work with the basic computer. Unlocked
  advanced profiles/variable geometry/asymmetric fins need `aero:advanced_profiles`, and the light
  build needs `aero:light_structures`.
- **Contracts stay soft.** Only surface-to-air offers add `STABILITY ≥ 20`, with weight 0.5 and
  `hard: false`; a marginal airframe is a compromise, not an impossible contract. Evidence carries
  the stability of the trial's design, so an edit invalidates it through the normal hash check.
- **Save schema 4.** New `PartSpec` variants; schema 3 and earlier load with no surfaces and rewrite
  as 4 on the next edit. A newer schema is still refused.
- **Boards sized for a real airframe.** A design with wings, fins and control surfaces adds up to
  eighteen rows, so the workshop stage list is capped (56 % of the column) and scrollable and the
  aero snapshot control sits under the panel title; the five aero technologies form their own
  research department and the research grid scrolls. Palette and drag edits persist immediately:
  only `UiAction` clicks used to save, so a palette-only edit was lost on the next launch.
- **Values accepted after the balance tests.** Four fins (span 0.3 m, chord 0.35 m) on a 0.4 m body
  score 100 stability and damp the seeded speed spread; one 1.2 × 0.5 m swept wing lets the turbofan
  run at 70 m/s while the wingless design never accelerates; four classic control surfaces settle the
  900 hPa target about 25 % sooner. No constant needed a second tuning pass.

## 2026-11-24 — Post-aerodynamics follow-ups

- **Reliability gets one consumer, not a system.** Below `SURFACE_FAULT_RELIABILITY` (60) a design
  exposes `surface_fault_chance()` (linear, capped at 35 % at the 25 floor); the trial makes one roll
  from the seed, halves control authority for the run and records an anomaly. A sound airframe never
  draws it, so no existing flight changes, and the light build stops being a free mass saving. No
  per-part failure model is introduced.
- **The fee follows the dossier score.** Nominal at `ACCEPTANCE_THRESHOLD` and +15 % at a perfect
  dossier, linear in between; below the threshold there is no fee because the dossier is refused.
  This is the only economic effect of quality, and it keeps contracts affordable at the bottom.
- **Aero requirements follow the family's purpose.** Cruise asks `LIFT ≥ 40`, air-to-air
  `AUTHORITY ≥ 25`, surface-to-air `STABILITY ≥ 20`; all are soft (weight 0.5) so a compromise never
  blocks a contract, and all proofs are test-run evidence bound to the design hash.
- **Research durations are the lever, not costs.** The tree's 63 000 cr budget stays, so the economy
  target is unchanged; serial days drop from 90 to 67 (aero 22 → 16) and a test pins both totals.
  Costs were left alone because credits have a second source (delivery bonuses) and changing them
  would move every existing campaign's balance.
- **Balance data is a file.** `aero_effects.json` is embedded, parsed once, and a missing entry is a
  hard error instead of a silent neutral fallback; the report is generated from the loaded tables so
  the documentation cannot drift from the rules. Only design choices are persisted, so this changes
  no save schema.
- **The CLI is the verification surface.** Flight, comparison, export and balance live behind flags
  that open no window; the UI keeps only the parts that need a pointer. `--export-design` writes the
  vehicle itself, and `--import-design` validates before writing through `SaveSlot`, so a broken
  export cannot corrupt a campaign.
- **Explainability is derived, not stored.** `aero::breakdown` recomputes contributions from the same
  `Effect` tables; warnings carry an optional part id rather than a new warning type in the save.

## 2026-11-26 — 3D assembly layer (0.42.0)

- **Eight radial mounts, not a new node type.** Bodies expose `Radial(0..8)`; the first four keep
  their old 90° positions, the last four add 45° points. `radial_angle(spec, index)` is shared by
  poses, layout, picking and the preview, so a body and every consumer agree on where a mount is.
  Existing designs that use the first four mounts are unchanged.
- **Symmetry stays inside schema 7.** The existing `symmetry` string now reads
  `<key>-<kind><index>` (`r` radial, `b` bilateral, `a` axial). A legacy `<key>` (for example
  `sym-1`) decodes as radial index 0. This avoids a vehicle migration and keeps old four-way sets
  linked while adding modes, counts and a copy index.
- **A symmetry edit is a delta mapped per copy.** Parameter and design-choice edits are applied to
  every member directly. A gizmo translation is rotated around Y for radial sets and mirrored on X
  for bilateral sets; an axial set receives the 180° turn. A rotation delta follows the same mode.
  The alternative — recomputing every copy from one anchor — was rejected because old sets do not
  store enough information to prove which copy is the anchor.
- **Duplicate onto free mounts; break the clone's link.** A duplicated subtree keeps its inner
  `parent` edges and is attached to a free mount of the same parent. Clones start with `symmetry:
  None`, so editing one does not silently drag the original set. A root part cannot be duplicated
  because a vehicle allows exactly one root; the player is told to select a subassembly instead.
- **Library files sit outside the slot.** Saved sets live in the slot (`designs/subassemblies.json`,
  own schema and atomic write), while deliberate exports are plain JSON files in
  `exports/subassemblies/<slug>.json`. Import scans that folder, validates every set and reports
  unreadable files without hiding the valid ones. This is what makes a set reusable in another
  campaign instead of being bound to one slot.
- **The placement ghost is presentation only.** Automatic snapping uses a 140 px search radius and
  the nearest free mount; the tighter 28 px `SNAP_PIXELS` only makes the exact node marker larger.
  The ghost mesh is built from the real part dimensions and the rules' `node_status` decides
  green/red; releasing on red attaches nothing. No rule lives in the render pass.
- **Assembly control reads the shared report.** `MissileSession::assembly_report` classifies the
  existing `Vehicle::issues`, then adds editor locks and named-group connectivity. The panel is a
  view; it never reimplements a rule, and every row keeps a part id so one click selects it.
