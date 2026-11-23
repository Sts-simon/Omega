# Architecture

## Main rule

The renderer must never wait for a long scientific computation. The Bevy app owns interaction and visualization; independent workers own long-running jobs; workers return immutable result records through channels.

## Logical modules

### Campus and facilities (`campus.rs`, `campus/board.rs`, `campus/verification.rs`)
- Owns the current view (`CampusView`: island, contracts, construction, missile, research,
  programming, computation), tags the five interactive building meshes and performs local-space
  ray/AABB picking.
- Routes the player into each facility and back to the island.
- Hosts the randomized five-domain civil contract board, the two-active-project limit, the campaign
  economy, the research state and day clock, and campaign persistence.
- `campus/board.rs` builds the card boards (island facilities, Contract Office, dossier matrix,
  Research Centre, missile workshop palette and stage figures, workshop project card) and only
  rebuilds a board when its content signature changes.
- `campus/verification.rs` turns contract requirements and results into hash-bound evidence and a
  delivery dossier; evidence becomes stale when the design hash changes.
- The `interface` module owns shared navigation, contextual tool shelves, mouse actions and the
  message lane. Accepted civil projects progress through design checks, generic material screening,
  a field-test gameplay stage and dossier-gated delivery/payout.

### Missile workshop (`missile.rs`)
- Owns the staged-vehicle session (`aerolab_rules::vehicle::Vehicle`), the selected part, the active
  stage and the part id counter.
- Applies button actions (add part, select, add/remove stage, adjust length/diameter/burn
  time/engine scale/module mass, remove part, presets, clear) with per-part clamps, a four-stage
  limit and the rule that the single flight computer always sits in the last stage.
- Gates part templates by the capabilities owned through research and reports the missing
  technology by label.
- Lays the parts out as a 3D stack (engines first inside each stage, nose cone on top) and rebuilds
  the meshes only when the vehicle or selection changes.
- Persists the vehicle to `designs/vehicle.json` on every change and validates saved parts on load.

### Presentation (`main.rs`, `scene.rs`, `interface.rs`)
- Bevy window, orbit camera, island, persistent top navigation and contextual facility tool shelves.
- `interface.rs` dispatches mouse-clickable actions to the campus, design, sequencer and job modules.
- UI overlays block world picking and camera motion; they do not implement scientific algorithms.

### Design model (`design/model.rs`)
- Serializable project data.
- Generic parametric parts and materials.
- Validation at load/save boundaries.
- Approximate mass estimates from primitive volume assumptions.
- Directed parent-child connection graph with cycle, endpoint and single-parent validation.
- Schema 1 to schema 2 migration for older project files.
- No dependency on Bevy, so the model can be tested headlessly.

### Design editor (`design/editor.rs`, `design/storage.rs`)
- Selection, local-space mouse picking, rotation, axis-specific resizing and keyboard editing.
- Converts the serializable design model into Bevy preview meshes.
- Owns the current edit-session revision, selected component ID and bounded snapshot history.
- Provides keyboard undo/redo and metadata-only parent-child link editing.
- Persists assemblies as JSON and migrates schema 1 files on load.

### Scientific core (`crates/sim-core/`)
- Independent Rust library crate with no Bevy dependency.
- Provides validated SI quantity wrappers and versioned provenance metadata.
- Implements transient 2D finite-volume heat conduction for a solid slab (per-cell materials,
  harmonic face conductivity, insulated or fixed-temperature boundaries, volumetric source,
  explicit stability limit, energy balance).
- Implements a 2D linear-elastic pin-jointed truss solver (sparse assembly, Jacobi-preconditioned
  conjugate gradients, nodal displacement/reactions, member force/stress, energy consistency).
- Meshes coplanar rectangles and simple concave polygon regions with holes (Earcut), welds conforming
  shared edges, preserves material/thickness/source-part provenance, supports conforming
  longest-edge refinement and reports mesh-quality metrics.
- Runs transient triangular finite-element heat conduction on those meshes with per-element
  thickness, explicit stability checks, progress and cancellation.
- Exports self-contained HTML/SVG reports and full-resolution CSV fields for thermal and truss
  results.

### Research jobs (`jobs/`)
- A worker thread receives jobs from a queue.
- A Rayon pool performs CPU-parallel calculations.
- Progress, completion, cancellation and failure events are sent back to the main thread.
- Cancellation uses an atomic token checked between numerical iterations.
- A Bevy HUD displays the latest task records.
- Results are persisted separately from the live scene.
- Each thermal result includes its exact configuration and model/solver metadata.

## Data flow

```text
User input
   |
   +----> Design editor -----> Generic design model + assembly graph -----> JSON project file
   |              |                    |
   |              |                    +-----> In-memory undo/redo snapshots
   |              +-----> Bevy preview meshes / HUD
   |
   +----> Job manager / queue
                 |
                 v
              Worker thread
                 |
                 v
              Rayon CPU pool
                 |
                 v
              Result + progress events
                 |
                 +-----> JSON report
                 +-----> Bevy status / future plots
```

## Assembly graph boundary

The current graph stores parent-child relationships and connection kinds as metadata. It validates topology but does not solve connector alignment, rigid-body transforms or joint kinematics. Those capabilities should be introduced later as an independent constraint-solving layer, keeping serialized design data independent from Bevy.

## Future boundaries

The `sim-core` crate is now independent of Bevy. The next separation should be the geometry/mesh pipeline and the result-visualization pipeline. The solver should keep consuming explicit numerical meshes and material maps rather than depending directly on Bevy entities. Bevy should consume immutable result data and downsample fields for display.

A scientific result should carry at least:
- model identifier and version;
- input parameters and units;
- solver configuration;
- convergence / termination status;
- validity domain;
- uncertainty or known limitations;
- software version and timestamp.

