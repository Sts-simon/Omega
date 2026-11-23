# Omega — Missile Design

[![CI](https://github.com/Sts-simon/Omega/actions/workflows/ci.yml/badge.svg)](https://github.com/Sts-simon/Omega/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-0.30.0-blue.svg)](https://github.com/Sts-simon/Omega/releases/tag/v0.30.0)
[![Licence: MIT](https://img.shields.io/badge/licence-MIT-green.svg)](LICENSE)

**Version 0.30.0** — a fictional missile-design game built with Rust and Bevy. The player accepts
military-themed contracts, assembles an abstract staged vehicle, edits its onboard program, runs
reproducible simulated trials and delivers a dossier. You build a staged missile in a 3D workshop, unlock parts, families and contracts through a
research tree that advances with a day clock, fly deterministic trials, and deliver a plan for
credits.

Everything in this project is fictional and abstract. Every mass, thrust, cost, duration and
probability is a gameplay value, not physical or real-world performance data. The project
deliberately models no real vehicle, no guidance or tracking law, no seeker logic and no warhead:
a target lock is a single seeded probability roll. See `docs/VISION_MISSILES.md`.

## Direction

The game rests on **missile design alone**: contract → staged vehicle → program built from installed
hardware → seeded trials → delivery of a plan (design, program and evidence; nothing is produced).

The inherited civil tooling — assembly bench, polygon sketch, instrument profile, chamber sequencer,
thermal and structural studies, civil contracts — has been removed. `omega-sim-core` stays in the
workspace as a standalone, Bevy-free scientific library, but the game no longer depends on it.

The project is published as the public repository <https://github.com/Sts-simon/Omega> on the
`main` branch, under the MIT licence. `target/`, `saves/`, `results/` and the local `.freebuff/`
workspace folder are ignored.

## What works

### Research Centre

- 13 technologies in five departments (missile families, propulsion and structures, sensors and
  electronics, onboard computing, design and test tools), each with a credit cost, a duration in
  days and prerequisites.
- One research project at a time. Credits are deducted when research starts; the **NEXT DAY**
  button advances the game clock, shown as `DAY n` in the navigation bar. Finishing a project
  grants its capabilities and prints the technology name.
- **Research gates the contract office.** A mission family only appears on the offer board once its
  technology is researched: air-to-surface and surface-to-surface work needs the starting ballistic
  family, surface-to-air and air-to-air need their own family, a cruiser needs the cruise family,
  and difficulty 4–5 surface-to-surface offers need the anti-ship family. The board lists what is
  still missing, by technology name.
- **`Parallel project management` raises the number of active contracts** from one to two. Trying to
  accept a second contract without it is refused with a readable message.
- The laboratory still runs one project at a time; it cannot start a second while one is running.
- Parts in the missile workshop are gated by capabilities: locked parts are greyed out and the
  message names the technology to research, not an internal id.

### Missile workshop

- A staged vehicle built from 13 part types: solid motor, fuel tank, liquid engine, turbofan,
  pitot tube, barometer, basic radar, active radar, three flight computers, decoupler and mission
  module. Each part is resizable within validated ranges.
- **The vehicle is an attachment tree.** A part is mounted on a node of another part (`top`,
  `bottom`, or one of four radial mounts on motors, tanks and modules); the stages are derived from
  that tree, so a decoupler starts a new stage instead of a separate stage editor. The tree is saved
  as `designs/vehicle.json` schema 3; older stage-list files are migrated when read.
- The palette adds above the selected part when its top node is free, otherwise above the highest
  part. A second flight computer replaces the first in place, so the single computer stays in the
  last stage. A diameter step between a part and its mount is reported as a warning, not refused.
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
- The vehicle is saved to `designs/vehicle.json` on every change and reloaded at startup.
- Missile designs join the revision log (`designs/revisions.json`). The workshop shows the head
  hash, the change summary since it, the latest revisions and a `RECORD REVISION` button.

### Missile program editor

- The **PROGRAM** tab (`F4`) composes the onboard program as parallel threads of the seven blocks of
  `omega-rules::program`. The palette enables only what the fitted hardware unlocks: pitot tube →
  hold speed, barometer → hold pressure, radar → lock target, and a second stage → separate.
- Live meters compare threads and blocks with the flight computer class (1/8, 4/16, 8/32). Every edit
  is validated immediately and each issue names the thread and block at fault, or the missing
  hardware.
- Blocks are appended to the selected thread, selected, moved up or down, tuned with `-` / `+`
  (wait, stage number, throttle, speed, pressure) and removed. `Lock target` has no value and must
  close its thread.
- The program is saved to `programs/missile.json` on every change and reloaded at startup; its
  content hash is shown so a trial can be bound to it.

### Trials

- The **TRIALS** tab flies the saved vehicle with its onboard program and shows four bar plots
  (speed, altitude, mass, thrust), the event timeline (ignition, lift-off, pitch-over, separation,
  burnout, stall) and every anomaly in plain language, such as "stage 1 dropped with 280 kg of
  propellant unburnt".
- The seed is editable (`-10 / -1 / +1 / +10`) and the mission profile switches between a vertical
  launch and an air launch at 200 m/s. The input hash of vehicle, program, profile and seed is shown
  next to the report, so a design edited after a run is visibly stale.
- The seed rolls the target lock **and** a small manufacturing spread (±3 % engine thrust and
  solid-motor burn time, ±5 % drag), so a batch samples the design instead of repeating one flight.
  The flight stays deterministic: same vehicle, program and seed give the same report. The setup
  panel shows what the current seed does before a run: `ROLL 0.11` against the radar's 55 % reads
  `LOCK` and flips to `NO LOCK` as the seed changes, next to the thrust/burn/drag spread, and a
  warning says so when no radar or no `LOCK TARGET` block is fitted.
- Monte Carlo batches of 10 / 100 / 1,000 seeds summarize the outcome distribution and stay locked
  until the "Batch trials" research grants `analysis:monte_carlo`. A batch flies on a worker thread
  with a live progress line, and each report is folded into running totals so memory stays flat. A
  finished batch marks its result `STALE` when the vehicle, program, profile or seed changes.
- A/B comparison stores snapshot A, flies the edited design over the same ten seeds and prints the
  deltas for path, maximum speed and lock rate.
- With active contracts, every trial is recorded automatically as evidence, bound to the vehicle
  hash, the program hash and the seed.

### Missile contracts

- The **CONTRACTS** tab opens the missile procurement office: one offer per researched mission
  family (air-to-air, air-to-surface, surface-to-air, surface-to-surface) with a vehicle family
  (ballistic, cruise, short-range), a difficulty, a reward and a deadline. Accepting one closes the
  offer and starts its evidence clock.
- Two contracts can be active at once once `Parallel project management` is researched; each is
  listed with its own status and due day, and clicking `SELECT` shows its dossier.
- Requirements are abstract flight figures, not typed-in paperwork: path length, top speed, launch
  mass, lift-off and stage separation, plus a hard target lock on air-to-air and surface-to-air
  offers. They are evaluated by the shared `omega-rules` requirement and dossier machinery.
- Every proof carries the vehicle hash, the program hash and the trial seed, and the panel shows the
  bound hashes. Edit a part or a block after the trial and all rows turn `STALE`, the score drops to
  0 and delivery is disabled; reverting the edit restores the dossier.
- Deliver a current dossier with the reward paid, the delivered counter incremented and a fresh
  offer for that family. A passed deadline or an incomplete dossier refuses delivery and keeps the
  contract.

## Not implemented

- **Research still gates families only through their technology.** There is no reputation, no
  penalties, and a passed deadline refuses delivery instead of failing the contract.
- The office does not model production scheduling, staffing, inventory or island expansion.
- `analysis:monte_carlo` is consumed by the trial screen; the other grants are part and sensor
  capabilities.
- No arbitrary 3D CAD, no bending/buckling/fatigue/dynamic structural mechanics, no implicit time
  integration and no certified engineering validation.
- `omega-sim-core` (thermal finite volume, truss, meshing, mesh thermal, reports) is not wired to the
  game; it remains a standalone library with its own tests.

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

## Prebuilt Linux release

Tagged releases publish an x86_64 Linux archive containing the game binary, README, licence and
product vision. Download the archive from
[GitHub Releases](https://github.com/Sts-simon/Omega/releases), verify its SHA-256 file, extract it,
and run the `omega` binary from a graphical desktop session. The release archive is not a
self-contained AppImage: the system's graphics, audio and windowing runtime libraries are still
required.

The release workflow checks that the Git tag matches the Cargo package version before publishing.
Only Linux x86_64 is packaged at this stage.

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

## Interface and controls

The top bar switches views: **ISLAND**, **CONTRACTS**, **WORKSHOP** (missile workshop), **RESEARCH**,
**PROGRAM** (missile program), **TRIALS**. The right side of the bar shows the message lane and
`DAY n | credits | active contracts | delivered`. The island shows five facility cards; clicking a
card opens it.

| Input | Action |
|---|---|
| `F1` | Contract Office |
| `F2` | Missile workshop |
| `F3` | Research Centre |
| `F4` | Missile program editor |
| `F5` | Trials |
| `Escape` | Return to the island |

### Missile workshop

Editing is mouse-only: presets, part palette (locked parts greyed), stage buttons and `+ STAGE` /
`- STAGE`, the part list per stage, `-` / `+` on the selected part's adjustable values,
`REMOVE PART` and `RECORD REVISION`. Stage figures and validation update immediately.

### Research Centre

`START RESEARCH` on an available technology, `NEXT DAY` to advance the clock. The lab runs one
project at a time.

### Contract Office

| Input | Action |
|---|---|
| `1`–`4` | Accept the corresponding offer, if a slot is free and the family is researched |
| `V` | Deliver the selected contract when its dossier is current and accepted |

Each active contract has its own `SELECT`, `GO TO TRIALS` and `DELIVER` buttons; the dossier panel
follows the selected one.

### Camera

Right-drag orbits, middle-drag pans, wheel or touchpad scroll zooms; WASD/arrows pan and `Q`/`E`
change height. The camera is disabled on the contract, research and program boards or while the
pointer is over an interactive panel.

## Saves

All persistent files go through `src/save.rs` and `omega_data::SaveSlot`:

```text
saves/slot_1/
  manifest.json                 committed last; read verifies every file hash
  campaign.json                 credits, research, day (schema 2; schema 1 still loads)
  contracts/missile.json        offers, active contracts, evidence, deliveries (schema 2)
  designs/revisions.json        missile design revisions (schema 3)
  designs/vehicle.json          missile vehicle (schema 3, schema 1-2 migrated)
  programs/missile.json         missile program
```

Writes are atomic (temporary file, fsync, rename) and a damaged file is reported instead of loaded,
with the previous generation kept as `<name>.bak`. Files from the pre-0.22 layout (`saves/*.json`)
are imported once without being modified. Earlier saves load: campaign schema 1 keeps its credits,
contract schema 1 keeps its single active contract, and an assembly-era revision log (schema 1–2)
opens empty because those snapshots can no longer be restored.

## Project structure

```text
src/
  main.rs              Bevy app, orbit camera, ordered system chain
  campus.rs            views, campaign (credits, research, day), island picking, save
  campus/board.rs      card boards: island, contracts, research, workshop, program, trials
  procurement.rs       offers, active contracts, trial evidence, dossier, delivery, persistence
  revisions.rs         missile design revision log
  missile.rs           missile workshop session, parts, attachment tree view, 3D layout
  program_editor.rs    missile program session, validation, persistence
  trials.rs            seeded trial session, batches, comparison
  interface.rs         navigation bar, UiAction dispatch
  save.rs              SaveSlot access and 0.21 import
  ui_kit.rs            UI primitives (cards, chips, buttons, palette)
  scene.rs             procedural low-poly island and facilities
crates/
  sim-core/            standalone Bevy-free solvers: thermal FV, truss, mesh, mesh thermal, reports
  omega-data/          content hashes, atomic writes, versioned save slot
  omega-rules/         assembly graph, vehicle parts, program, flight, research, contracts,
                       requirements, dossier, metrics, diff
docs/                  architecture, vision, roadmap, changelog, model notes
examples/              sample assembly, chamber program, polygon sketch
```

## Scientific models and limitations

The missile figures (mass, thrust, burn time, TWR, delta-v) come from abstract part coefficients in
`omega-rules::vehicle`. The flight test integrates speed along a path with thrust, drag and, for a
vertical launch, gravity; altitude only changes air density. There is no steering, trajectory
optimisation, tracking or guidance law, and the lock is one seeded probability roll. The
manufacturing spread is a deterministic multiplier per seed (±3 % thrust and burn time, ±5 % drag),
not a validated tolerance model.

The contract requirements and dossier are an abstract game rubric: range, speed, mass, lift-off,
separation and lock are read from the trial and compared to limits derived from the offer's family
and difficulty. They are not engineering verification.

`omega-sim-core` keeps its Bevy-free thermal finite-volume, truss, meshing and mesh-thermal solvers
with their own tests; none of it is coupled to the missile game, and none is validated against
experimental data or certified for engineering use.

## Licence

MIT. See `LICENSE`.
