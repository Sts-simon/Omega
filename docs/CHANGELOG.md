## 0.18.0 — Mouse-first navigation and contextual UI
- Added a persistent top navigation bar for Island, Contracts, Workshop and Laboratory views.
- Added scrollable contextual tool shelves for the Contract Office, Workshop and Computation Lab.
- Added mouse-clickable component selection, axis translation, axis rotation and per-axis dimension controls in the Workshop.
- Added CAD-style camera controls: right-drag orbit, middle-drag pan and wheel zoom. Pixel-based touchpad scrolling is normalized separately from line-based wheel input.
- Prevented camera movement and world-space picking while the pointer is over an interactive UI panel.
- Repositioned and made HUD panels scrollable so navigation, inspectors and result queues do not cover one another.
- Kept the scientific solvers and saved assembly/campaign/polygon schemas unchanged.

## 0.17.2 — Sketch system visibility and application compile cleanup
- Matched the crate-level Bevy schedule visibility with `pub(crate)` sketch resources, marker components and systems.
- Kept the polygon sketch session as the exported resource API while restricting internal render/query types to the application crate.
- Used the assembly mesh work-plane height in the research job summary.
- Removed an unused contact-tolerance constant and retained the point-add convenience method in the normal drawing path.
- Exposed undo availability in the workshop HUD and removed the resulting dead-code warnings.
- No save schema changes; all 0.17.1 sketch and assembly files remain compatible.

## 0.17.1 — Workspace integration compile fixes
- Fixed region-scoped mutable ring lookup in selected-vertex deletion.
- Fixed optional coordinate extraction in `vertex_at` with `Option::copied`.
- Avoided overlapping immutable/mutable borrows of the polygon sketch `ResMut` when entering hole mode.
- Split the 21-system Update chain into three nested chained groups, preserving execution order while respecting Bevy tuple arity.
- Removed unused re-exports in `design` and `jobs`.
- No data schema changes.

## 0.17.0 — Polygon vertex editing and snapping
- Added an explicit vertex-edit mode with visible handles for outer contours and holes.
- Added click-and-drag movement with live updates to only the incident edges and selected marker.
- Added 5 cm grid snap, nearest-vertex capture within 12 cm, a snap toggle and Shift bypass.
- Validated edited regions before commit; invalid geometry or a failed save restores the original vertex.
- Added safe selected-vertex deletion with a three-vertex minimum per ring; deleting an interface vertex shared by multiple regions is blocked.
- Moving a welded vertex moves every coincident region copy together to preserve shared material interfaces.
- Kept polygon sketch schema 1 unchanged for compatibility with 0.16.0 files.
- Added tests for snapping, hole vertex identification, drag cancellation, invalid-move rollback, self-snap exclusion and shared-interface vertex behavior.

## 0.16.0 — Interactive polygon sketch and multi-region meshing

- Added an interactive Workshop polygon sketch mode with mouse-added vertices, contour validation, hole drawing, multiple regions, material selection and thickness controls.
- Added persistent sketch documents at `saves/polygon-sketch.json` and in-scene colored contour visualization.
- Added a multi-region polygon mesh adapter that welds complete shared edges and preserves element material/thickness/source provenance.
- Added overlap and contact validation, plus Computation Lab `P` to queue a sketch-derived thermal study.
- Added tests for material interfaces, node welding, overlap rejection, point-only contact rejection, input limits and JSON round-trip.

## 0.15.0 — Concave polygon meshing and holes
- Added `PlanarPolygonRegion` and `mesh_planar_polygon` to the independent simulation crate.
- Added constrained triangulation using `earcutr` for simple concave outer contours with up to 64 holes.
- Added contour cleanup, winding normalization, simple-ring checks, strict hole containment/non-contact checks and total vertex budgets.
- Added triangulated-area verification and full triangle validation, preserving material, thickness and source-part provenance.
- Added a headless polygon thermal study example and a JSON input deck containing an L-shaped region with two interior openings.
- Added tests for concave polygons, one and multiple holes, reversed winding, repeated closure points, collinear vertices, invalid holes, self-intersections and thermal-solver integration.
- The workshop editor remains rectangular; polygon editing and multi-region polygon interfaces are future work.

