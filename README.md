# Omega — Missile Design

[Version française](README.fr.md) | [English](README.md)

[![CI](https://github.com/Sts-simon/Omega/actions/workflows/ci.yml/badge.svg)](https://github.com/Sts-simon/Omega/actions/workflows/ci.yml)
<<<<<<< Updated upstream
[![Version](https://img.shields.io/badge/version-0.39.0-blue.svg)](https://github.com/Sts-simon/Omega/releases/tag/v0.39.0)
[![Licence: MIT](https://img.shields.io/badge/licence-MIT-green.svg)](LICENSE)

**Version 0.39.0** — a fictional missile-design game built with Rust and Bevy. The player accepts
=======
[![Version](https://img.shields.io/badge/version-0.47.0-blue.svg)](https://github.com/Sts-simon/Omega/releases/tag/v0.47.0)
[![Licence: MIT](https://img.shields.io/badge/licence-MIT-green.svg)](LICENSE)

**Version 0.47.0 (release candidate)** — a fictional missile-design game built with Rust and Bevy. The player accepts
>>>>>>> Stashed changes
military-themed contracts, assembles an abstract staged vehicle, edits its onboard program, runs
reproducible simulated trials and delivers a dossier. You build a staged missile in a 3D workshop, unlock parts, families and contracts through a
research tree that advances with a day clock, fly deterministic trials, and deliver a plan for
credits.

Everything in this project is fictional and abstract. Every mass, thrust, cost, duration and
probability is a gameplay value, not physical or real-world performance data. The project
deliberately models no real vehicle, no guidance or tracking law and no real warhead: an explosive
charge is an abstract payload mass with a game yield, and a target lock is a single seeded
probability roll. See `docs/VISION_MISSILES.md`.

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

- 22 technologies in six departments (missile families, propulsion and structures, aerodynamics,
  sensors and electronics, onboard computing, design and test tools), each with a credit cost, a
  duration in days and prerequisites. Aerodynamics, fin stabilisation, control surfaces, advanced
  profiles and light aero structures unlock the airframe choices; improved and advanced explosive
  charges add researchable payload grades; flight control electronics and navigation and datalink
  unlock the mission modules.
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
- **Dependency view.** Each department column is a lane: cards show their tier, their prerequisite
  chips and a `PARCOURS` line from the first technology to the last. Every card previews the real
  parts and campaign slots its grants unlock; startable root technologies are marked `NOUVELLE
  POSSIBILITÉ`. A compact comparison strip lists the affordable next technologies with cost,
  duration and unlocks, so the tree is useful without opening every card.

### Missile workshop

- A staged vehicle built from 26 part types: solid motor, fuel tank, liquid engine, turbofan,
  pitot tube, barometer, basic radar, active radar, three flight computers, control augment,
  inertial platform, datalink, power cell, self-test suite, decoupler, mission module, three
  explosive charge grades, adapter, player-placed nose cone, wing, fin set and control surface.
  Each part is resizable within validated ranges; fixed-section parts (sensors, computers,
  decouplers, payloads and mission electronics) have a mounting diameter in 5 cm steps so they can
  match a wider body without an adapter.
- **Explosive charges are a trade-off, not decoration.** A charge carries its own mass and an
  abstract game yield. Standard charges are available from day one; improved and advanced grades
  are research projects, and a grade converts the same mass into more yield. Contracts ask for a
  minimum charge mass and yield, so the choice interacts with range and launch-mass limits.
- **The nose cone is a part, not an automatic cap.** The palette's `NOSE CONE` exposes length,
  base diameter and a conical, ogive or parabolic profile. Removing it leaves the stack open; it
  closes the top node until the player removes it to add another part.
- **The vehicle is an attachment tree.** A part is mounted on a node of another part (`top`,
  `bottom`, or one of eight radial mounts on motors, tanks and modules); the stages are derived from
  that tree, so a decoupler starts a new stage instead of a separate stage editor. The tree is saved
  as `designs/vehicle.json` schema 7; schema 3-6 graphs and older stage-list files are migrated
  when read.
- **The workshop reads as one screen.** The palette groups parts under the rules' own categories
  (propulsion, airframe, aerodynamic surfaces, sensors, avionics); the right column shows the
  assembly as an indented, collapsible tree (node, stage, part count, lock, hide, rename and
  whole-group selection), the selected part's attachment, stage, dimensions, category and missing
  research, a dedicated assembly-control panel that groups construction problems (unconnected
  parts, occupied mounts, category conflicts, locks and badly connected subassemblies) with every
  row a shortcut to the part at fault, and the contract check panel. The latter compares the design
  on paper with the selected active contract and marks each figure `MET`, `CLOSE`, `MISSED` or
  `TRIAL` when only a flight can prove it.
- **Selection is visible and works from both views.** Hovering a part outlines it in cyan; the
  selected parts (amber, red when locked) are outlined in the 3D view and highlighted in the tree.
  `Ctrl+click` adds or removes one part, clicking a member of a group keeps the group and makes it
  the gizmo's primary part, `Escape` clears the selection, and a tree group button selects its
  whole subtree. Moving a multiple selection applies the same local-axis delta to every independent
  subtree root, so no hierarchy is broken and no part is moved twice.
- **The tree is a build tool.** Collapse or expand a group, rename it, read its part count and
  derived stage, hide it temporarily (the 3D view and picking skip the whole subtree while the
  rules still count it) and lock it against accidental edits. A lock covers the subtree; deleting
  or moving an ancestor that contains a locked part is refused instead of silently taking it along.
- **The camera has complete framing controls.** `F` frames the selection (all selected parts when
  there are several), `Home` frames the vehicle, `1`–`5` give front, side, top, rear and isometric
  views, and every framing transition eases into place. `V` switches between orbital and free
  cameras, and `[` / `]` or the palette buttons change the camera speed. The same commands are
  clickable in the palette, so the mouse alone is enough.
- **The palette is searchable and documented in place.** `/` opens a name/description search,
  categories filter it, `+`/`*` toggles a favourite, recent parts stay at the top and hovering an
  entry shows a rotating 3D preview, its role and the technology that still locks it. The panel
  also carries the click, Ctrl+click, Escape, W/R, F/Home, 1–5, V and undo reminders.
- **The selected-part inspector explains the choice.** It states the part's role, what its
  settings change, the incompatibility easiest to miss and any missing research, next to the
  editable parameters, the lock/hide/group actions and the group rename field.
- **The selected-part inspector edits exact local transforms.** Click any position or rotation
  value, type a decimal number and press Enter; Escape cancels and an invalid entry leaves the
  vehicle untouched. The precision selector (coarse, normal, fine, very fine) chooses the `-`/`+`
  step, and dedicated buttons zero one axis, all translations, all rotations or restore the
  initial mounting. Gizmo drags update the displayed values immediately and each gesture is one
  undo entry; a validated entry moves the 3D model and the gizmo in the same operation. The root
  part stays locked.
- **Mission electronics are a stack, not decoration.** Five modules cover control, navigation,
  communication, power and diagnostics: a control augment adds authority, a navigation platform
  and a datalink raise the lock chance, a power cell feeds the mission bus and a self-test suite
  halves the airframe fault exposure. The rules refuse a module without a cell, a datalink without
  navigation, a control augment on a basic computer and a second copy of any non-power module;
  they warn about thin power headroom and modules that would be dropped early. The selected-part
  panel quotes each module's cost, draw, complexity, effect and dependency, and the stage panel
  summarises devices, power margin and cost. Effects reach the trial through the flight report
  (`avionics`, `surface_fault`), so a saved design and its evidence stay hash-bound.
- **Adapters bridge a diameter change.** A body adapter carries its inlet and outlet diameters,
  length and a conical, ogival or stepped profile; it absorbs the diameter step it spans so the
  two neighbouring parts no longer earn a warning, and its mass follows the chosen profile. The
  workshop draws the taper as one continuous rounded body and the outlet meets the next part.
- **Parts are placed with the mouse.** Drag a palette entry onto any free node shown in the 3D view;
  a transparent, real-size ghost follows the nearest mount, turns green when the attachment is legal
  and red when it is not, and a plain click still adds above the selected part (or the highest one).
  Drag an existing part onto another node to move its whole subtree, `Delete` removes the selection,
  `X` cycles radial symmetry (1 → 8) and `Shift+X` switches between radial, bilateral and axial
  sets; `Ctrl+Z` / `Ctrl+Y` undo and redo up to 50 edits.
- **Advanced symmetry and subassemblies (0.42).** A linked set shares parameter edits, design
  choices and gizmo transforms across all its copies; `LIER LA SÉLECTION` reassociates existing
  parts and `ROMPRE` detaches one. `DUPLIQUER` clones a subtree on a free mount while preserving its
  inner edges. The subassembly library captures surfaces, electronics, structures, stages or a
  whole configuration, reinserts them with their relative attachments, and exports/imports them as
  JSON (`exports/subassemblies/`). A dedicated assembly-control panel groups unconnected parts,
  occupied mounts, category conflicts, locks and badly connected sets; every row selects its part.
- The wheel adjusts the selected part's main value when the pointer is over it, and zooms the camera
  everywhere else. A second flight computer replaces the first in place, so the single computer
  stays in the last stage. A diameter step between a part and its mount is reported as a warning,
  not refused.
- **Aerodynamics is a design decision.** A wing, fin set or control surface mounts on a free radial
  slot, mirrors with `X`, and carries span, chord and (for a wing) sweep plus discrete choices:
  planform, airfoil profile, stabiliser role, fin layout, control family and standard/light build.
  The workshop quotes the resulting stability, control authority, lift, drag, complexity,
  reliability and aerodynamic mass, plus the centre of gravity, the centre of aerodynamics, the
  stable band and an A/B snapshot. Moving surfaces need a compatible flight computer; an oversized
  or unstable design is a warning, not a refusal.
- Presets: two-stage rocket, cruise vehicle, or an empty vehicle. Presets whose parts are not yet
  researched are refused with the missing technologies.
- Selected part resizing with `-` / `+`: length and diameter (motors, tanks), burn time (solid
  motors), engine scale (liquid and air-breathing engines), module mass. All edits are clamped and
  validated.
- Live figures per stage: wet mass, mass carried at ignition, thrust, burn time, thrust-to-weight
  ratio (red below 1 and amber below 1.3 for the first stage) and delta-v; total mass; computer
  class with its thread and memory limits; the programming blocks the installed sensors unlock;
  research still missing; and design validation messages.
- The 3D stack is rebuilt on change with rounded rectangular airframe sections, panel joints, bell
  nozzles, fan nacelles, tapered wing plates and an ogive nose, all in one muted military palette.
  Air-breathing cruise vehicles are laid out horizontally so their wings read as wings; the camera
  frames either orientation when the workshop opens.
- The vehicle is saved to `designs/vehicle.json` on every change and reloaded at startup.
- Missile designs join the revision log (`designs/revisions.json`). The workshop shows the head,
  its hash, the last two revisions with their labels and associated trial counts, the submitted
  revision and a `RECORD REVISION` button. Every revision can be renamed, compared with the head
  (`vehicle_diff`) or restored; restoring creates a new revision named `Restored from …`, so the
  history is never rewritten. Matching trial campaigns are those whose `revision_id` is the same.

### Missile program editor

- The **PROGRAM** tab (`F4`) composes the onboard program as parallel threads of 21 generic blocks
  in the eight categories of `omega-rules::program`: inputs (wait for a speed or a pressure),
  conditions (wait for a flag), logic and control (toggle a flag, if/else, wait until, repeat N,
  repeat while, break, call routine), states (set a flag), timers, outputs
  (ignite, separate, throttle, hold speed or pressure, lock target), modes (set and wait for a
  mode) and diagnostics (leave a numbered mark). The palette enables only what the fitted hardware
  unlocks: pitot tube → speed blocks, barometer → pressure blocks, radar → lock target, and a second
  stage → separate.
- **A real canvas.** Blocks are cards on a pannable, zoomable surface; drag a card to move it freely,
  drag the background to pan and use the wheel or `ZOOM -` / `ZOOM +` to scale. Elbow connectors run
  between consecutive blocks, control cards show their THEN / ELSE / BODY rows, and named groups
  draw a purple label and border. `ALIGN` and `RESET VIEW` restore an orderly layout.
- **Branches, loops and routines.** `IF / ELSE`, `WAIT UNTIL`, `REPEAT N`, `REPEAT WHILE`, `BREAK` and
  `CALL ROUTINE` are editable from the card: select a branch, then add blocks from the palette into
  it. A selection can be captured as a reusable routine, called from several places, and deleted;
  unknown calls, duplicate names, cycles and recursive memory are rejected. Calls are capped at
  depth 8 and loops at 1,024 iterations.
- **Multi-selection and bulk edits.** `MULTI` toggles add/remove selection on click; `COPY`, `PASTE`,
  `DUPLICATE`, `DELETE`, `GROUP` and `UNGROUP` act on the selection. The rules still reject an
  invalid connection: nothing can follow `Lock target`, and a stage cannot be ignited after it was
  separated.
- Live meters compare threads and blocks with the flight computer class (1/8, 4/16, 8/32). Every edit
  is validated immediately: errors block the trial and warnings (a condition nobody sets, an empty
  thread, a 0 s wait, thin memory) are listed separately; each issue names its thread and block.
- Blocks are selected, moved up or down, tuned with `-` / `+` (time, stage, throttle, speed,
  pressure, flag, mode, mark), flipped (`FLIP VALUE`), duplicated, inserted and removed.
- **The debugger** runs the current program once with a block trace (`RUN`) and plays it back: the
  current card is cyan, completed cards green, waiting cards yellow and anomalies red. `PAUSE` /
  `PLAY`, step buttons, a 0.5×–4× speed and `RESET` drive the cursor; the inspector shows the current
  flags and mode, and the validation panel shows the final flags. Any edit clears the trace so the
  panel cannot show a stale run.
- **Reusable library.** `SAVE COPY` stores the current program under a free name in
  `programs/library.json`; any saved program can be loaded on any vehicle (the rules then report the
  hardware it misses) or deleted.
- The working program is saved to `programs/missile.json` on every change and reloaded at startup;
  its content hash is shown so a trial can be bound to it.

### Trials

- The **TRIALS** tab is a three-page laboratory. **DASHBOARD** groups the run summary, phase table,
  four trajectory plots (speed, altitude, mass, thrust), important events, statistical chips,
  contract compliance and anomalies. **COMPARISON** puts snapshot A, the current design B and the
  last campaign C side by side on mass, fictional cost, stability, authority, complexity and
  reliability, then repeats the exercise for trial results and contract requirements. Every line
  shows absolute values, the B−A difference and its percentage, and zero baselines are shown as
  `—` instead of an invented ratio. **HISTORY** lists campaigns with their revision, program
  version, seed range and stale state, and opens a detail page with phases, hashes, changes and
  replay/export actions.
- A run shows four bar plots, the event timeline (ignition, lift-off, pitch-over, separation,
  burnout, stall) and every anomaly in plain language, such as "stage 1 dropped with 280 kg of
  propellant unburnt". Anomalies are classified (instability, component failure, resource
  shortage, incompatible program, contract requirement) and link to the workshop part or the
  program block when one can be identified.
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
  deltas for path, maximum speed, lock rate, missed lift-offs and separations.
- With active contracts, every trial is recorded automatically as evidence, bound to the vehicle
  hash, the program hash and the seed. The laboratory keeps a persisted campaign history in
  `trials/history.json`: it stores the full vehicle/program snapshots, seed, profile, revision and
  program version. `RELAUNCH` replays that snapshot exactly, even after the current design moved on;
  an entry is marked obsolete when its design or program no longer matches the workshop.
- **Exports.** `HTML` writes a standalone report with inline SVG trajectory charts, phase and
  history tables; `CSV` writes the per-sample table; `PDF` writes a self-contained technical
  dossier with a trajectory chart and the revision change summary. Files go to
  `saves/slot_N/reports/` and never modify the campaign.

### Missile contracts

- The **CONTRACTS** tab opens the missile procurement office: up to two mission-profile offers per
  researched family (air-to-air, air-to-surface, surface-to-air, surface-to-surface), so the board
  shows at most eight cards. Each offer names its mission profile, vehicle family (ballistic,
  cruise, short-range), difficulty, reward and deadline. Accepting one closes the offer and starts
  its evidence clock.
- Two contracts can be active at once once `Parallel project management` is researched; each is
  listed with its own status and due day, and clicking `SELECT` shows its dossier.
- Requirements are abstract flight figures, not typed-in paperwork: path length, top speed, launch
  mass, lift-off, a minimum charge mass and yield, and either a low-altitude discretion score or a
  high-altitude apogee. Ballistic profiles also require stage separation; short-range and cruise
  profiles do not, so a single-stage design is viable. Air-to-air and surface-to-air offers add a
  hard target lock. They are evaluated by the shared `omega-rules` requirement and dossier
  machinery.
- Every proof carries the vehicle hash, the program hash and the trial seed, and the panel shows the
  bound hashes. Edit a part or a block after the trial and all rows turn `STALE`, the score drops to
  0 and delivery is disabled; reverting the edit restores the dossier.
- Deliver a current dossier with the reward paid, the delivered counter incremented and the board
  topped back up. A passed deadline or an incomplete dossier refuses delivery and keeps the
  contract. A delivered airframe cannot be sold again under a new name or renamed part ids: one
  airframe, one contract.
- **Detailed offers.** Every card now also carries a fictional context, a design budget, one
  primary mass/reliability/complexity goal set, optional stretch goals and a bonus, plus the
  requested dossier sections. Meeting the primary goals and staying inside the budget are delivery
  gates; meeting every secondary goal adds the bonus to the quality-scaled fee. The active panel
  shows live values, the goal marks, the submitted revision and the requested sections.
- **Delivery dossier.** A successful delivery writes a second PDF into `saves/slot_N/reports/`,
  named `contrat-<id>-dossier-<hash8>.pdf`. It contains the fictional context, general vehicle
  view, assembly tree, equipment, design parameters, embedded program, matching trial campaigns,
  anomalies, validated requirement matrix and the submitted revision hash.

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

The same archive can be produced locally with `scripts/package-linux.sh`; see
`docs/INSTALLATION.md` for runtime packages, checksum verification and launch troubleshooting.

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

Editing is mouse-first: presets, part palette (locked parts greyed), drag a part from the palette to
a glowing node in the 3D view or drag an existing part to move its subtree, `Delete`, `X` for the
radial copy count and `Shift+X` for the symmetry mode, `Ctrl+Z` / `Ctrl+Y`, the derived part list
with click-to-select, `-` / `+` on the selected part's adjustable values (or the wheel over the part,
one undo entry per continuous gesture), `UNDO` / `REDO`, `REMOVE PART` and `RECORD REVISION`.
`Shift+D` duplicates the selection, `Shift+K` breaks a symmetry link and `Shift+J` links the current
selection. Escape cancels a drag and keeps the workshop open; a second Escape returns to the island.
Stage figures, assembly control and validation update immediately.

### Research Centre

`START RESEARCH` on an available technology, `NEXT DAY` to advance the clock. The lab runs one
project at a time.

### Contract Office

| Input | Action |
|---|---|
| `1`–`4` | Accept the corresponding offer, if a slot is free and the family is researched |
| `V` | Deliver the selected contract when its dossier is current and accepted |

**Offers are mission profiles, not only family thresholds.** The office can list several profiles
per mission family: deep strike, veil raid, precision strike, air superiority, long-range intercept,
point defence, area defence, theatre strike and sea skimmer. Each has its own client, brief, reward,
deadline, range requirements and emphasis on design or program quality. Low-altitude profiles
require a high **discretion** score, while high-apogee profiles require the trial to reach an
altitude band. The trial derives discretion from its peak altitude, so a high ballistic flight and a
low cruise flight cannot prove the same profile. The dossier also requires an explosive charge mass
and yield, and a profile-bearing contract only accepts evidence from its own vehicle family.

The office refuses to deliver the exact same airframe to a second contract: build a distinct design
for another mission. Each active contract has its own `SELECT`, `GO TO TRIALS`, `RE-FLY` (when its
evidence is stale or missing) and `DELIVER` buttons; the dossier panel follows the selected one. A
delivered dossier pays the nominal fee at the 60 % acceptance threshold and up to +15 % for a perfect
one.

### Camera

Right-drag orbits, middle-drag pans, wheel or touchpad scroll zooms; WASD/arrows pan and `Q`/`E`
change height. The camera is disabled on the contract, research and program boards or while the
pointer is over an interactive panel.

## Screenless commands

The binary also runs without opening a window, which is how flight behaviour is checked in CI and
over SSH:

```text
omega --trial saves/slot_1 --seed 3           # fly the saved design and program, print a JSON report
omega --trial saves/slot_1 --air             # same, from an air launch at 200 m/s
omega --revisions saves/slot_1 --seed 3      # replay the working copy and up to ten revisions, with deltas
omega --export-design saves/slot_1 design.json
omega --import-design saves/slot_1 design.json
omega --aero-report                          # render the effect tables as Markdown (docs/AERO_BALANCE.md)
omega --balance-report                        # parts, contracts and research audit (docs/BALANCE.md)
```

`--import-design` refuses a design that does not validate and never writes a partial save. The design
file is the schema-5 vehicle itself, so an export from an older build imports after the normal
migration. The complete contract loop still needs the window; these commands cover the flight,
comparison, export and balance parts. Installation, release archives and launch troubleshooting
are documented in `docs/INSTALLATION.md`; the tuning audit is in `docs/BALANCE.md`.

## Saves

The title screen manages six campaign slots, `saves/slot_1/` to `saves/slot_6/`; the game waits for
one to be created or loaded before it starts. All persistent files go through `src/save.rs` and
`omega_data::SaveSlot`:

```text
saves/slot_N/
  manifest.json                 committed last; read verifies every file hash
  campaign.json                 credits, research, day (schema 2; schema 1 still loads)
  contracts/missile.json        offers, active contracts, evidence, deliveries (schema 2)
  designs/revisions.json        missile design revisions (schema 3)
  designs/vehicle.json          missile vehicle (schema 7; schema 3-6 graphs and 1-2 lists migrate)
  programs/missile.json         missile program
  programs/library.json         named programs, reusable on another vehicle
```

Writes are atomic (temporary file, fsync, rename) and a damaged file is reported instead of loaded,
with the previous generation kept as `<name>.bak`. Files from the pre-0.22 layout (`saves/*.json`)
are imported once into slot 1 without being modified. Earlier saves load: campaign schema 1 keeps its credits,
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
  subassembly.rs       reusable subassembly capture, library JSON and export/import
  headless.rs          screenless trial, revision comparison, design export/import, balance report
  picking.rs           camera-ray picking and screen-space node snapping (pure, tested)
  workshop.rs          pointer system: palette drag, subtree move, delete, symmetry, undo, wheel
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
separation, lock, explosive charge mass and yield, peak altitude and a discretion curve are read
from the trial and compared to limits derived from the offer's profile and difficulty. Discretion is
only `100 * exp(-peak altitude / 7000 m)`, a gameplay curve; it is not a radar-cross-section or
sensor model. They are not engineering verification.

`omega-sim-core` keeps its Bevy-free thermal finite-volume, truss, meshing and mesh-thermal solvers
with their own tests; none of it is coupled to the missile game, and none is validated against
experimental data or certified for engineering use.

## Licence

MIT. See `LICENSE`.
