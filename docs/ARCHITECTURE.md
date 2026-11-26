# Architecture

## Product boundary

Omega is a Rust 2024 and Bevy game built around a fictional, abstract contract loop:

**contract → workshop → program → trial → dossier → delivery**

The game layer owns presentation, player progression and persistence. Reusable rules and data crates remain independent of Bevy so they can be tested without launching the application.

The project intentionally does not implement real-world weapon engineering, guidance or navigation laws, tracking algorithms, seeker logic, real-warhead models or real-world performance validation. An explosive charge is an abstract payload mass with a game yield; simulation values are game parameters.

## Workspace

- `omega` — Bevy application and user interface.
- `omega-data` — Bevy-independent persistence primitives, content hashes and save-slot handling.
- `omega-rules` — serializable game models and deterministic rules for vehicles, programs, trials, research, requirements, dossiers and contracts.
- `omega-sim-core` — standalone scientific-computing library retained in the workspace. The game application does not depend on it.

## Application modules

### `campus.rs` and `campus/board.rs`

Own the campaign state and facility navigation. The current views are the island, contract office, research centre, workshop, program editor and trials. The board module builds the facility and gameplay panels. Research progression and the campaign day/credit state are managed here.

### `procurement.rs`

Owns the contract office integration: available offers, accepted and completed contracts, active-contract capacity, deadlines, trial evidence, dossier state and delivery. Offers carry a mission profile (payload, yield, discretion or apogee demand), and a profile-bearing contract only records trials flown by the matching vehicle family. Evidence is bound to the relevant vehicle/program inputs; changes to those inputs make dependent evidence stale. Delivery also records a structural design fingerprint (name and part ids removed, tree and attachment transforms kept) so one airframe cannot be sold to two contracts by renaming it or its parts.

### `missile.rs`

Owns the workshop editing session and the Bevy presentation of the staged vehicle. It handles part selection, stage changes, parameter editing, presets, validation feedback and the 3D preview. The authoritative serializable vehicle model lives in `omega-rules`.

### `inspector.rs`

Owns the numeric selected-part inspector: the six local transform fields (three offsets in metres and three rotations in degrees), keyboard entry and cancellation, the four precision steps used by `-`/`+`, axis and group resets, and restoration of the initial mounting. Committed values go through `MissileSession::set_transform_values`, so validation happens before the vehicle is touched and one user action is one undo entry. The module also refreshes field text when a gizmo drag changes the vehicle. It does not change the save schema.

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

The title screen manages six campaign slots under `saves/slot_1/` to `saves/slot_6/`. Selecting a slot makes it active; the manifest indexes the files in that slot and records their hashes. No gameplay file is written while the title screen has no active slot.

| File | Purpose |
|---|---|
| `campaign.json` | credits, research progression and campaign day |
| `designs/vehicle.json` | current staged vehicle |
| `designs/revisions.json` | missile design revision history |
| `programs/missile.json` | current abstract program |
| `programs/library.json` | named programs, reusable across vehicles |
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

`PlacedPart::size_scale` (default 1) lets the fixed-section parts match a wider body. It multiplies
only the radius returned by `PlacedPart::dimensions()`; `poses()`, the node-diameter warning,
picking and the mesh all use that one method. `PartSpec::NoseCone` is a normal placed part with a
length, base diameter and `NoseShape`; nothing generates a cap implicitly. The nose closes the top
node once placed, and `node_status` refuses a child above it.

`src/missile.rs` keeps only the session (selection by part id, palette additions, parameter edits),
the 3D layout built from `Vehicle::poses()` and the revision diff. Save files carry
`schema_version: 7`; deserialization accepts a schema-3 to schema-6 graph or the old stage list and
rewrites it as 7, and refuses a newer schema with an error instead of guessing.

Since 0.42, symmetry metadata stays inside the existing `PlacedPart::symmetry` string. A new set is
encoded as `<key>-<kind><index>` (`r`, `b`, `a`), while an old `<key>` decodes as a radial first
copy; `Vehicle::symmetry_members`, `symmetry_kind_of` and `symmetry_placements` are the only readers.
Bodies expose eight evenly spaced radial mounts, with `radial_angle` shared by poses, layout,
picking and the placement preview. The session propagates a parameter edit to every member, and
maps a gizmo translation/rotation delta through the set's rotation or mirror before applying it to
each copy. `link_symmetry` reassociates existing parts; `break_symmetry` clears the link without
touching geometry.