## 0.12.0 — Scientific visualization and report viewer
- Added a Bevy-independent HTML/SVG and CSV exporter for thermal and civil truss results.
- Added block-averaged thermal field heatmaps with full-resolution CSV data and energy diagnostics.
- Added structural reports showing undeformed/deformed geometry, tension/compression coloring, supports, applied loads, member tables and nodal/member CSV values.
- Added a civil truss study to the in-game research queue (`G`) and opening the latest HTML report in the default browser (`O`).
- Kept exact JSON solver reports and added result-shape, identifier and finite-value validation before export.
- The deformed shape is visually amplified and explicitly labeled; exports preserve unamplified SI quantities.

# Changelog

## 0.26.0 — Missile workshop and working research (0.25 completed)
- **Missile workshop** (`missile.rs`, `campus/board.rs`): the WORKSHOP tab now builds a staged vehicle in 3D from the parts of `aerolab-rules::vehicle`. Add parts to the active stage, add or remove stages (up to 4), select a part and resize it with +/- buttons (lengths, diameters, solid-motor burn time, engine scale, module mass), remove it, or start from a preset (two-stage rocket, cruise vehicle) or an empty vehicle. The single flight computer always sits in the last stage and follows new stages.
- Live figures per stage: mass, mass carried, thrust, burn time, thrust-to-weight ratio (red below 1 and amber below 1.3 for the first stage) and delta-v; total mass; computer class with its thread and memory limits; the programming blocks the installed sensors unlock; research still missing; design validation messages.
- The 3D stack is rebuilt on change, with a highlight on the selected part and a nose cone; the camera centres on the vehicle when the workshop opens. The vehicle is saved in the slot (`designs/vehicle.json`) on every change and reloaded at startup (invalid parts are rejected on load).
- **Research Centre is now playable:** 13 technologies in five departments with cost, duration, prerequisites and status (RESEARCHED, N DAYS LEFT, LOCKED, NEED x cr, LAB BUSY, START RESEARCH). One project at a time; the new **NEXT DAY** button advances the game clock (day counter in the navigation bar). Credits are deducted when research starts; research state and day are saved in the campaign (older campaigns load with the defaults).
- **Parts are gated by research:** locked parts are greyed and explain which technology to research (technology names, not internal ids). Capabilities come from the starting set plus completed research.
- The old generic assembly bench is kept as the **LEGACY BENCH** tab (and from the active-contract card), because civil contracts still run on it until the missile-family contracts arrive.
- Not done: programs are not edited in the application yet, there is no trial screen, contracts are still the civil ones and do not use the missile vehicle, and the missile design is not part of revisions or evidence. Research does not yet limit contract families or the number of active projects (those grants exist but nothing checks them).
- Tests (Linux sandbox): `sim-core` 71, `aerolab-data` 10, `aerolab-rules` 51, application 121; all pass. Exercised under a virtual display with software rendering: open the workshop, select and resize a part, click a locked part, start a research project, advance days until it completes, and add the newly unlocked radar.

## Unreleased (0.25 groundwork) — missile design core and research tree
- Game direction restated in `docs/VISION_MISSILES.md`: contract for a missile family, KSP-style staged build with resizable parts, sensors that unlock programming blocks, a computer that limits threads and memory, seeded trials, delivery of a plan (no production). Fictional and abstract by design; the lock block is a probability roll, with no guidance logic.
- `aerolab-rules::vehicle`: parts (solid motor with length, diameter and burn time; tanks; liquid and air-breathing engines with a scale; sensors; computers; decouplers; mission module), validation, per-stage mass, thrust, burn time, thrust-to-weight and delta-v, installed-hardware to block mapping, and missing-research check.
- `aerolab-rules::program`: seven blocks (wait, ignite, separate, throttle, hold speed, hold pressure, lock target) validated against the hardware and the computer (threads, memory), with lock target required to close its thread.
- `aerolab-rules::flight`: seeded, deterministic abstract flight test with events, anomalies (propellant dropped unburnt, out-of-order separation, lock above radar limit, radar jettisoned, cannot lift off, nothing ignited) and a report of speed, path, altitude, mass and thrust over time.
- Research tree rewritten for this loop (missile families, propulsion, sensors, computers, design tools) plus a set of starting capabilities. 51 tests in the rules crate.
- **Not wired into the application:** none of this is visible in the game yet.
- (Earlier in this entry) `aerolab-rules::research`: technologies with department, cost, duration, prerequisites and grants (named capabilities such as `part:cylinder`, `analysis:truss`, or +1 active project); tree validation (unique ids, known prerequisites, no cycles); one research project at a time; `start`, `advance_days`, capability and project-limit queries; JSON-serializable state. A 10-node starting tree covers the five departments. 6 unit tests (rules crate: 25/25).
- **Not wired into the game yet:** no game clock, no research board with START buttons, no persistence in the campaign, and no facility checks the capabilities. The Research Centre still shows PLANNED cards.

