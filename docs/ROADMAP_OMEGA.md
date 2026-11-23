# Omega — improvement plan

## The goal in one sentence

Make Omega a game where **a full play-through rests on missile design alone**: the player accepts a
contract, assembles a staged vehicle, writes the onboard program, runs reproducible trials, builds a
dossier of evidence and delivers a plan — without ever touching the civil assembly bench inherited
from 0.6.

Today that sentence is false in three places: the missile program cannot be edited in the game, there
is no trial screen, and the only complete contracts are civil ones that run on the legacy bench.
Everything below exists to make the sentence true, then to make it pleasant and reliable.

## Starting point (0.26.0, verified)

- **Research** — playable: 13 technologies in five departments, one project at a time, credits
  charged when research starts, NEXT DAY button, research state and day saved.
- **Missile workshop** — playable: 13 part types, 1 to 4 stages, ROCKET/CRUISE/EMPTY presets, `-`/`+`
  editing, parts gated by research, saved to `designs/vehicle.json`.
- **Missile rules (`omega-rules`)** — complete and tested but invisible in the game: `vehicle`,
  `program` (7 blocks), `flight` (headless trials), `research`.
- **Contracts** — civil only: they use neither the missile vehicle, nor the program, nor trials.
- **Onboard program** — the Programming Studio still runs the 0.6 chamber sequencer.
- **Trials** — no screen; `flight` is exercised by tests only.
- **Research → contracts** — the `family:*` and `ExtraActiveProjects` grants are never consulted;
  the two-project limit is hard-coded.
- **Quality** — 253 passing tests, 69 Bevy deprecation warnings, no CI, no tagged release.

## Definition of done

The project is finished when all five criteria hold, each verifiable by a test or a manual pass:

1. **Complete loop** — a missile contract is carried out (accept → design → program → trial → dossier
   → delivery) without opening the legacy bench.
2. **No dead civil code** — the legacy bench, polygon sketch, instrument profile, chamber sequencer
   and civil contracts are gone from the repository.
3. **Bound evidence** — every delivery dossier references the vehicle, program and trial hashes;
   changing any of them invalidates the evidence.
4. **Research that restricts** — no contract family is offered without its technology; active-project
   grants and Monte Carlo are actually consumed.
5. **Presentable repository** — green CI on `main`, clean `clippy -D warnings`, a tagged `1.0.0`
   release with notes, and a README that matches what the game does.

## Work packages

Each package lists a **purpose**, the **work**, and an **acceptance criterion**. They are ordered:
each one unblocks the next. Version numbers follow the existing `CHANGELOG` (0.26 is current).

### Lot 0 — Repository foundations (0.26.x)

**Purpose.** Make the public repository credible and verifiable before adding more game.

**Work.**

- GitHub Actions on `ubuntu-latest`: Bevy system packages, Cargo cache, then
  `cargo fmt --all -- --check`, `cargo check --workspace --all-targets` and `cargo test --workspace`.
- Clear the Bevy deprecations and the dead code, then add
  `cargo clippy --workspace --all-targets -- -D warnings` to CI.
- Tag `v0.26.0` with notes drawn from the `CHANGELOG`, and add CI and version badges to the README.
- Move `CAMPUS.md`, `ROADMAP.md`, `UX_CONTROLS.md` and `SEQUENCE_EDITOR.md` to `docs/archive/`
  behind an "archived" header.

**Acceptance.** Green badge on `main` in under ten minutes, zero `clippy -D warnings` output, a
visible public release, and a README that no longer presents stale documents as current.

### Lot 1 — Missile program editor (0.27)

**Purpose.** Supply the missing half of design: after the vehicle, the behaviour. This is the highest
leverage lot, because the rules are already written and tested but unreachable.

**Work.**

- New `src/program.rs` editing session behind the **PROGRAMMING** card, with the seven
  `omega-rules::program` blocks as the only source of truth: `wait`, `ignite`, `separate`,
  `throttle`, `hold speed`, `hold pressure`, `lock target`.
- Palette **derived from the hardware actually installed** in the vehicle, with live thread and
  memory meters for the onboard computer (which always lives in the last stage).
- Per-thread editing: add, remove, reorder; validation on every change; error messages naming the
  offending block and the missing resource.
- Persist the program in the slot (`programs/missile.json`) through `SaveSlot`, with a content hash
  ready for evidence.
- Demote the 0.6 chamber sequencer to an explicitly labelled legacy tab instead of occupying the
  "Programming Studio" name.

**Acceptance.** A valid program for a two-stage vehicle can be built in under two minutes without
leaving the interface; an invalid program explains why; save and reload work; at least 8 tests cover
the editing logic.

