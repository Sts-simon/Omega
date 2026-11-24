# Architecture

## Product boundary

Omega is a Rust 2024 and Bevy game built around a fictional, abstract contract loop:

**contract → workshop → program → trial → dossier → delivery**

The game layer owns presentation, player progression and persistence. Reusable rules and data crates remain independent of Bevy so they can be tested without launching the application.

The project intentionally does not implement real-world weapon engineering, guidance or navigation laws, tracking algorithms, seeker logic, warhead models or real-world performance validation. Simulation values are game parameters.

## Workspace

- `omega` — Bevy application and user interface.
- `omega-data` — Bevy-independent persistence primitives, content hashes and save-slot handling.
- `omega-rules` — serializable game models and deterministic rules for vehicles, programs, trials, research, requirements, dossiers and contracts.
- `omega-sim-core` — standalone scientific-computing library retained in the workspace. The game application does not depend on it.

## Application modules

### `campus.rs` and `campus/board.rs`

Own the campaign state and facility navigation. The current views are the island, contract office, research centre, workshop, program editor and trials. The board module builds the facility and gameplay panels. Research progression and the campaign day/credit state are managed here.

### `procurement.rs`

Owns the contract office integration: available offers, accepted and completed contracts, active-contract capacity, deadlines, trial evidence, dossier state and delivery. Evidence is bound to the relevant vehicle/program inputs; changes to those inputs make dependent evidence stale.

### `missile.rs`

Owns the workshop editing session and the Bevy presentation of the staged vehicle. It handles part selection, stage changes, parameter editing, presets, validation feedback and the 3D preview. The authoritative serializable vehicle model lives in `omega-rules`.

### `program_editor.rs`

Owns the interactive editing session for the abstract onboard program. The rules crate remains the source of truth for block definitions and validation. The application presents available blocks, thread and memory usage, editing actions and validation results.

### `trials.rs`

Owns the trial screen and its presentation state: run configuration, deterministic seed, plots, event timeline, anomalies, batch progress and A/B comparison. Long batch work is kept off the main update path and results are associated with the inputs used to create them.

### `revisions.rs`

Tracks missile design revisions and change summaries. Revision records are persisted separately from the current vehicle and program.

### `save.rs`

Provides the application-level boundary around `omega-data::SaveSlot`. It maps application save types to paths inside the active slot and handles the one-time import of supported legacy campaign files.

### `interface.rs`, `scene.rs` and `ui_kit.rs`

Own navigation/action dispatch, island and camera presentation, shared UI primitives and interaction state. They should not become alternate sources of gameplay rules.

## Data and rules crates

### `omega-data`

Provides content hashing, atomic file replacement with a previous-generation backup, a versioned save-slot manifest, per-file integrity checks, validation of paths relative to the slot and import support for the documented legacy save layout.

The manifest is committed after the data files so it identifies a consistent saved generation. The application must report unreadable or corrupt data rather than silently treating it as a valid new campaign.

### `omega-rules`

Contains the serializable domain models and deterministic rules used by the game: staged vehicle model and validation; abstract program blocks, threads and resource validation; seeded trial calculations and result records; research tree, prerequisites and capability grants; contract families, requirements, offer generation and active-contract rules; evidence evaluation, dossier construction and input comparison.

These rules should remain testable without Bevy or UI state.

### `omega-sim-core`

A separate Bevy-free scientific library. It is retained for independent use and historical continuity, but it is not part of the missile game's runtime dependency path. Its scientific solvers must not be presented as validated weapon-design tools.

## Persistence layout

The application stores its active campaign under `saves/slot_1/`. The manifest indexes the files in the slot and records their hashes.

| File | Purpose |
|---|---|
| `campaign.json` | credits, research progression and campaign day |
| `designs/vehicle.json` | current staged vehicle |
| `designs/revisions.json` | missile design revision history |
| `programs/missile.json` | current abstract program |
| `contracts/missile.json` | offers, active/completed contracts and evidence |

The exact schema version belongs to each data model and must be changed only with an explicit migration and tests.

## Runtime and threading