## 0.24.1 — Interface overhaul (cards, status chips, readable workshop)
- **Shared look** (`ui_kit.rs`): one palette, rounded cards with a coloured top edge, status chips, and buttons with tones (primary, secondary, danger, ghost, disabled) that react to hover and press. Thousands separators for credits.
- **Island:** a title with live stats and five facility cards (F1-F5 chips, short description, OPEN button) instead of a text wall.
- **Contract Office** (`campus/board.rs`): offers are cards (domain colour, difficulty pips, key limits, reward and deadline, ACCEPT; disabled look when two projects are active). Active projects show a status chip, a 4-step progress bar (design, analysis, field test, delivery) and context buttons (SELECT, WORKSHOP, COMPUTE, DELIVER). The selected project's dossier is a table of coloured chips (OK / THIN / FAIL / STALE / OPEN) with a verdict, next to the last completed contracts. Boards are rebuilt only when the data they show changes.
- **Workshop:** the left shelf now uses compact rows (35 stacked buttons became about 18 buttons in rows; rotation and size remain in the exact inspector); the polygon-sketch panel is hidden unless sketching; mouse/keyboard help is folded behind `H` / SHORTCUTS; a project card at the bottom shows the dossier chips, issues, profile and SUBMIT DESIGN. Panels no longer overlap. Record/restore revision are buttons.
- **Status messages** appear in the navigation bar (they used to be buried in the text panel and never expired; they now fade after 7 s). The bar also shows credits, active projects and completed contracts.
- **Research Centre:** five department cards marked PLANNED (the tree itself is still version 0.25).
- Not redone yet: Computation Centre and Programming Studio still use the earlier text panels; the instrument-profile editor (F11) is still a text panel; no scrolling inside boards (they are laid out for windows about 960 px tall).
- Tests: `sim-core` 71/71, `aerolab-data` 10/10, `aerolab-rules` 19/19, application 111/111. The interface was exercised under a virtual display with software rendering: opening every view by clicking, accepting an offer, submitting a design from the project card, and reading the dossier.

## 0.24.0 — Design revisions, comparison and live budget (partial)
- **Revisions** (`design/revisions.rs`): `Ctrl+B` records the current assembly as a numbered revision whose parent is the previous head; an unchanged design is refused. `Ctrl+Backspace` restores the latest revision. The log is saved in the slot (`designs/revisions.json`, schema 1, validated on load) and reloaded at startup; it keeps at most 100 revisions (the oldest is dropped and its children detached).
- **Comparison** (`aerolab-rules::diff_parts`): the Workshop inspector shows what changed since the latest revision (parts added, removed, moved with the largest distance, resized, mass delta, change of CG offset in percentage points).
- **Live budget** in the Workshop inspector: mass (with its analysis uncertainty), envelope length and CG offset as a fraction of length.
- On-screen text fixes found with screenshots: characters missing from the default font (`x`, degrees, minus, plus-minus) are now ASCII; the Research Centre text no longer announces a past version; version labels read 0.24.0 (the programming studio still said 0.10.1).
- **Not done from the 0.24 plan:** typed vehicle modules, axial (vertical/horizontal) stacking by vehicle family, and a side-by-side view of two revisions. Revisions are linear snapshots of the whole assembly; there is no branch selection UI yet.
- **Known UI defects seen in screenshots (not fixed):** the Contract Office text is taller than the window with 5 offers, so the verification matrix is only readable in the Workshop; in the Workshop several panels overlap (polygon-sketch panel over the component inspector, project panel over the sketch panel, and its last lines fall below the window); the keys `1`-`5` also add components in the Workshop, so they do not accept contracts there.
- Test status (Linux sandbox, software rendering): `sim-core` 71/71, `aerolab-data` 10/10, `aerolab-rules` 19/19, application 109/109. The game was also run under a virtual display to take the screenshots, which exercised: opening every view, accepting a contract, submitting a design, recording two revisions, comparing, restarting and reloading revisions and the campaign.