`src/subassembly.rs` stores an exported forest with relative parent indices, transforms,
separation flags and mounting sizes. `Subassembly::capture` walks selected subtrees,
`MissileSession::apply_subassembly` replays the inner edges before mounting each root, and the
library itself lives in the slot file `designs/subassemblies.json`. Standalone exports are plain
JSON files under `exports/subassemblies/`, deliberately outside the slot so another design or
campaign can import them. Duplication follows the same rule locally: cloned parts keep their inner
parent links and are attached to free mounts of the original parent.

## Workshop screen (0.34.0, extended in 0.42.0)

The workshop is one screen with four panels and one 3D view: a category-grouped palette (with the
subassembly library at its foot), an indented assembly tree, a selected-part inspector and a
dedicated assembly-control panel, plus the middle column with the aerodynamic sheet, the contract
check and the stage figures. Dragging a palette entry previews a transparent real-size mesh at the
nearest free mount; the preview material turns green or red with mount validity, and a red mount
cannot be released. The ghost and the node markers are presentation only; the rules still decide.

Every panel reads a rule, none owns one:

- the palette order comes from `PartCategory::ALL` and `PartSpec::category()`;
- the tree comes from `Vehicle::tree_entries()` (depth-first, parent, node, derived stage), filtered by the session's collapsed set and decorated with part counts, locks, hides and group names;
- selection, collapse, hide, lock, group names, favourites, recents and the palette query are session-only editor state, so they redraw a board without changing the schema-7 vehicle;
- hover and the 3D palette preview live outside the session (`WorkshopHover`, `PalettePreview`): hovering must not mark the design dirty, and the preview renders on its own camera and render layer to a target texture;
- the camera rig stores its orbital target and an optional interpolated `CameraGoal`; `F`/`Home`/view keys and the palette camera buttons all feed the same goal, while free mode changes what a drag or the wheel moves;
- errors and warnings come from `Vehicle::issues()` (`IssueSeverity`, message, optional part id),
  with `validate()` kept as the error-only string view;
- the contract rows come from `omega_rules::compliance::contract_preview`, which reuses
  `ContractOffer::missile_requirements()` and only predicts the figures the design determines
  (launch mass, stability, lift, authority), marking the rest `TRIAL`;
- the figures, stage reports and selected-surface contribution keep their existing owners.

Selecting a validation row or an aero warning dispatches `MissileAction::Select`, so every panel
feeds the same session and selection state.

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

## Program editor (0.36.0, extended in 0.43.0)

`crates/omega-rules/src/program.rs` owns the whole language: 21 blocks, eight categories, four
flags, a mode register, eight diagnostic marks and the validation. `program_issues()` returns one
`ProgramIssue` per problem with a severity, a thread and a block, so the editor can colour and point
at the fault; `validate_program()` is the error-only string view the flight and the trials keep
using. Connection rules live there too: `Lock target` must close its chain and a stage cannot be
ignited after it was separated.

`Block` is recursive: `If`, `Repeat` and `RepeatWhile` carry their own `Vec<Block>` bodies, `Break`
closes the nearest loop and `Call` names a `Routine`. `Program` also carries `routines` and a
`ProgramLayout` (zoom, pan, per-block positions and named groups), both `#[serde(default)]` so old
JSON loads unchanged. `block_count()` counts nested bodies and routines, so memory validation covers
what the flight can actually execute; `MAX_CALL_DEPTH` and `MAX_LOOP_ITERATIONS` keep execution
bounded.

`Vehicle::available_blocks()` maps the fitted hardware to the block kinds, and the palette groups
`BlockKind::ALL` by `BlockCategory::ALL`, so the editor has no rule of its own. The flight executes
input and condition blocks as gates with a timeout; each thread owns a stack of `ExecFrame`s so a
branch, loop pass or routine call returns to exactly the frame that pushed it. `FlightConfig::trace`
records a `BlockTrace` per executed block into `FlightReport::trace`, including the flag/mode state
after the block; this is the debugger's only input. `src/campus/program_canvas.rs` renders the cards
from `ProgramLayout`, moves the live nodes during a drag and persists positions on release; the
board signature includes the selection and debug cursor so a click or a playback step redraws it.
The named program library is `ProgramLibrary` (12 entries, unique names); `src/program_editor.rs`
writes it to `programs/library.json` through the same save slot as the working program.

## Trial laboratory (0.44.0)

`src/lab.rs` is the analysis layer between `omega-rules::flight` and the screen. It turns a
`FlightReport` into `DesignMetrics` (mass, fictional cost, stability, authority, complexity,
reliability), `PhaseSummary` slices, `TrialStats`, classified `AnomalyInsight` values and a
`ComplianceSnapshot` built from the active contract's dossier. `analyze_report_anomalies` and
`analyze_failure` attach the component id or the program `BlockFocus` when one can be identified;
the board dispatches those anchors to the workshop or the editor. `compare_rows` is the single
source of the A/B table, so absolute and percentage values cannot diverge between screens.

