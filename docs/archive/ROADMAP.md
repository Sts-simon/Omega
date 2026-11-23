> **Archived document.** Written before the 0.26 missile-only direction and kept for historical
> reference only. It no longer describes the current project: see `README.md`,
> `docs/ARCHITECTURE.md` and `docs/ROADMAP_OMEGA.md`.

## Completed in 0.18.0

- [x] Persistent mouse-clickable navigation between island and facilities.
- [x] Contextual scrollable tool shelves for contracts, workshop and laboratory.
- [x] Mouse-driven workshop transforms: translation, rotation and per-axis dimension changes.
- [x] Right-drag orbit, middle-drag pan and normalized wheel/touchpad zoom.
- [x] Prevent world picking/camera controls from receiving clicks over UI overlays.
- [x] Separate research status panel from the general campus status panel.

## Completed in 0.17.2
- [x] Fix polygon sketch ring helper calls and optional vertex extraction.
- [x] Resolve the Bevy `ResMut` borrow in hole mode.
- [x] Split the long ordered Update schedule into smaller chained groups.
- [x] Remove unused exports reported by `cargo test --workspace`.

## Completed in 0.17.0

- Polygon vertex handles and drag editing for outer rings and holes.
- Grid and vertex snapping, with modifier bypass and a global toggle.
- Validation and rollback on invalid edits; guarded vertex deletion.
- Existing polygon sketch save schema preserved.

## Completed in 0.16.0
- [x] Interactive planar polygon sketch mode in the Workshop.
- [x] Draw and validate concave outer contours and interior holes.
- [x] Multiple material regions with per-region material and thickness metadata.
- [x] Persist polygon sketches independently of the primitive assembly.
- [x] Separate new-region material/thickness defaults from edits to the selected region.
- [x] Bound total sketch vertices and make save mutations transactional.
- [x] Weld common complete edges and queue a combined multi-region thermal mesh study from the Computation Lab.

## Completed in 0.15.0
- Added an independent 2D polygon-region mesher for concave outer contours and interior holes.
- Added contour validation, winding normalization, hole containment checks and area-consistency verification.
- Connected the resulting mesh directly to the existing triangular thermal solver through a headless JSON study example.

## Completed in 0.14.0
- Added conforming longest-edge bisection for selected mesh elements.
- Added centroid-selected regional mesh refinement with target edge length and bounded rounds.
- Added geometric quality diagnostics and area/volume preservation checks.
- Added `Shift+M` to queue a locally refined workshop thermal study.

## Completed in 0.10.0
- Precomputed cell heat capacities and internal/boundary conductances; added a configurable JSON input deck for the standalone solver example.
- Added a guard on total cell updates to bound accidental runaway study sizes.

- [x] Extract the numerical engine into an independent Rust crate with no Bevy dependency.
- [x] Replace the unitless diffusion stencil with SI finite-volume solid heat conduction.
- [x] Support heterogeneous per-cell material maps and harmonic interface conductivity.
- [x] Support insulated and fixed-temperature boundary conditions and uniform volumetric source terms.
- [x] Derive and enforce a safe explicit time-step limit from each cell's conductance and heat capacity.
- [x] Persist the final field and whole-domain energy diagnostics.
- [x] Add headless tests for energy conservation, uniform fields, heterogeneous materials, cancellation, stability and thread-count reproducibility.

## Completed in 0.11.0
- Added an independent 2D pin-jointed civil truss solver with sparse stiffness assembly and preconditioned conjugate gradients.
- Added displacement, support reaction, axial force/stress and energy/residual diagnostics.
- Added analytical and thread-count reproducibility tests plus a civil bridge JSON input deck.

## Completed in 0.9.0

- Randomized scientific/civil-aerospace contract board with five distinct non-weaponized domains, generated requirements, clients, budgets, rewards and deadlines.
- Refreshable offers and acceptance of up to two active contracts.

# Roadmap

## Milestone 0 — foundation (complete)
- [x] Rust + Bevy app skeleton
- [x] 3D placeholder research island
- [x] Generic part / assembly data model
- [x] JSON persistence
- [x] Background job queue
- [x] CPU-parallel thermal demonstration
- [x] Persisted result report

## Milestone 1 — usable assembly editor (prototype complete)
- [x] Select components with mouse picking
- [x] Cycle selection with keyboard
- [x] Add and remove generic primitive parts
- [x] Move, rotate, resize and change material
- [x] Procedural primitive mesh preview
- [x] Stable component IDs and duplicate-ID validation
- [x] Show dimensions, position, material and estimated mass
- [x] Save and load the assembly
- [x] Independent X/Y/Z dimension adjustment
- [x] Optional 0.25 m movement snapping
- [x] Fine 1 cm movement / dimension edits and 1° rotation
- [x] Parent-child assembly graph metadata
- [x] Graph validation and schema 1 -> 2 migration
- [x] In-memory undo / redo (64 snapshots)
- [x] Local-axis transform gizmos with mouse dragging
- [ ] On-screen numeric transform/property inspector
- [ ] Connector-face definitions, automatic snapping and geometric constraints
- [ ] Multi-selection and persistent edit history