## 0.23.0 — Evidence, verification matrix and dossier-gated delivery
- Contract limits are now turned into requirements (`campus/verification.rs`): mass, unit cost, balance (when present), power, temperature range, storage, uptime and data completeness.
- Every check produces **evidence bound to the design hash** and stored in the campaign (`ActiveContract.evidence`):
  - at design submission, *analysis* evidence: mass, balance and cost derived from the real geometry with an analysis uncertainty (mass 3%, cost follows it), plus the exact values declared in the instrument profile;
  - at the field test, *test* evidence for uptime and data completeness, with the standard error of the 1,000-sample measurement (about 0.6 points at 96%) as uncertainty.
  Uptime and data completeness can only be proven by a field test, so they show as open until one is run.
- The selected contract shows a **verification matrix** in the Contract Office, Workshop and Computation Centre (`[OK]`, `[THIN]`, `[FAIL]`, `[STALE]`, `[OPEN]`) with value, uncertainty, margin and method, and a verdict line. Evidence becomes `[STALE]` as soon as the assembly changes.
- **Rule changes (gameplay):** (1) submission now also blocks a mass or cost whose analysis uncertainty crosses the limit (a "thin margin"); (2) delivery requires the generated dossier to be accepted: a field test that passes by less than one standard error is no longer enough; (3) completed contracts record the dossier score. Campaigns saved before 0.23 are upgraded transparently: evidence is rebuilt at delivery time from the current design and field-test report.
- The cost formula was extracted into `estimated_unit_cost_cr` (unchanged values).
- Test status (Linux sandbox): `sim-core` 71/71, `aerolab-data` 10/10, `aerolab-rules` 15/15, application 104/104. Not run on screen; the new HUD text and the delivery gating have only been exercised through unit tests.
- Still open: `InstrumentProfile` is still a set of declared numbers; the matrix is text in the HUD, not a dedicated panel; programs and revisions are not part of the evidence yet (`program` is always empty).

## 0.22.0 — Shared data layer, verification rules and a geometry-derived requirement
- Added `crates/aerolab-data` (no Bevy): `ContentHash` change-detection fingerprints, crash-safe `write_atomic` (temp file, fsync, rename, one `.bak` generation) and `SaveSlot`, a versioned save slot whose `manifest.json` (per-file hash and size) is committed last. Includes a one-time, non-destructive import of the four 0.21 save files.
- Added `crates/aerolab-rules` (no Bevy): `Requirement`, `Estimate` (value plus uncertainty), `Evidence` bound to design/program hashes, `evaluate_requirement` (Verified / Marginal / Failed / Stale / Unverified / MethodNotAllowed), `build_dossier` (verification matrix, weighted score, acceptance decision with reasons) and `derive_metrics` (mass, length, centre of gravity and balance ratio from real part geometry).
- Contracts now carry an optional geometry-derived balance requirement (`max_cg_offset_ratio`, tighter at higher difficulty). `evaluate_design` checks it through the rules crate, so moving parts in the Workshop changes whether a design passes. Older campaign files load unchanged (the field defaults to none).
- **Save slot is now used by the application** (`src/save.rs`): campaign, assembly, polygon sketch and program are written to `saves/slot_1/` (`campaign.json`, `designs/assembly.json`, `designs/polygon-sketch.json`, `programs/lab_sequence.json`) through atomic writes, with a hashed `manifest.json` committed last. On first access, existing 0.21 files in `saves/` are imported once; they are left untouched and ignored afterwards. A file that no longer matches its manifest entry is reported instead of loaded (campaign: a new campaign starts, and the previous generation stays as `<name>.bak`).
- **Not yet wired:** the dossier, evidence store and verification matrix exist as library code but are not yet shown or stored by the campaign. Part rotations are ignored by the metrics. The save slot is fixed to `slot_1` (no slot selection UI).
- No change to the assembly, program or polygon-sketch schemas.
- **Build fixes needed for Bevy 0.20.1 / rustc 1.97:** the deprecated `Button` marker is no longer usable as a value, so buttons now spawn `Interaction` + `FocusPolicy::Block` and two `With<Button>` query filters were removed; nine component/resource types became `pub(crate)`. Compile-checked only, not run on screen.
- **Fixes found by running the tests:** polygon-sketch snapping produced floating-point noise (`-0.15000000000000002` instead of `-0.15`); snapping now divides by the grid count. Two outdated test expectations were corrected (`storage`: the nearest part to the selected base plate is `sensor-case`; `meshing`: the error text now says "isolated point contact").
- **Test status (measured in a Linux sandbox):** `sim-core` 71/71, `aerolab-data` 10/10, `aerolab-rules` 15/15, application 95/95. Not run on screen.

