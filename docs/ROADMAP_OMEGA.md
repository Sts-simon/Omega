# Omega — maintenance and release roadmap

## Product objective

Omega is a fictional, abstract game about completing military-themed design contracts. Its core loop is:

**contract → abstract vehicle workshop → program editor → deterministic trials → evidence dossier → delivery**

All values and outcomes are game mechanics. The project does not attempt to model real weapons, real targeting, guidance, navigation, seeker logic, real warheads or real-world performance. The explosive charge is an abstract payload with a game yield. This boundary is deliberate and is not a future feature.

## Current baseline (0.45.0)

The following features are already present in the application:

- **Research:** 22 technologies across six departments, credit costs, prerequisites, a day clock and capability grants, including two explosive-charge upgrade projects.
- **Workshop:** attachment-tree vehicle editor with a searchable, category-filtered palette (favourites, recents, hover 3D preview and technology hint), an indented collapsible assembly tree (part counts, group rename/select, temporary hide and subtree lock), multiple selection with a group gizmo and `Escape`, framed camera views (`F`, `Home`, `1`–`5`, free/orbital, adjustable speed), a numeric selected-part inspector (exact local position and rotation entry, four precision steps, resets and initial-transform restore) with contextual role/consequence/compatibility help, a dedicated assembly-control panel (unconnected parts, occupied mounts, category conflicts, locks and badly connected subassemblies, each linked to its part) and a contract check against the selected active contract; 26 part types including three explosive charge grades, diameter adapters, a player-placed nose cone, wings, fin sets, control surfaces and five mission electronics modules, one to four derived stages, presets, editable parameters and design choices, derived game statistics and persistence. Fixed-section parts expose a mounting diameter in 5 cm steps, and the nose cone exposes length, base diameter and profile instead of being generated implicitly.
- **3D assembly (0.42):** automatic nearest-mount snapping with a real-size transparent ghost, green/red validity and mount-aligned preview; eight radial mounts; radial (2–8), bilateral and axial symmetry with shared parameter/transform edits, reassociation and link breaking; subtree duplication that preserves inner edges; and a versioned subassembly library (capture, insert, export, import) covering surfaces, electronics, structures, stages and configurations. The vehicle schema remains 7.
- **Program editor (0.43):** 21 blocks in eight categories, including `IF/ELSE`, `WAIT UNTIL`, `REPEAT N`, `REPEAT WHILE`, `BREAK` and `CALL ROUTINE`; a freely pannable and zoomable card canvas with connectors, multi-selection, copy/paste, duplication, auto-alignment and named groups; reusable routines with call-depth and loop guards; a coloured step debugger with pause, speed, state inspection and final flags; live validation separating errors from warnings (hardware, connections, parameters, timeouts, flags/modes never written, dead conditions, resource limits). Program layouts and routines are saved with `#[serde(default)]` fields, so older programs still load.
- **Trials (0.44):** a three-page laboratory. The dashboard combines run/batch summary, per-phase results, four trajectory plots, event timeline, statistics, contract compliance and classified anomalies with links to the failing part or program block. The comparison page shows A/B/C absolute values and B−A percentage variation for design metrics, trial results and contract requirements. The history persists every campaign in `trials/history.json` with full vehicle/program snapshots, revision, program version, seed range and profile; replay is exact and entries become stale when the design or program changes. HTML, CSV and self-contained PDF exports go to `saves/slot_N/reports/`. Seeded runs of 10/100/1,000 seeds keep flat memory and report progress.
- **Contracts and progression (0.45):** every offer carries a fictional context, mission and vehicle family, difficulty, design budget, three primary mass/reliability/complexity goals, optional secondary goals with a bonus, deadline, reward and requested dossier sections. Primary goals and budget gate delivery; secondary goals add their bonus to the quality-scaled fee. The offer card, active dossier panel and delivery list expose the goal marks, submitted revision and paid bonus. Research columns show tiers, dependency paths, real unlocked parts and a comparison strip of available technologies; revisions can be named, compared, restored (as a new revision) and identified as submitted, with their matching trial counts.
- **Delivery dossier:** a successful delivery writes a self-contained PDF to `saves/slot_N/reports/` with the vehicle overview, assembly tree, equipment, design parameters, embedded program, matching trial campaigns, anomalies, requirement matrix and submitted revision hash.
- **Contracts:** mission and vehicle families, nine mission profiles with payload, yield, discretion and apogee demands, research-gated offers, active-contract limits, deadlines, hash-bound evidence dossiers and delivery rewards.
- **Persistence:** versioned save slot, manifest, per-file hashes, atomic writes, backup copies and legacy campaign import.
- **Quality:** GitHub Actions checks formatting, Clippy with warnings denied and workspace tests.