`TrialSession` owns `TrialHistory`. Every completed run or batch appends a `TrialHistoryEntry`
containing the full `Vehicle` and `Program` snapshots, seed range, profile, revision context,
program version, hashes, metrics, stats, phases, anomalies, events, optional compliance snapshot
and the id it was relaunched from. `src/save.rs` maps the history to `trials/history.json`; loading
a slot resets or reloads it. `stale_reason` compares the stored hashes with the current semantic
hashes, so a moved program card never invalidates a proof while a design or program edit does.
Relaunch uses `entry_config` and the stored snapshots, not the current workshop state.

`src/trial_export.rs` writes `reports/*.html`, `*.csv` and `*.pdf` under the active slot. HTML and
PDF embed their own charts (inline SVG and a vector line chart respectively) and repeat the
campaign history plus the revision change summary; no external renderer or asset is required.
Exports read the selected campaign and never write to the campaign files.

## Contracts and progression (0.45.0)

`omega-rules::contracts::ContractOffer` remains the persisted offer shape. The 0.45 fields
(`context`, `design_budget_cr`, `primary_goals`, `secondary_goals`, `secondary_bonus_cr`,
`requested_dossier`) use `#[serde(default)]`, so old campaign schemas load without migration.
Goals are integer ratings (`DesignGoalMetric::{MassKg, Reliability, Complexity}`) to keep the
offer `Eq` and the save comparison deterministic. The application evaluates them with the same
`src/lab.rs::DesignMetrics` used by the trial dashboard; `goal_statuses` and `budget_ok` are the
single source for the offer card, active dossier and delivery gate.

`Procurement::deliver` now takes the submitted revision id. It refuses a design over budget or
missing a primary goal, computes the secondary bonus, and records `revision_id` and
`secondary_bonus` on `CompletedContract`. Successful deliveries call
`trial_export::export_contract_dossier`, which writes `contrat-<id>-dossier-<hash8>.pdf` under the
active slot's `reports/` directory. The pure `contract_dossier_lines` builder is separate from the
PDF writer so the content can be tested without touching a save slot. It reuses
`Vehicle::tree_entries`, the board's French part summaries and `TrialHistory` entries filtered by
vehicle and program hash.

`src/revisions.rs::RevisionLog` keeps schema 3. `submitted` is a persisted optional revision id;
`compare`, `naming_target` and `naming_draft` are `#[serde(skip)]` UI state. A rename changes only
the label. A comparison calls the existing `missile::vehicle_diff` from one revision to another and
stores the pair. A restore copies the selected revision's vehicle into `MissileSession` and commits
it as a new revision (`Restored from …`), preserving the original history and parent chain. The
workshop keyboard reader owns naming while the workshop is open; the footer also offers explicit
validate/cancel buttons. Trial campaigns remain associated by `TrialHistoryEntry::revision_id`, so
the footer can count evidence for each revision.

The research board derives tiers with a memoised prerequisite walk, previews every grant through
`PartTemplate::ALL`/`PartSpec::required_capability`, and labels non-part grants with
`research::capability_label`. The available-technology strip is a presentation of the same
`default_tree`, not a second tree, so adding a technology still requires a real grant.

## Onboard electronics (0.35.0)

`crates/omega-rules/src/avionics.rs` owns the family: seven `AvionicsCategory` values, five mission
modules and one `AvionicsSpec` shape used by the legacy computers and sensors too. `spec_of()` maps
any part to its electronics view, `figures()` makes one pass over the tree and `issues()` returns
ordinary `VehicleIssue`s, which `Vehicle::error_issues()` and `warning_issues()` merge with the
assembly and aerodynamic rules. The module therefore owns every electronics rule: categories,
costs, power, dependencies, duplicates and the power budget.

`PartSpec::Avionics { module }` is the only new save variant; mass comes from `stats()`, dimensions
from `avionics::dimensions()` and the palette category from `PartSpec::category()`. The trial reads
`AvionicsFigures` once, applies the authority, lock and fault multipliers to the existing flight
integration, and quotes `AvionicsSummary` and `surface_fault` on the report. A design without
mission modules keeps every previous figure, so no pre-avionics save changes behaviour.

## Screenless commands (0.33.0)

`src/headless.rs` runs before the Bevy app when a command flag is present: `--trial` flies the
saved design and program, `--revisions` replays the working copy and the newest revisions with one
seed, `--export-design` / `--import-design` move a schema-5 vehicle through the normal save path,
and `--aero-report` prints the effect tables. All of them reuse `omega-rules`, so the flight and
dossier rules are identical to the game's; only the rendering is skipped.