## 0.14.0 — Conforming local mesh refinement
- Added deterministic longest-edge bisection of marked triangular mesh elements.
- Updated every neighbour sharing a bisected edge so refinement remains conforming and does not create hanging nodes.
- Added axis-aligned local refinement regions, target edge lengths and a bounded iteration budget.
- Preserved per-element material, thickness and source-part provenance.
- Added mesh quality metrics (minimum angle, radius ratio, maximum edge and minimum area).
- Added a headless refinement example and regression tests for conformity, quality and conservation of area/volume.
- Added `Shift+M` in the Computation Lab to locally refine the centre of the workshop mesh before thermal analysis.


## 0.13.0 — Workshop plate meshing and unstructured thermal FEM
- Added a deterministic triangular mesher for adjacent coplanar rectangular workshop `Panel`/`Platform` parts; nodes are shared across matching boundaries.
- Added per-element material ID, thickness and source-part provenance.
- Added fail-fast checks for positive-area overlap, point-only/partial-edge contact, tilted/stacked geometry, invalid triangles, non-manifold edges and size limits.
- Added a Bevy-independent unstructured heat-conduction solver using linear triangular finite elements, lumped heat capacity and CSR stiffness assembly.
- Added fixed-temperature outer boundaries, volumetric source input, explicit stability validation, cancellation, progress and global energy diagnostics.
- Added an HTML/SVG mesh heat map and full-resolution nodal/element CSV output.
- Added an assembly adapter that converts coplanar horizontal workshop panels/platforms into a conforming mesh and reports unsupported component types.
- Added `M` to the Computation Lab to mesh the current workshop geometry and queue a background study.
- Added a two-material JSON input deck, a headless runner and adapter/solver/report regression tests.
- Limits: 2D plate meshes only; no stacked 3D bodies, arbitrary CAD, holes, contact resistance, anisotropic materials or certified engineering claims.


## 0.11.0 — Standalone 2D civil truss solver
- Added a Bevy-independent linear-elastic solver for pin-jointed civil trusses.
- Assembles a sparse global stiffness matrix and solves it with Jacobi-preconditioned conjugate gradients.
- Parallelizes row-wise matrix-vector products with Rayon while keeping deterministic row and dot-product ordering.
- Reports nodal displacement/reaction, member force/stress/strain, solver residual and strain-energy/external-work consistency.
- Added a civil bridge input deck, headless runner and analytical/reference tests.
- Explicitly documents the model limits: no bending, buckling, plasticity, fatigue, dynamics or 3D continuum FEA.

## 0.10.1 — Exact floating-point JSON round-tripping
- Enabled serde_json's `float_roundtrip` feature in both the application and the independent simulation crate.
- Scientific `f64` values now survive JSON save/load without the small last-bit changes produced by the default best-effort parser.
- Kept the strict equality assertion in the complete thermal-result round-trip test and added a focused regression test using representative IEEE-754 values.
- No numerical solver equations, input schemas or result schemas changed. The only runtime trade-off is slower JSON float parsing; it does not affect the solver's time-stepping loop.

## 0.10.0 — Independent scientific engine and finite-volume solver
- Extracted numerical code into `crates/sim-core`, a Bevy-independent Rust library.
- Replaced the unitless five-point stencil with transient 2D finite-volume conduction for a solid slab with SI dimensions and thickness.
- Added material catalogues and per-cell material maps with harmonic-mean interface conductivity.
- Added insulated and fixed-temperature boundaries, a volumetric heat source and a per-cell explicit stability check.
- Reports now include the final temperature field, simulated time, stability limit, initial/final energy, boundary/source energy and energy-balance residual.
- Added independent tests for conservation, uniform-field invariance, boundary cooling, heterogeneous materials, thread-count determinism, stability rejection and cancellation.
- Solver revision 2.1 precomputes per-cell heat capacities and internal/boundary face conductances once, reducing repeated material lookups in the parallel time-step loop.
- Uses compensated energy sums, a numerically safer harmonic mean and an incremental mean-temperature calculation.
- Updated the worker queue to persist full finite-volume result reports.
- The solver remains a generic material study and is not coupled to the 3D assembly or intended as a certified engineering model.

