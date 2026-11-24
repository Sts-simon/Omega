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

## 2026-11-24 — Audit fixes after 0.33.0

- **Attachment is transactional.** `Vehicle::attach` applies the edge to a clone and swaps `parts`
  only once the stage count is legal, so a refused move can no longer leave the design in the new
  place while reporting an error. The stage rule itself is unchanged: a move that would drop a
  stage boundary is still refused, now without side effects.
- **Fixed control surfaces use their stability entry.** The `fixed` row in `aero_effects.json` is the
  passive trade (stability 1.25, authority 0); the aggregation now reads it into the fin
  effectiveness term. Moving families keep authority only, so no design changes except one carrying
  a fixed surface, which gains the stability its entry always claimed.
- **Legacy import follows the live schema.** Only `campaign.json` survives from the 0.21 layout; the
  other three files are unreadable by every code path, and importing them left manifest entries
  whose damage would block every later save.