The recent CI run on the current `main` commit is green. This is a repository baseline, not proof that every platform or user workflow has been manually tested.

### Delivered follow-up (0.39.0)

- Done: the selected-part inspector exposes local X/Y/Z translation and rotation nudges, with separate values and actions for each axis.
- Done: translation steps are 0.05 m and rotation steps are 5°; each click is one undo/redo transaction and uses the existing vehicle save schema.
- Done: regression tests cover axis independence, undo/redo and refusing independent transforms on the root part.
- Remaining: direct numeric text entry, per-field precision selection and a complete visual pass at 1280×720 and 1600×960.

## Definition of done for 1.0

1. A new player can complete the full contract loop without consulting source code.
2. Every persistent file has an explicit schema and a tested failure/migration path.
3. Core screens have consistent mouse and keyboard navigation, readable feedback and useful empty states.
4. Gameplay-critical paths have regression tests, including a complete contract from acceptance to delivery.
5. Performance claims are backed by repeatable measurements on documented hardware.
6. The repository has a current README, changelog, installation instructions, CI and a tagged release.

## Work packages

### Lot 0 — Project consistency (0.30.0)

**Purpose:** establish one accurate description of the current project.

- Align Cargo package version, lockfile, window title, README and changelog.
- Rewrite the roadmap around completed missile-gameplay systems and remaining maintenance work.
- Replace architecture and vision documentation that still describes deleted civil modules as active.
- Keep historical changelog entries and archived documents intact.

**Acceptance:** current documentation matches the source tree and the current CI workflow; no gameplay or save format changes.

### Lot 1 — Save reliability (0.30.1)

**Purpose:** make campaign data resilient to interrupted writes, corruption and future versions.

- Audit manifest validation, relative-path validation, hash verification and atomic replacement.
- Verify backup behavior and define what happens when the primary file is corrupt but a backup exists.
- Add tests for truncated manifests, unsupported schemas, missing files, hash mismatches, unsafe paths, interrupted writes and legacy import.
- Document recovery behavior clearly. Never silently overwrite a campaign that failed validation.
- Keep migrations explicit and test each supported source schema.

**Acceptance:** every failure mode has a deterministic result and a regression test; valid existing saves continue to load.

### Lot 2 — Interface and controls (0.30.2)

**Purpose:** make the game understandable without reading the README.

- Audit every navigation card, shortcut and destination for consistency.
- Ensure every screen is reachable by mouse and keyboard.
- Add contextual help, clear empty states and visible feedback for refused actions.
- Check scrolling, focus, panel bounds and readability at the supported window sizes.
- Remove references to facilities or controls that no longer exist.

**Acceptance:** no dead shortcut or misleading navigation label; the main loop can be completed using the interface alone.

### Lot 3 — Gameplay regression coverage (0.30.3)

**Purpose:** protect the complete game loop from regressions.

- Add integration-level tests for accepting a contract, editing a vehicle and program, recording trial evidence, invalidating stale evidence and delivering a valid dossier.
- Test research unlocks, active-contract capacity, deadlines and save/reload boundaries.
- Test that changes to vehicle/program inputs invalidate dependent results.
- Keep tests deterministic and avoid relying on wall-clock timing.

**Acceptance:** the critical gameplay loop is covered end-to-end and all failure paths produce stable, understandable outcomes.

### Lot 4 — Performance and resource use (0.30.4)

**Purpose:** measure before optimizing.

- Record startup and campaign-load times, idle frame rate, workshop rebuild cost and batch-trial duration.
- Measure memory use during long sessions and large batches.
- Verify that long-running batch work does not block the render/update loop.
- Document hardware, build profile, test scenario and measurement method.
- Optimize only measured bottlenecks.

**Acceptance:** a reproducible benchmark report exists; no performance target is claimed without a measurement.

### Lot 5 — Distribution and release candidate (0.31.0)

**Purpose:** make the game straightforward to install and test.

- Verify clean builds on supported Linux environments.
- Provide concise dependency and launch instructions.
- Add release packaging and versioned release notes.
- Test a clean first launch, save creation, restart and upgrade from the previous supported save format.
- Run a final manual play-through.

**Acceptance:** a tester can install, launch, play, save and update using only the release instructions.