## 0.9.0 — Contract-to-delivery campaign loop

- Connected the three campus facilities into a playable project loop: accept contract, submit assembly/profile, run linked laboratory analysis, perform a field test, deliver the production batch and receive payment.
- Added contract-specific design checks for estimated assembly mass, civilian instrument power draw, operating-temperature envelope, uptime target, data completeness, storage and per-unit production budget.
- Added a configurable civilian instrument profile with keyboard controls and revision tracking.
- Linked background thermal-screening jobs to contract IDs; successful completion unlocks the field-test stage.
- Added a deterministic 1,000-sample instrument availability/data-completeness field-test report written to `results/`.
- Added production-cost deduction, client payout, net-profit history and a completed-contract ledger.
- Added atomic campaign save/load at `saves/campaign.json`, including offers, active/completed projects, credits, profile and PRNG state. Interrupted background analyses are recoverable as retryable jobs.
- Reduced default demonstrator and newly-created structural component dimensions to lightweight instrument-scale assemblies; lowered editable dimension minimum to 2 mm.
- Added tests for campaign serialization, design snapshots, deterministic contract generation and revised requirement ranges.
- Explicitly documented that the thermal-screening task is still a generic heat-diffusion demo and that the field-test model is a gameplay abstraction, not an engineering qualification standard.

## 0.8.0

- Replaced fixed demo contracts with a randomized board containing five scientific/civil-aerospace domains.
- Added seeded contract generation, shuffled domain presentation, unique offer IDs and repeatable tests.
- Added per-offer client, title, difficulty, unit count, instrument mass/power limits, operating-temperature interval, uptime and data-completeness thresholds, storage capacity, production budget, prospective reward and deadline.
- Added `N` to refresh the board while preserving active contracts.
- Expanded offer acceptance to keys `1` through `5`; accepted offers are removed from the available list.
- Added tests for domain coverage, deterministic generation, constraint ranges, refresh behavior and the active-contract limit.
- Contract families are deliberately non-weaponized: scientific sounding, long-duration observation, ground weather networks, cooperative civil-air telemetry and environmental air-ground mapping. No missile-class performance or weapon-system requirements are generated.
- Contract validation, payouts, deadline countdown and career persistence remain future work.


## 0.7.0 — Interactive island campus

- Added three clickable facilities: Contract Office, Assembly Workshop and Computation Lab.
- Added facility navigation from the island and Escape-to-return behavior.
- Added a prototype contract board with three generic research/equipment offers, duplicate protection and a two-active-contract limit.
- Connected the workshop to the existing assembly editor and visual sequencer.
- Connected the computation lab to the existing background thermal-diffusion research queue.
- Hide assembly/research/sequencer interfaces outside their corresponding facilities.
- Assembly component visuals are now spawned only while the workshop is open.
- Added campus behavior tests and documentation.

## 0.6.3 — Graph editor borrow-checker fixes
- Copied the hit-tested block ID and position before mutating `SequenceWorkspace`.
- Copied the drag offset before taking a mutable borrow of the selected block.
- Preserved graph hit-testing, drag offsets, and movement clamps; no save-schema changes.

## 0.6.2 — Bevy 0.20 input and window marker fixes
- Imported `bevy::window::PrimaryWindow` explicitly where primary-window queries are used.
- Changed the breakpoint shortcut to `KeyCode::KeyB`, the Bevy 0.20 physical key variant.
- Removed currently unused re-exports that caused compiler warnings.
- No runtime model or save-schema changes.

## 0.6.1 — Compile visibility and Bevy prelude fixes
- Replaced the narrow Bevy import list in the research job manager with `bevy::prelude::*`, covering `Color`, `default`, and related UI types used there.
- Made marker/resource types used in public Bevy system signatures visible (`ResearchHud`, editor markers/assets/revision, sequence HUD and graph markers).
- Kept the runtime behavior and serialized project schemas unchanged.
- This is a compile-fix release; a full Cargo build still needs to be run on a machine with the pinned Rust/Bevy toolchain.

