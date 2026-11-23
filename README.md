# Omega — Research Island

[![CI](https://github.com/Sts-simon/Omega/actions/workflows/ci.yml/badge.svg)](https://github.com/Sts-simon/Omega/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-0.26.0-blue.svg)](https://github.com/Sts-simon/Omega/releases/tag/v0.26.0)
[![Licence: MIT](https://img.shields.io/badge/licence-MIT-green.svg)](LICENSE)

**Version 0.26.0** — a Rust + Bevy engineering sandbox shaped like a fictional island research
campus. You build a staged missile in a 3D workshop, unlock parts through a research tree that
advances with a day clock, and queue scientific studies that run on background worker threads.

The older civil contract loop (civilian instrument assembly, instrument profile, dossier, field
test) is still present and is currently the only complete contract path.

Everything in this project is fictional and abstract. Every mass, thrust, cost, duration and
probability is a gameplay value, not physical or real-world performance data. The project
deliberately models no real vehicle, no guidance or tracking law, no seeker logic and no warhead:
a target lock is a single seeded probability roll. See `docs/VISION_MISSILES.md`.

## Direction

The finished game rests on **missile design alone**: contract → staged vehicle → program built from
installed hardware → seeded trials → delivery of a plan (design, program and evidence; nothing is
produced).

Today only the design and research halves are playable. The legacy assembly bench, the polygon
sketch, the instrument profile, the chamber sequencer and the civil contract families are
transitional tooling, kept while they are the only way to complete a contract, and removed once
missile-family contracts, trials and plan delivery replace them.

The project is published as the public repository <https://github.com/Sts-simon/Omega> on the
`main` branch, under the MIT licence. `target/`, `saves/`, `results/` and the local `.freebuff/`
workspace folder are ignored.

## What works in 0.26.0

### Research Centre (playable)

- 13 technologies in five departments (missile families, propulsion and structures, sensors and
  electronics, onboard computing, design and test tools), each with a credit cost, a duration in
  days and prerequisites.
- One research project at a time. Credits are deducted when research starts; the **NEXT DAY**
  button advances the game clock, shown as `DAY n` in the navigation bar. Finishing a project
  grants its capabilities and prints the technology name.
- Research state and the day counter are saved in the campaign (older campaigns load with the
  defaults).
- Parts in the missile workshop are gated by capabilities: locked parts are greyed out and the
  message names the technology to research, not an internal id.

### Missile workshop (playable, mouse-driven)

- A staged vehicle built from 13 part types: solid motor, fuel tank, liquid engine, turbofan,
  pitot tube, barometer, basic radar, active radar, three flight computers, decoupler and mission
  module. Each part is resizable within validated ranges.
- One to four stages. New parts go to the selected stage. The single flight computer always sits in
  the last stage and moves there automatically when a stage is added or removed.
- Presets: two-stage rocket, cruise vehicle, or an empty vehicle. Presets whose parts are not yet
  researched are refused with the missing technologies.
- Selected part resizing with `-` / `+`: length and diameter (motors, tanks), burn time (solid
  motors), engine scale (liquid and air-breathing engines), module mass. All edits are clamped and
  validated.
- Live figures per stage: wet mass, mass carried at ignition, thrust, burn time, thrust-to-weight
  ratio (red below 1 and amber below 1.3 for the first stage) and delta-v; total mass; computer
  class with its thread and memory limits; the programming blocks the installed sensors unlock;
  research still missing; and design validation messages.
- The 3D stack is rebuilt on change, with a highlighted selected part and a nose cone; the camera
  centres on the vehicle when the workshop opens.
- The vehicle is saved to `designs/vehicle.json` on every change and reloaded at startup. Saved
  parts are validated on load; invalid parts are rejected.

### Missile program editor (playable, 0.27 work)

- The **PROGRAM** tab (`F4`) composes the onboard program as parallel threads of the seven blocks of
  `omega-rules::program`. The palette enables only what the fitted hardware unlocks: pitot tube →
  hold speed, barometer → hold pressure, radar → lock target, and a second stage → separate.
- Live meters compare threads and blocks with the flight computer class (1/8, 4/16, 8/32). Every edit
  is validated immediately and each issue names the thread and block at fault, or the missing
  hardware.
- Blocks are appended to the selected thread, selected, moved up or down, tuned with `-` / `+`
  (wait, stage number, throttle, speed, pressure) and removed. `Lock target` has no value and must
  close its thread.
- The program is saved to `programs/missile.json` on every change, reloaded at startup, and shown
  with a content hash ready to bind trial evidence. Blocks are refused when the computer memory or
  the parallel-thread limit is reached.
- The 0.6 chamber sequencer is now the **SEQUENCER** tab, explicitly labelled legacy.

### Trial screen (playable, 0.28 work)

- The **TRIALS** tab flies the saved vehicle with its onboard program and shows four bar plots
  (speed, altitude, mass, thrust), the event timeline (ignition, lift-off, pitch-over, separation,
  burnout, stall) and every anomaly in plain language, such as "stage 1 dropped with 280 kg of
  propellant unburnt".
- The seed is editable (`-10 / -1 / +1 / +10`) and the mission profile switches between a vertical
  launch and an air launch at 200 m/s. The input hash of vehicle, program, profile and seed is shown
  next to the report, so a design edited after a run is visibly stale.
- The seed is the target-lock roll and nothing else (the flight itself is deterministic), so the
  setup panel shows what the current seed does: `ROLL 0.11` against the radar's 55 % reads `LOCK`
  and flips to `NO LOCK` as the seed changes, and a warning says so when no radar or no
  `LOCK TARGET` block is fitted.
- Monte Carlo batches of 10 / 100 / 1,000 seeds summarize the outcome distribution (runs without
  lift-off, runs without separation, lock acquisition, mean speed, path and time) and stay locked
  until the "Batch trials" research node grants `analysis:monte_carlo`. A batch flies on a worker
  thread and shows its progress, so a thousand seeds never block the frame, and each report is
  folded into the running totals instead of being kept, so the memory footprint stays flat.
- A/B comparison stores snapshot A, flies the edited design over the same ten seeds and prints the
  deltas for path, maximum speed and lock rate.
- The session (seed, reports, batch, snapshot) is in memory only; the vehicle and program it flies
  are the saved ones.

### Legacy civil loop (playable, transitional)

- Contract Office: five randomized civilian/scientific domains, one offer each, with unit
  quantities, mass/power/temperature/uptime/data/storage limits, budgets, rewards and deadlines.
  Up to two active projects.
- Full phase path: accept → design → instrument profile → submit (design checks, geometry-derived
  centre-of-gravity balance limit) → background thermal analysis → deterministic 1,000-sample field
  test → hash-bound evidence → dossier-gated delivery with production cost and payout. A design or
  profile change makes the stored snapshot and evidence stale.
- Legacy assembly bench: 3D primitive assembly editor (panel, beam, enclosure, platform, cylinder),
  mouse picking, translate/rotate gizmos, numeric inspector, movement grid, parent-child metadata
  links, in-memory undo/redo, JSON persistence, design revisions, polygon sketch with vertex
  editing, and the instrument-profile editor.
- Legacy sequencer: the 0.6 visual sequence editor (chamber demonstrator) is kept as the
  SEQUENCER tab. Computation Centre: thermal finite-volume study, civil truss study, mesh thermal
  studies from the assembly or the polygon sketch (optionally locally refined), field test, HTML/SVG
  report opening and cancellation.

### Scientific core

- `omega-sim-core`, Bevy-free: transient 2D finite-volume heat conduction; 2D linear-elastic
  pin-jointed truss; triangular meshing of coplanar rectangles and concave polygon regions with
  holes, conforming local refinement and quality metrics; transient triangular-finite-element heat
  conduction on those meshes; HTML/SVG + CSV report export.
- `omega-data`, Bevy-free: content hashing, atomic writes and the versioned save slot.
- `omega-rules`, Bevy-free: missile vehicle, program, flight test, research, requirements,
  evidence, dossier, metrics and design diff.

## Not implemented in 0.26.0

- **Programs are not part of a contract yet.** The program editor exists, but no contract, trial or
  dossier consumes a program: the missile program is not part of the evidence and there is still no
  plan-delivery path.
- **No trial screen.** `omega-rules::flight` (seeded integration with events, anomalies and a lock
  roll) is headless only: no graphs, anomaly display, seeds, batch trials or design comparison. The
  `analysis:monte_carlo` grant exists but nothing consumes it.
- **Contracts are still civil.** They do not use the missile vehicle, missile designs are not part
  of design revisions or verification evidence, and there is no plan-delivery path for missile
  contracts.
- **Research does not gate contracts.** The `family:*` capabilities and the `ExtraActiveProjects`
  grant are not checked anywhere; the active-project limit stays at two.
- The legacy civil loop and the legacy bench are still required to finish a contract.
- No scoring, reputation, live deadlines, production scheduling, staffing, inventory or island
  expansion.
- No arbitrary 3D CAD meshing, bending/buckling/fatigue/dynamic structural mechanics, implicit time
  integration or certified engineering validation. Connection kinds remain graph metadata only.

## Requirements (Linux Mint)

Install Rust stable with rustup, then the Bevy Linux development packages:

```bash
sudo apt update
sudo apt install build-essential pkg-config libx11-dev libasound2-dev libudev-dev \
  libxkbcommon-x11-0 libwayland-dev libxkbcommon-dev mesa-vulkan-drivers
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup update stable
```

`install-mint-deps.sh` contains the package command. The project pins Bevy `=0.20.1` in
`Cargo.toml` and selects the current stable toolchain through `rust-toolchain.toml` (with rustfmt
and clippy). Cargo 1.100 / Rust 1.100 is known to build the workspace.

## Build, run and verify

```bash
cargo run                       # opens the Bevy window (needs a display and Vulkan)
cargo check --workspace --all-targets
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

The Cargo package and its binary are both named `omega`; the workspace crates are `omega-sim-core`,
`omega-data` and `omega-rules`.

The first build compiles Bevy and its dependencies and takes several minutes; dependency crates are
built with `opt-level = 3` even in the dev profile. `saves/` and `results/` are created on demand and
ignored by git, as is `target/`.

Scientific studies run on a dedicated worker thread with a Rayon pool. Worker threads default to
detected logical CPUs minus one, capped at 16; override with `OMEGA_WORKER_THREADS`, for example
`OMEGA_WORKER_THREADS=16 cargo run --release`.

## Interface and controls

The top bar switches views: **ISLAND**, **CONTRACTS**, **WORKSHOP** (missile workshop), **RESEARCH**,
**PROGRAM** (missile program), **COMPUTE**, **SEQUENCER** (legacy 0.6 editor), **LEGACY BENCH**. The
right side of the bar shows the message lane and
`DAY n | credits | active projects | completed`. The island shows five facility cards; clicking a
card opens it.

| Input | Action |
|---|---|
| `F1` | Contract Office |
| `F2` | Missile workshop |
| `F3` | Research Centre |
| `F4` | Missile program editor |
| `F5` | Computation Centre |
| `Escape` | Return to the island (closes the profile editor or an open editor first) |

The legacy assembly bench and the legacy sequencer have no keyboard shortcut: use the navigation bar
(`LEGACY BENCH`, `SEQUENCER`) or the active-project card.

### Missile workshop

Editing is mouse-only: presets, part palette (locked parts greyed), stage buttons and `+ STAGE` /
`- STAGE`, the part list per stage, `-` / `+` on the selected part's adjustable values, and
`REMOVE PART`. Stage figures and validation update immediately.

### Research Centre

`START RESEARCH` on an available technology, `NEXT DAY` to advance the clock. The lab runs one
project at a time.

### Contract Office

| Input | Action |
|---|---|
| `1`–`5` | Accept the corresponding offer |
| `Tab` / `Shift+Tab` | Select the next / previous active project |
| `V` | Deliver a project that passed its field test and dossier |
| `N` | Refresh the offer board |

### Legacy assembly bench

| Input | Action |
|---|---|
| `T` / `R` | Translate / rotate gizmo mode |
| `P` / `Shift+P` | Connect the selected part to the nearest eligible part / remove its links |
| `Ctrl+Tab`, `Ctrl+Shift+Tab` | Switch the selected active project |
| `Ctrl+Enter` | Evaluate and submit the design |
| `F11` | Instrument-profile editor |
| `Ctrl+S` / `Ctrl+L` | Save / load the assembly |
| `H` | Toggle the shortcut help overlay |

The left tool shelf mirrors add, select, move, rotate, resize, link, history, save/load, revisions,
sketch and profile actions. Polygon sketch editing, vertex dragging, the numeric inspector and the
sequence editor have their own key tables in `docs/UX_CONTROLS.md`, `docs/CAMPUS.md` and
`docs/SEQUENCE_EDITOR.md`; those documents were written before 0.26 and describe the legacy tools.

### Programming Studio

`F6` opens or closes the visual sequence graph. Its full key table is in
`docs/SEQUENCE_EDITOR.md`.

### Computation Centre

| Input | Action |
|---|---|
| `R` | Queue thermal screening for the selected contract (generic thermal study if none) |
| `T` | Run the contract field test after a successful analysis |
| `M` | Mesh supported workshop plates/panels and queue a thermal study |
| `Shift+M` | Refine a central region of that mesh, then queue the study |
| `G` | Queue the civil truss reference study |
| `P` | Mesh the persisted polygon sketch and queue a thermal study |
| `O` | Open the latest HTML report |
| `C` | Cancel the newest active job |

The shelf provides the same actions as buttons (mesh assembly, mesh sketch, analyse project, field
test, open report, cancel).

### Camera

Right-drag orbits, middle-drag pans, wheel or touchpad scroll zooms; WASD/arrows pan and `Q`/`E`
change height. The camera is disabled while an editor overlay is open or the pointer is over an
interactive panel.

## Saves and reports

All persistent files go through `src/save.rs` and `omega_data::SaveSlot`:

```text
saves/slot_1/
  manifest.json                 committed last; read verifies every file hash
  campaign.json                 credits, offers, projects, evidence, research, day, PRNG
  designs/assembly.json         legacy assembly
  designs/polygon-sketch.json   legacy polygon sketch
  designs/revisions.json        legacy design revisions
  designs/vehicle.json          missile vehicle (0.26)
  programs/missile.json         missile program (0.27)
  programs/lab_sequence.json    chamber sequence program (legacy)
```

Writes are atomic (temporary file, fsync, rename) and a damaged file is reported instead of loaded,
with the previous generation kept as `<name>.bak`. Files from the pre-0.22 layout (`saves/*.json`)
are imported once without being modified. Field-test and solver reports, meshes and HTML/CSV
exports are written to `results/`.

## Project structure

```text
src/
  main.rs              Bevy app, orbit camera, ordered system chain
  campus.rs            views, campaign, civil contracts, phases, research, day, save
  campus/board.rs      card boards: island, contracts, research, missile workshop, project
  campus/verification.rs  hash-bound evidence and dossier
  missile.rs           missile workshop session, parts, stages, 3D stack
  program_editor.rs    missile program session, validation, persistence
  interface.rs         navigation bar, tool shelves, UiAction dispatch
  save.rs              SaveSlot access and 0.21 import
  ui_kit.rs            UI primitives (cards, chips, buttons, palette)
  scene.rs             procedural low-poly island
  design/              legacy assembly model/editor, inspector, gizmos, polygon sketch,
                       meshing adapter, metrics, revisions, storage
  jobs/                worker thread, queue, cancellation, HUD, report writing
  sequence/            chamber sequencer: model, runtime, graph, expressions
crates/
  sim-core/            Bevy-free solvers: thermal FV, truss, mesh, mesh thermal, reports
  omega-data/        content hashes, atomic writes, versioned save slot
  omega-rules/       vehicle, program, flight, research, requirements, dossier, metrics, diff
docs/                  architecture, vision, roadmap, changelog, model notes, legacy UX docs
examples/              sample assembly, chamber program, polygon sketch
```

## Scientific models and limitations

The thermal finite-volume solver (`sim-core::thermal`) uses SI dimensions, per-cell materials,
harmonic face conductivity, insulated or fixed-temperature boundaries, a volumetric source and an
explicit stability bound; it reports energy balance and a deterministic input fingerprint. The
triangular mesh solver (`mesh_thermal`) adds per-element thickness on conforming planar meshes. The
truss solver (`mechanics`) covers small-displacement linear-elastic pin-jointed 2D structures only.

None of these models is validated against experimental data or certified for engineering use, and
none is coupled to the missile vehicle in 0.26. The mesh path accepts only coplanar plates and drawn
planar polygon regions, not arbitrary solid CAD. Solver jobs report progress and support cooperative
cancellation, and every result carries model/solver metadata and its input configuration.

The missile figures (mass, thrust, burn time, TWR, delta-v) come from abstract part coefficients in
`omega-rules::vehicle`. The flight test integrates speed along a path with thrust, drag and, for a
vertical launch, gravity; altitude only changes air density. There is no steering, trajectory
optimisation, tracking or guidance law, and the lock is one seeded probability roll.

## Tests and quality

The workspace contains unit tests in all four crates (application logic in `src/`, plus
`sim-core`, `omega-data` and `omega-rules`), including headless solver checks, save-slot
integrity, research/vehicle/program/flight rules, sketch validation and revision handling. Run them
with `cargo test --workspace`; `cargo test -p omega-sim-core` runs the numerical crate alone.

Every change is checked by the GitHub Actions workflow in `.github/workflows/ci.yml`: formatting,
`clippy -D warnings` and the full test suite.

## Documentation status

`docs/CHANGELOG.md` is the authoritative release history. `docs/VISION_MISSILES.md` and
`docs/ARCHITECTURE.md` describe the 0.26 state, and `docs/ROADMAP_OMEGA.md` is the improvement plan
that carries the project to a missile-only loop. `docs/CAMPUS.md`, `docs/ROADMAP.md`,
`docs/UX_CONTROLS.md` and `docs/SEQUENCE_EDITOR.md` were written before 0.26 and now live in
`docs/archive/`: they are kept for historical reference only and no longer describe the project.