### Lot 6 — Workshop assembly graph (0.31.1, in progress)

**Purpose:** replace the flat stage list with an attachment tree so radial boosters, free placement
and a real build interaction become possible.

- Done: `omega-rules::assembly` (nodes, attachments, symmetry slots, derived stages, validation,
  diameter warnings, save schema 3 with migration), 3D layout from the tree, workshop UI showing
  derived stages, part selection by id and palette additions above the selected part.
- Delivered: 3D picking (`src/picking.rs`: cylinder raycast, nearest part, 28 px node snapping), a
  drag ghost and node markers (`src/workshop.rs`), palette-to-node drops, subtree moves with a 5 px
  drag threshold so a click only selects, `Delete`, linked radial symmetry (`X`, 1 → 2 → 3 → 4),
  undo/redo (50 snapshots, buttons and layout-independent `Ctrl+Z` / `Ctrl+Y`) and the wheel
  adjusting the selected part's main value without zooming the camera.
- Remaining for this lot: live flight forecast and the active-contract requirement bar.

**Acceptance:** a new player builds a two-stage rocket with two radial boosters with the mouse only;
old presets keep identical stage figures; every refusal has a message and a test.

### Lot 7 — Aerodynamics: wings, fins and control surfaces (0.32.0, delivered)

**Purpose:** make the airframe shape matter. Cruise vehicles get wings that let the air-breathing
engine work at a lower speed; ballistic vehicles get fins for stability and control surfaces for
authority, all placed on the same radial nodes and mirrored with `X`.

- Done: one `omega-rules::aero` aggregation (areas, sweep, stability, control authority, lift, drag,
  complexity, reliability, mass, CG/CA) consumed by the existing 1-D flight integration; no guidance
  law, no CFD, no per-step geometry.
- Done: `Wing`, `FinSet` and `ControlSurface` specs with planform, profile, stabiliser role, fin
  layout, control family and light build; `designs/vehicle.json` schema 4 with migration from schema
  3; a design without surfaces keeps its previous flight figures.
- Done: aero palette row, plate 3D meshes, oriented-box picking in `src/picking.rs`, span/chord/sweep
  and design choices in the inspector, leaf-only attachment, CG/CA markers and stable band.
- Done: five aerodynamics technologies, a soft stability requirement on surface-to-air offers, and
  flight tests for fin damping, wing lift and control authority.
- Full plan, accepted values and delivery record: `docs/AERODYNAMICS_PLAN.md`.

**Acceptance:** fins measurably improve stability and altitude settling, wings enable a cruise design
that the wingless version cannot complete, control surfaces reduce the time to the target, old
designs fly unchanged, schema 3 migrates, and the whole lot is checked in game at 1600×960.

### Post-aerodynamics follow-ups (0.33.0, delivered)

**Purpose:** make the aerodynamics lot pay, and make it verifiable without a display.

- Done: one seeded surface fault for fragile airframes (the reliability sink), dossier fees that
  scale with the score, per-family aero requirements (`LIFT`, `AUTHORITY`, `STABILITY`), and a
  research pacing audit with durations reduced from 90 to 67 serial days.
- Done: the design-choice effect tables moved to `aero_effects.json` with a generated
  `docs/AERO_BALANCE.md`; per-surface contributions and part-linked warnings in the workshop.
- Done: a screenless harness (`--trial`, `--revisions`, `--export-design`, `--import-design`,
  `--aero-report`), a one-press `RE-FLY` for stale contracts, and `FINS` / `WINGS` presets.
- Full record: `docs/CHANGELOG.md`; the numbers accepted for the aero tables are in
  `docs/AERO_BALANCE.md`.

### Stable release (1.0.0)

Publish only after all work packages pass, CI is green and the full play-through has been manually verified.

## Explicitly out of scope

- Realistic weapon engineering, real targeting or guidance, navigation laws, seeker logic or warhead modelling.
- Manufacturing, factories, inventories, staffing or a complex economic simulation.
- Multiplayer or networking.
- Large-scale island expansion.
- New simulation complexity that does not improve the abstract game loop.

## Working rules

- Each lot is developed on its own branch and submitted as a pull request.
- A lot is not considered complete until its acceptance criteria and CI checks pass.
- Do not combine unrelated gameplay changes with maintenance work.
- Preserve save compatibility unless a deliberate, tested migration is part of the lot.
- Update this roadmap and the changelog when scope or status changes.
- Each lot lands on `main` with a merge commit (`--no-ff`) and its own package version bump.