## 0.6.0 — Visual sequence graph and debugging
- Added a draggable Bevy UI node canvas synchronized with the existing typed sequencer.
- Added visible sequential-flow links and TRUE branch links, with distinct colors.
- Persisted node positions in program schema 2 and added a version 1 to 2 migration.
- Added numeric variables, assignment, arithmetic expressions, increment and comparison-branch blocks for generic lab automation.
- Added per-block breakpoints and pause-before-execute behavior.
- Added a bounded execution trace and JSON export (`Ctrl+F7`).
- Added runtime variable values to the sequencer and graph inspectors.
- Avoided key collisions by activating sequencer editing shortcuts only while the graph editor is open.
- Included completed waits in the execution trace and added a variable-overflow guard.
- Added validation and runtime tests for variables, breakpoints and node coordinates.
- Kept the graph as an execution-flow editor; arbitrary port wiring and custom expression parsing remain future work.


## 0.5.0 — Laboratory block sequencer
- Added a typed sequence model with Start, Wait, temperature read, temperature condition, heater output, Record and End blocks.
- Added stable block identifiers and branch targets that survive block reordering.
- Added structural validation, protected Start/End blocks, protected branch targets and a 128-block editor limit.
- Added deterministic single-step and real-time run/pause execution.
- Added a generic lumped thermal-chamber demonstrator with closed-form time integration.
- Added bounded instruction execution to protect against runaway branch loops.
- Added JSON save/load for laboratory programs.
- Added a sequence HUD and keyboard editor controls, including condition comparison and jump-target editing.
- Added a serialized demo program and JSON round-trip test.
- Added tests for sequence validation, branch references, editing constraints, step execution, pause/resume and completion.
- Kept the sequencer scoped to generic laboratory automation; no missile-specific propulsion, guidance or flight-control logic is implemented.

## 0.4.0
- Added schema 2 assembly connections with schema 1 migration.
- Added directed parent-child graph validation: existing endpoints, unique connection IDs, one parent per child and cycle rejection.
- Added keyboard creation/removal of metadata-only assembly links.
- Added 64-snapshot in-memory undo/redo.
- Added 1 cm fine movement and dimension adjustment, plus 1-degree rotation mode.
- Added tests for schema migration, connection graph validation and undo/redo.
- Documented graph semantics and current limitations.

## 0.3.0
- Added SI quantity wrappers and simulation provenance metadata.
- Added configurable Rayon worker count and cooperative job cancellation.
- Added local-space picking for rotated components and independent axis resizing.

## 0.2.0
- Added interactive generic component editing and JSON save/load.

## 0.1.0
- Initial Bevy scene, assembly model and background thermal demonstration.


## 0.19.0

- Added local-axis translation arrows with conical heads and rotation rings to the Workshop viewport.
- Added mouse picking, hover highlighting and drag transforms with ray/plane projection.
- Added distance-aware gizmo sizing so manipulators remain readable across camera zoom levels.
- Added clickable Translate/Rotate mode controls with active-mode highlighting and `T` / `R` shortcuts.
- Captured the pointer during a gizmo drag and paused camera navigation until release.
- Added absolute-position grid snapping for gizmo translation.
- Made each mouse drag a single undoable transaction; no-op clicks do not create history entries.
- Kept the scientific simulation crate unchanged.


## 0.20.0

- Added a right-side numeric inspector in the Workshop view.
- Added keyboard entry for absolute position, rotation and dimension values.
- Added per-field decrement/increment controls with sensible SI-sized steps.
- Position/dimension inputs use metres; rotation inputs use degrees.
- Added finite-value checks, bounds, angle normalization and one-step undo support for each committed edit.
- Numeric entry bypasses grid snapping so typed values remain exact.
- Camera and editor shortcuts are suspended while a numeric field is active.
- Added tests for absolute numeric edits, undo, angle normalization and dimension bounds.


## 0.21.0 - Five-facility campus separation

- Expanded the campus from three interactive facilities to five: contracts, construction, research, programming and computation.
- Added dedicated navigation destinations and F1-F5 shortcuts for all facilities.
- Moved the visual sequence editor and runtime ownership to the Programming Studio.
- Kept scientific background jobs and the research-job HUD in the Computation Centre.
- Added the Research Centre department overview as the 0.22 research-system entry point.
- Updated architecture and campus documentation to state what is and is not separated in this release.