**Status.** Done in the working tree: `program_editor.rs` behind the PROGRAM tab (`F4`), palette
driven by the installed hardware, live thread and memory meters, per-block tuning and reordering,
validation naming the thread and block, `programs/missile.json` persistence, content hash, and the
0.6 sequencer demoted to a legacy SEQUENCER tab. Covered by 8 unit tests and verified on screen
(palette gating, meters, validation, tuning, reordering, persistence and reload). The TRIALS screen
(lot 2) now consumes the program; contracts consume it in lot 3.

### Lot 2 — Trial screen (0.28)

**Purpose.** Give the design a visible consequence. `omega-rules::flight` already produces complete
results that nobody can see.

**Work.**

- **TRIALS** screen: choose the contract and vehicle, show and edit the seed, `RUN` button, four
  plots (speed, altitude, mass, thrust), event timeline and anomalies named in plain language.
- Monte Carlo batch mode: 10 / 100 / 1,000 seeds, outcome distribution (failure, separation, lock not
  acquired), locked behind the existing `analysis:monte_carlo` grant.
- A/B comparison of two designs over the same seed set, with metric deltas.
- Visible determinism: same design, program and seed ⇒ same result, with the input hash shown.

**Acceptance.** A two-stage trial shows the four plots; replaying a seed reproduces a result
identical to a `omega-rules::flight` test; the batch refuses to start without the grant.

**Status.** Done in the working tree: `trials.rs` behind a TRIALS tab (navigation bar and island
card), seed stepper, vertical or air-launch profile, `RUN`, four plots, event timeline,
plain-language anomalies, input hash with a stale marker, batches of 10 / 100 / 1,000 gated on
`analysis:monte_carlo`, and A/B comparison over the same ten seeds with deltas. Covered by 8 unit
tests and verified on screen with a two-stage solid rocket: the four plots and the full event
timeline render, an empty program reports "no stage was ever ignited", the batch and A/B refuse
without the grant, and igniting the second stage moves the A/B speed delta to +425 m/s on the same
seeds. Vehicle/contract selection arrives with lot 3.

Batches fly on a worker thread: a 1,000-seed batch over a 546 s flight blocked the frame for ~290 ms
and left a 31 MB peak when it kept every report; it now runs off the frame with a live progress line
and folds each report into running totals, so the peak stays flat. A finished batch also remembers
the input hash it was started with: stepping the seed, switching the profile or editing the vehicle
or program afterwards marks its result `STALE` on the batch panel until it is flown again.