Acceptance criteria: a user can create, validate, save, close and reopen a small mechanical test assembly without losing its parameters. The current prototype covers basic primitives and logical graph links; geometric constraints remain open.

## Milestone 2 — research job system
- [x] Worker thread separate from Bevy update loop
- [x] Rayon CPU pool
- [x] Job progress and completion reporting
- [x] JSON result reports
- [x] Job list UI with queued/running/completed/failed/cancelled states
- [x] Environment-configurable worker thread budget (`OMEGA_WORKER_THREADS`)
- [x] Cooperative cancellation between solver iterations
- [ ] Cooperative pause
- [ ] Job priorities
- [ ] Scheduled jobs and persistent queue
- [ ] In-game worker thread budget control
- [ ] CPU and memory usage metrics
- [ ] Crash-safe checkpoints for long tasks

Acceptance criteria: a long-running job never freezes the viewport; cancellation is reflected in the UI; completed results are written to JSON reports. Persistent queues and crash recovery remain future work.

## Milestone 3 — scientific data foundation
- [x] Dedicated `sim-core` library crate independent of Bevy
- [x] Explicit non-negative SI quantity types
- [x] Model / solver metadata and deterministic input fingerprint
- [x] Thermal input validation
- [x] Thermal report stores exact input configuration and metadata
- [ ] Result schema and backward-compatible migrations
- [x] Analytical invariants and solver-output reproducibility tests (reference datasets remain future work)
- [ ] Convergence and sensitivity test harness

Acceptance criteria: every saved result can be reproduced from its recorded input and solver metadata, within a documented tolerance.

## Milestone 4 — generic material and structure studies
- [x] Stationary 2D transient heat-conduction example with an explicit stability bound and energy accounting
- [x] Generic linear-elastic 2D pin-jointed civil truss example
- [ ] 2D continuum/coupon model with documented element formulation
- [x] Local mesh refinement and geometric quality metrics
- [ ] Mesh resolution / convergence visualization
- [x] Browser-based thermal field and civil truss result reports (SVG + CSV)
- [ ] In-game interactive plot viewport and contour/time-history controls
- [ ] Error estimates and domain-of-validity warnings
- [ ] Comparison between approximate and higher-fidelity studies

Acceptance criteria: each solver has documented assumptions, known limitations and automated reference tests.

## Milestone 5 — visual logic editor (first sequencer prototype complete)
- [x] Typed block definitions (Start, Wait, temperature read, condition, heater output, Record, End)
- [x] Stable block IDs and conditional jump targets
- [x] Program validation and bounded execution budget
- [x] Deterministic single-step and run/pause playback
- [x] Generic thermal-chamber demonstration
- [x] JSON save/load
- [x] Draggable node canvas with sequential and conditional flow links
- [x] Numeric variables, breakpoints and execution trace export
- [ ] Arbitrary typed data ports and port-to-port wiring
- [ ] More generic sensor and actuator types
- [ ] Event triggers and reusable subroutines
- [x] Breakpoints, watch values and execution trace export

Acceptance criteria: a small non-flight industrial test process can be sequenced, validated, saved and replayed deterministically. The current version meets this for a keyboard-driven list and a draggable visual graph. Arbitrary port wiring remains future work.

## Milestone 6 — island management and contracts (prototype loop complete)
- [x] Clickable Contract Office, Assembly Workshop and Computation Lab buildings
- [x] Facility navigation and context-specific interfaces
- [x] Connect Workshop to the existing assembly editor and visual sequencer
- [x] Connect Computation Lab to the background research queue
- [x] Queue thermal and civil truss demonstration studies and open generated HTML reports
- [x] Randomized civilian instrument contracts with five domains and varied constraints
- [x] Duplicate acceptance protection and two-active-contract limit
- [x] Persistent campaign state and interrupted-analysis recovery
- [x] Contract-specific design checks for assembly mass and instrument-profile constraints
- [x] Contract-linked background analysis and 1,000-sample field-test stage
- [x] Delivery, production cost, client payout and completed-contract history
- [ ] Reputation, live deadlines and client relationship changes
- [ ] Place new buildings on a grid or freeform terrain
- [ ] Workshop capacity, inventory and production scheduling
- [ ] Research equipment upgrades and island expansion

