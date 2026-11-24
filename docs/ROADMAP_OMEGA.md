# Omega — maintenance and release roadmap

## Product objective

Omega is a fictional, abstract game about completing military-themed design contracts. Its core loop is:

**contract → abstract vehicle workshop → program editor → deterministic trials → evidence dossier → delivery**

All values and outcomes are game mechanics. The project does not attempt to model real weapons, real targeting, guidance, navigation, seeker logic, warheads or real-world performance. This boundary is deliberate and is not a future feature.

## Current baseline (0.33.0)

The following features are already present in the application:

- **Research:** 18 technologies across six departments, credit costs, prerequisites, a day clock and capability grants.
- **Workshop:** attachment-tree vehicle editor, 16 part types including wings, fin sets and control surfaces, one to four derived stages, presets, editable parameters and design choices, derived game statistics, validation and persistence.
- **Program editor:** parallel threads, seven abstract block types, hardware and computer-limit validation, editing, persistence and content hashing.
- **Trials:** repeatable seeded runs, four result plots, event timeline, anomaly summaries, batch runs of 10/100/1,000 seeds, progress reporting and A/B comparison.
- **Contracts:** mission and vehicle families, research-gated offers, active-contract limits, deadlines, hash-bound evidence dossiers and delivery rewards.
- **Persistence:** versioned save slot, manifest, per-file hashes, atomic writes, backup copies and legacy campaign import.
- **Quality:** GitHub Actions checks formatting, Clippy with warnings denied and workspace tests.

The recent CI run on the current `main` commit is green. This is a repository baseline, not proof that every platform or user workflow has been manually tested.

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