The Bevy application owns interaction and rendering. Deterministic game rules are called through the rules crate. Expensive batch trial work must not run synchronously inside a frame update; progress and completed results are returned to the application and displayed there.

Keep worker inputs and outputs owned and explicit. Do not let worker tasks mutate Bevy world state directly.

## Change boundaries

- Put game rules in `omega-rules`, not in UI callbacks.
- Put file integrity and slot mechanics in `omega-data`, not in individual screens.
- Keep Bevy types out of the data and rules crates.
- Treat hashes as provenance identifiers, not as proof that a simulation is physically valid.
- Any change to persisted structures requires a migration or a documented reason why no migration is needed.
- Any change to trial inputs must preserve deterministic replay and invalidate dependent evidence.

## Historical documentation

The former assembly editor, civil contract workflow, chamber sequencer and scientific-study UI are historical features. Their implementation notes remain in the archived documents and changelog, not in the current application architecture.


## Workshop assembly graph (0.31.1)

`crates/omega-rules/src/assembly.rs` owns the design tree. A `Vehicle` is a list of `PlacedPart`s,
each with an optional `Attach { parent, node, angle_deg, separation }`; `NodeId` is `Top`, `Bottom`
or `Radial(0..3)`. Stage indices are derived by walking the tree from the single root: an edge
starts a new stage when its `separation` flag is set or when the parent is a decoupler.

`Vehicle::stage_views()` rebuilds the flat `Vec<Stage>` that the program validator, `flight::simulate`
and the dossier already consume, so those rules and their figures are unchanged. `validate()` refuses
cycles, occupied nodes, unknown parents, multiple roots, an engine without a tank in its stage and a
computer outside the last stage; `warnings()` reports diameter steps without blocking the design.

`src/missile.rs` keeps only the session (selection by part id, palette additions, parameter edits),
the 3D layout built from `Vehicle::poses()` and the revision diff. Save files carry
`schema_version: 4`; deserialization accepts a schema-3 graph or the old stage list and rewrites it as
4, and refuses a newer schema with an error instead of guessing.

## Aerodynamics (0.32.0)

`crates/omega-rules/src/aero.rs` is Bevy-free and owns every aerodynamic rule. Three leaf parts
(`Wing`, `FinSet`, `ControlSurface`) live on radial nodes; each design choice (planform, profile,
stabiliser role, fin layout, control family, light build) maps to one `Effect` table loaded from
`aero_effects.json` next to the module, so a coefficient lives in data and the code only maps an
enum to its key. A missing key is a programming error and panics at first use; a test walks every
variant, and `omega --aero-report` renders the same tables as `docs/AERO_BALANCE.md`.

`aero::figures(&Vehicle)` walks the tree once and returns `AeroFigures`: reference area, wing/fin/
control areas, area-weighted sweep, stability, control authority, lift, drag index, complexity,
reliability, aerodynamic mass, surface drag area, CG, CA, margin, stable band and balance. It is
computed on edit, not per flight step. `flight::simulate` applies it to the existing 1-D integration:
surface drag area, a lift-dependent air-breathing minimum speed and climb gravity, a stability-damped
seeded drag spread and a stability/authority-dependent altitude rate. A design without surfaces
leaves every term at its previous value.

`Vehicle::validate()` refuses aerodynamic parts on axial nodes, surfaces with children and moving
surfaces without a compatible computer; `Vehicle::warning_details()` reports oversized spans, heavy
surface mass, control surfaces with no fixed surface and an unstable CG/CA margin, each naming the
part it concerns so the workshop can select it. The workshop draws CG (yellow), CA (cyan), the
acceptable band and the surface plates from the same figures, quotes the selected surface's
contribution (`aero::breakdown`) and lists the surface fault risk of a fragile airframe.

## Screenless commands (0.33.0)

`src/headless.rs` runs before the Bevy app when a command flag is present: `--trial` flies the
saved design and program, `--revisions` replays the working copy and the newest revisions with one
seed, `--export-design` / `--import-design` move a schema-4 vehicle through the normal save path,
and `--aero-report` prints the effect tables. All of them reuse `omega-rules`, so the flight and
dossier rules are identical to the game's; only the rendering is skipped.