Acceptance criteria: a player can accept a civilian instrument contract, submit a design and profile, complete the prototype analysis and field-test steps, pay production cost, receive payment and resume the campaign after restarting. Version 0.9.0 implements this as a first gameplay abstraction; engineering-grade product solvers and time-based production remain future work.

## Milestone 7 — polish
- [ ] Better low-poly assets and lighting
- [ ] Tutorials and in-game documentation
- [ ] Profiling and memory budgets
- [ ] Save migration tests
- [ ] Linux packaging and release builds


## 0.13.0 completed: planar workshop meshing

- Triangulate supported horizontal rectangular panels/platforms.
- Require conforming complete-edge contacts; reject point-only and partial-edge contacts.
- Preserve material, thickness and originating part ID on each triangle.
- Run unstructured 2D thermal FEM from the Computation Lab with `M`.
- Export a mesh-aligned heat map and complete CSV field.
- Queue geometry-derived studies from the Computation Lab and support a headless JSON study example.

Next scientific candidates: multi-region polygon interfaces, a convergence study for polygon meshes, temperature-dependent material data and an implicit time integrator. The next geometry milestone should let the Workshop store and edit polygon contours, then build conforming interfaces between adjacent material regions. A full 3D CAD mesher should wait until there is a validated need and a clear geometric kernel strategy.


## 0.19 UX iteration

The Workshop now has local-axis 3D translation arrows and rotation rings with hover feedback, camera-distance-aware sizing, pointer capture and one-step undo transactions. Follow-up CAD UX tasks remain: numeric transform inspector, multi-selection, edge/face snapping, constraint dimensions and undo/redo for polygon-sketch edits.


## 0.20 UX iteration — numeric inspector

Implemented a right-side component inspector for absolute numeric editing of position, rotation and dimensions. Values are entered in SI metres and degrees, committed with Enter and cancelled with Escape. Per-field step buttons provide quick adjustments. Multi-selection and scale gizmos remain separate future iterations to keep this interaction independently testable.


## Completed in 0.21.0 — five-facility campus separation
- [x] Split the campus into Contract, Construction, Research, Programming and Computation views.
- [x] Add five clickable building hitboxes, navigation destinations and F1-F5 shortcuts.
- [x] Move the visual sequence editor and runtime to the Programming Studio.
- [x] Keep scientific jobs and reports in the Computation Centre.
- [x] Add a Research Centre department overview for the upcoming technology progression system.
- [ ] Separate interior 3D scenes and facility-specific camera staging.
- [ ] Add persistent technology trees, prerequisites, research costs and unlock effects (0.22.0).


## Missile loop (see `docs/VISION_MISSILES.md`)
- [x] Core logic for parts, stages, programs, flight test and research in `omega-rules` (tested, not visible in the game).
- [x] 0.25 research board and clock; 0.26 missile workshop (parts, stages, resize, figures, locked parts).
- [ ] 0.27 programming v2; 0.28 trials UI; 0.29 missile-family contracts and plan delivery; 0.30 balancing and tutorial.

## Plan 0.22 – 0.30 (earlier, civil-contract status)

- [x] 0.22 (partial) `omega-data` and `omega-rules` crates with tests; geometry-derived balance requirement in contracts.
- [x] 0.22 `SaveSlot` used for campaign, assembly, sketch and program; 0.21 saves imported on first access.
- [ ] 0.22 (remaining) Split `campus.rs`; Bevy `States`/plugins per facility; slot selection.
- [x] 0.23 (partial) Evidence stored per contract, verification matrix in the HUD, delivery gated by the dossier.
- [ ] 0.23 (remaining) Replace `InstrumentProfile` with derived metrics; dedicated matrix panel; hash programs into the evidence.
- [x] 0.24 (partial) Design revisions, diff against the latest revision, live mass/balance budget.
- [ ] 0.24 (remaining) Vehicle modules and axial stacking by family; side-by-side comparison of two revisions.
- [ ] UI debt: overlapping workshop panels, Contract Office text taller than the window.
- [x] 0.25 (logic only) Research tree rules, tests and a starting tree in `omega-rules`.
- [ ] 0.25 (remaining) Game clock ("next day"), research board, campaign persistence, capability gating in the workshop, contracts and computation centre.
- [ ] 0.26 Programming v2: signal/action registry, programs bound to design revisions.
- [ ] 0.27 – 0.28 Trials: abstract scenarios, seeded runs, anomaly detection, comparison, analyses as verification methods.
- [ ] 0.29 Complete dossier, scoring, reputation, deadlines, platform heritage.
- [ ] 0.30 Balancing, tutorial, performance and save stress tests.