The interactive simulation should use precomputed data only when the requested state is inside the recorded validity domain. Extrapolation must be explicit and marked as uncertain, not silently treated as a measured result. The current finite-volume solver is transient and explicit; an implicit scheme, convergence harness and experimental reference data remain future work.


## Sequence module (0.6)

`sequence::model` stores a small typed control-flow program. Each block has a stable numeric ID; branch blocks refer to IDs rather than list indices so reordering does not silently retarget a jump. Validation checks the fixed Start/End boundaries, unique IDs, numeric ranges and branch references. Programs are persisted independently as JSON.

`sequence::runtime` executes blocks deterministically. Single-step mode advances one block, including a complete logical wait. Run mode consumes wait durations against simulation time and processes a bounded number of non-wait blocks per frame. A global instruction budget prevents an infinite branch loop. The included chamber model is a first-order gameplay demonstrator only and is not presented as a validated engineering solver.

`sequence::mod` exposes the typed program editor and status HUD. `sequence::graph` renders the program as draggable Bevy UI nodes with sequential and conditional links. `sequence::expression` evaluates a bounded scalar arithmetic grammar without dynamic code execution. The runtime supports variable watches, breakpoints and trace export. Program edits are rejected during active playback; edits made while paused reset the runtime to avoid executing stale instruction positions.

## Scientific crate boundary (0.11.0)

The root application depends on `aerolab-sim-core` through a local path dependency. The numerical crate owns its material, boundary, grid/time-step validation, solver and result types. It has no Bevy types and can be tested independently with `cargo test -p aerolab-sim-core`. The `src/jobs/thermal.rs` module is now only a compatibility re-export so the job manager can continue to schedule numerical jobs without owning solver implementation.


## Structural solver boundary (0.11.0)

`aerolab-sim-core::mechanics` implements a standalone planar pin-jointed civil truss solver. It receives an explicit node/element/support/load input deck and does not read the Bevy assembly model. This separation is intentional: the current workshop components are coarse visual primitives and do not define a validated finite-element mesh or joint model. The solver writes its own JSON result through the `truss_study` example. The game UI and geometry coupling can be considered only after the numerical model and its reference tests are established.


# Facility routing (0.26.0)

The campus routes to six navigation views plus the island: Contract Office, Research Centre,
Programming Studio, Computation Centre, the missile workshop (`CampusView::Missile`, the WORKSHOP
tab) and the legacy assembly bench (`CampusView::Construction`, the LEGACY BENCH tab). The missile
workshop owns new vehicle design; the legacy bench keeps the 3D primitive assembly editor, the
polygon sketch, the instrument profile and the sequence editor entry point that the civil contract
loop still uses. Programming owns the 0.6 visual sequencer; Computation owns scientific worker jobs
and their reports.

Facility selection is available through the island cards, the building hitboxes, the persistent
navigation bar and F1-F5 (F1 contracts, F2 legacy bench, F3 research, F4 programming, F5
computation; the missile workshop is opened from the WORKSHOP tab). Escape returns to the island,
except when an active modal editor consumes it. All views share campaign state and the same save
slot; there are still no separate serialized Bevy worlds or interior 3D scenes.

Research and development are gameplay layers only: `aerolab_rules` values are abstract and are not
physical performance data.


# Data and rules crates (0.26.0)

Two Bevy-independent crates sit beside `sim-core`:

- `aerolab-data`: content hashing, atomic file writes and the v2 save slot (`manifest.json`
  committed last; per-file hashes verified on read; unsafe relative paths rejected). Legacy 0.21
  files can be imported without modifying the source directory.
- `aerolab-rules`: two halves. The civil verification half provides requirements, evidence, the
  verification matrix, dossier generation and geometry-derived metrics; a requirement is only
  *Verified* when its whole uncertainty band satisfies the limit, and evidence becomes *Stale* when
  the design or program hash changes. The missile half provides `vehicle` (parts, stages, mass,
  thrust, TWR, delta-v, computer limits, sensor-to-block mapping, research requirements), `program`
  (seven blocks, threads, hardware and computer validation), `flight` (seeded abstract flight test
  with events, anomalies and a lock roll), `research` (13 technologies, five departments, grants,
  costs, durations, prerequisites) and `diff` (design comparison).

The application uses:

- `vehicle`, `research` and `PartSpec` capability checks in the missile workshop and the Research
  Centre (through `missile.rs` and `campus/board.rs`); `ResearchState` and the day counter are part
  of `CampaignSave`.
- `metrics`/`requirement` for the geometry-derived balance requirement in `campus::evaluate_design`
  and in verification evidence (`design/metrics.rs`).
- `dossier` through `campus/verification.rs` for dossier-gated delivery.
- `SaveSlot` through `src/save.rs` for all six persistent files (campaign, assembly, polygon sketch,
  program, revisions, vehicle).

`program` and `flight` are implemented and tested but not yet connected to any UI. All metrics and
cost constants are abstract gameplay values, not physical or real-world performance data.

# Target architecture: missile-only loop

The finished game is built on missile design alone. The convergence path is:

```text
Research Centre ---> capabilities ---> Missile Workshop ---> vehicle revision
                                            |                      |
                                            v                      v
                                     Programming Studio      verification evidence
                                            |                      |
                                            v                      v
                                     seeded flight trials --> plan delivery / dossier
```

The legacy assembly bench, instrument profile, polygon sketch, mesh thermal studies, chamber
sequencer and civil contract families are transitional. They stay as long as the civil loop is the
only complete contract path, and are removed once missile-family contracts, trials and plan
delivery replace it. `sim-core`, `aerolab-data` and the verification half of `aerolab-rules` remain
reusable tooling; nothing in the target loop depends on Bevy except the views themselves.