The lock stays one seeded probability roll, which the setup panel predicts (`ROLL 0.11` vs the
radar's 55 % reads `LOCK`), and a batch of 1,000 seeds reproduces that probability (549/1,000). A
seed also spreads the hardware: ±3 % engine thrust and solid-motor burn time, ±5 % drag. The spread is
symmetric, deterministic and published in the setup panel, so batch means still describe the design
while the distribution above the lock (paths, top speeds, lift-off failures) now varies. No guidance,
tracking or targeting is involved.

### Lot 3 — Missile contracts, evidence, plan delivery (0.29)

**Purpose.** Close the loop. This is the lot that turns the demo into a game and makes the
"missile design alone" sentence true.

**Work.**

- Missile contract families that reference a `Vehicle` as the current design, instead of the
  assembly + instrument-profile pair.
- Abstract missile requirements (range, accuracy, reliability, cost, deadline) evaluated with the
  existing `evaluate_requirement` / `build_dossier` machinery.
- Bound evidence: vehicle hash **and** program hash **and** trial hashes; any later change marks the
  dossier `STALE`.
- The missile design joins the design revisions (`designs/revisions.json`) and therefore the existing
  diff.
- Delivery: payment and a completed-contract counter, nothing more (no factory, no inventory).

**Acceptance.** A complete loop is carried out without opening the legacy bench once; changing a
part after a trial marks the dossier `STALE`; delivery is credited.

**Status.** Done in the working tree: `src/procurement.rs` behind the CONTRACTS tab. The office
offers four missile families and vehicle families; accepting one starts the evidence clock. Trials
flown afterwards are recorded automatically, bound to the vehicle hash, the program hash and the
seed. `ContractOffer::missile_requirements` turns family and difficulty into flight requirements
(range, speed, mass, lift-off, separation and a hard lock for air-to-air/surface-to-air) and
`ContractOffer::missile_dossier` evaluates them through `evaluate_requirement` / `build_dossier`.
The dossier panel shows every row and the bound hashes; editing a part or the program marks all
rows `STALE`, sets the score to 0 and disables delivery, and reverting restores it. Delivery pays
the reward, increments the delivered counter and generates a new board; a passed deadline refuses
delivery. Missile designs join `designs/revisions.json` (schema 2, legacy schema 1 still loads) with
a head hash, a change summary and a record button in the workshop. Verified in game on a two-stage
solid rocket against a ballistic offer: accept, fly, dossier 100 %, add a pitot → `STALE` and the
button disabled, remove it → current again, deliver → +8,000 cr and one completed contract
(22,500 → 30,500 cr). The civil office and bench are unchanged but reachable only through an
explicit `CIVIL (LEGACY)` toggle, ready for lot 5.

### Lot 4 — Research that actually constrains

**Purpose.** The research grants exist in the data but are consulted nowhere: progression is
decorative.

**Work.**

- Filter offer generation by `family:*` capabilities: an unresearched family offers no contracts.
- Honour `ExtraActiveProjects` instead of the hard-coded `2`.
- Consume `analysis:monte_carlo` (lot 2).

**Acceptance.** Without a family technology, zero offers from that family; with two grants, three
active projects are possible; each rule is covered by a test.

### Lot 5 — Remove the inherited civil loop

**Purpose.** Delete what no longer has a reason to exist. As long as the legacy bench is the only
complete path, the README cannot tell the truth.

**Work.**

- Remove from the game: legacy assembly bench, polygon sketch, instrument profile, chamber sequencer,
  civil contracts and their jobs.
- Keep `sim-core` as a standalone library (thermal, truss, meshing) and decide explicitly whether the
  missile game still uses it; otherwise take it off the critical path.
- Slot v3: legacy files are no longer read and the migration ignores them cleanly.

**Acceptance.** No code reference to the civil loop remains; all tests pass; a 0.26 save loads without
error; the README mentions no inherited tool.

### Lot 6 — Interface and controls

**Purpose.** The game must be usable without reading the README.

**Work.**

- Fix the known inconsistency: the island card for "MISSILE WORKSHOP" advertises `F2`, but `F2`
  opens the legacy bench.
- Consistent per-view shortcuts, contextual help, explicit empty states, full keyboard navigation,
  checked contrast and font sizes.
- Visible feedback for every refused action (credits, research, validation).

**Acceptance.** No card advertises a wrong key; every screen is reachable by keyboard and mouse; no
action happens without feedback.

### Lot 7 — Data robustness

**Purpose.** A public game receives saves from different versions.

**Work.**

- Explicit schema version and migrations tested from 0.26 to the current version.
- A corrupted file: clear message, `.bak` preserved, nothing lost.
- Drop the pre-0.22 import once 0.29 ships.

**Acceptance.** Automated migration tests; a corrupted-file scenario is covered.

### Lot 8 — Performance and finish

**Purpose.** Measure rather than assume.

**Work.**

- A 60 fps budget on integrated graphics, load times, Monte Carlo batch cost, worker-thread sizing.
- Before/after measurements recorded in `docs/`.

**Acceptance.** A table of measured numbers, not an assertion.

## Out of scope (deliberate)

These are excluded so the goal stays attainable:

- Real or certified physical models, experimental validation, serious CFD/FEA.
- Guidance, tracking, navigation law, seeker, warhead: the lock remains a seeded probability roll.
- Multiplayer, networking, a complex economy, staff, inventory, island expansion.
- Audio, translations, dark mode: not a priority while the missile loop is incomplete.

## Milestones

| Version | Objective | Exit state |
|---|---|---|
| 0.26.x | Foundations | Green CI, clean clippy |
| 0.27 | Editable program | Vehicle and program designed in-game |
| 0.28 | Trials | Trials visible and replayable |
| 0.29 | Missile contracts | Complete loop, legacy loop removed |
| 1.0 | Finish | Measured performance, tagged release |

## Risks

- **Removing the legacy loop regresses the game** → remove only in 0.29, after the missile loop
  passes end-to-end tests.
- **Bevy pinned at `=0.20.1` with deprecations** → handle the warnings in lot 0, before piling on
  interface code.
- **Scope drift** (factories, economy, graphics) → the out-of-scope list is authoritative; any
  addition must serve one of the five criteria of "done".
- **Balancing abstract values** → accept the abstraction, do not chase realism, tune through rule
  tests.

## Success measure

The goal is reached when someone who clones the repository can, without reading the code: launch the
game, accept a missile contract, assemble a vehicle, write its program, run a trial, deliver the plan,
and see that CI is green and a release is published. None of the five criteria of "done" depends on a
tool inherited from 0.6.
