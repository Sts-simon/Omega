# Omega — target architecture and subsystem roadmap

Status: **plan**. This lot adds no code, no save change and no gameplay. It records what the
repository already is, where the nine target systems belong, which interfaces connect them, and in
which order to build the missing ones.

Companion documents:

- `docs/ARCHITECTURE.md` — the as-built architecture (what exists today).
- `docs/ROADMAP_OMEGA.md` — maintenance and release lots (consistency, saves, interface, release).
- `docs/VISION_MISSILES.md` — the product boundary.

Everything below stays inside that boundary: fictional, abstract gameplay values. No guidance,
tracking, seeker or warhead model, no real structural analysis, no production or industrial
economy.

---

## 1. The loop the architecture has to serve

```
research unlocks ──▶ workshop builds ──▶ program is written ──▶ trial proves ──▶ dossier is delivered
        ▲                                                                                    │
        └──────────────────────── credits and reputation ────────────────────────────────────┘
```

Every subsystem answers one of four questions, and only those:

| Question | Answered by |
|---|---|
| What is the vehicle made of? | Workshop, Structure, Propulsion, Avionics |
| What can it do? | Aerodynamics, Propulsion, Avionics |
| What does it attempt? | Programming |
| What has actually been proven? | Trials, evidence, dossier |
| What does that earn? | Campaign, Research |

A subsystem that cannot point at one of these questions is out of scope.

---

## 2. Audit of the current architecture

### 2.1 Layers as built

| Layer | Crate / module | Bevy | Owns |
|---|---|---|---|
| Rules | `omega-rules` (`assembly`, `vehicle`, `aero`, `program`, `flight`, `contracts`, `requirement`, `dossier`, `research`, `metrics`, `diff`) | no | every persistent model and every deterministic figure |
| Data | `omega-data` (`hash`, `atomic`, `slot`) | no | content hashes, atomic writes, versioned slot, manifest |
| Application | `omega` (`main`, `campus`, `campus/board`, `procurement`, `missile`, `workshop`, `picking`, `program_editor`, `trials`, `revisions`, `interface`, `save`, `headless`, `scene`, `ui_kit`) | yes | presentation, input, sessions, wiring, save paths |
| Standalone | `omega-sim-core` (thermal FV, truss, mesh) | no | scientific library, **not linked to the game** |

Two properties are already true and must stay true:

1. **Rules never import Bevy or the application.** `omega-rules` depends on `omega-data` only.
2. **One owner per rule.** The flight integration is in `flight.rs`, part coefficients in
   `vehicle.rs`, aerodynamic aggregation in `aero.rs`, contract rules in `contracts.rs`. UI code
   calls these; it does not reimplement them (`ARCHITECTURE.md`, "Change boundaries").

### 2.2 Systems already implemented

| Target system | Today | Where |
|---|---|---|
| Workshop | attachment-tree editor, 16 part types, drag/drop, symmetry, undo/redo, picking, presets, validation | `assembly.rs`, `missile.rs`, `workshop.rs`, `picking.rs` |
| Aerodynamics | one aggregation per vehicle (areas, stability, authority, lift, drag, complexity, reliability, mass, CG/CA), data-driven {planform, profile, role, layout, control} tables | `aero.rs`, `aero_effects.json` |
| Propulsion | part coefficients (solid motor, tank, liquid engine, air-breathing engine), stage aggregation (wet/ignition mass, thrust, burn time, TWR, delta-v), seeded manufacturing spread | `vehicle.rs`, `assembly.rs::stage_reports`, `flight.rs` |
| Programming | seven abstract blocks, parallel threads, computer memory/thread limits, hardware gating, validation, editor session | `program.rs`, `program_editor.rs` |
| Trials | seeded 1-D flight, events, anomalies, four plots, batches of 10/100/1 000, A/B comparison, evidence recording | `flight.rs`, `trials.rs`, `procurement.rs` |
| Campaign | offers per mission family, research-gated board, two active slots, deadlines, hash-bound evidence, scored dossier, fee scaling | `contracts.rs`, `dossier.rs`, `procurement.rs`, `campus.rs` |
| Research | 18 technologies / 6 departments, credits, durations, prerequisites, capability grants, one project at a time, day clock | `research.rs`, `campus.rs`, `campus/board.rs` |

### 2.3 Systems partially implemented

| System | Present | Missing |
|---|---|---|
| Avionics | sensors (pitot, barometer, basic/active radar), three computers with threads/memory, radar lock probability, hardware→block mapping | a single avionics figure set (fit, bands, redundancy), sensor behaviour at speed/altitude, any avionics-specific contract evidence |
| Structure | part geometry, derived mass/length/CG, diameter-step warning, aerodynamic load tolerance through figures | sections and structural elements, a stiffness/load-margin score, structural evidence, any workshop control |
| Campaign | credits, day, deliveries, two active contracts, fee by score | deadline failure, reputation/rating, any contract pressure beyond "refused delivery" |
| Programming | block list, threads, memory | data-driven block registry (adding a block still touches Rust and UI), visible program score for contracts |
| Research | tree hard-coded in `research.rs` | tree as data (like `aero_effects.json`), so a new subsystem's technologies are not a code change |

### 2.4 Systems absent

- **Structure as a design surface.** No section or element parts, no structural figures, no consumer.
- **Subsystem trial report.** `FlightReport` carries one `aero` summary; structure/avionics/
  propulsion are not reported or evidenced separately.
- **One design-figures façade.** Each consumer recomputes what it needs; see §4.1.
- **Program file versioning.** `programs/missile.json` is a bare `Program` with no schema field
  (§7.3).

### 2.5 The seven categories are two axes, not seven siblings

The brief lists air-air, air-surface, surface-air, surface-surface, ballistic, cruise, short-range.
The code already models them as **4 mission families × 3 vehicle families**:

| Axis | Values | Type |
|---|---|---|
| Mission | air-to-air, air-to-surface, surface-to-air, surface-to-surface | `MissionFamily` (4) |
| Vehicle | ballistic, cruise, short-range | `VehicleFamily` (3) |

`family:anti_ship` is a **difficulty gate** on surface-to-surface offers (`difficulty >= 4`), not a
fifth mission family, and `family:cruise` is the vehicle-family capability. Every offer is one cell
of the 4 × 3 matrix plus a difficulty 1–5.

**Rule:** a future lot adds a category only by adding a value to one of these two enums, with a
capability, an offer generator branch, requirements and tests. Creating a parallel "category" enum
would duplicate the contract model.

---

## 3. Target systems and ownership

Each system gets one rules module, one figure type, one place that persists it (if any), and a
named set of consumers. "No new simulation" means the system produces abstract scores and static
tables, never per-step physics.

| System | Owner module (rules) | Output type | Persisted | Consumers | Status |
|---|---|---|---|---|---|
| Workshop | `assembly`, `vehicle` | `Vehicle` | `designs/vehicle.json` | all figure modules, trial, dossier | done |
| Structure | `structure` (new) | `StructureFigures` | inside `Vehicle` | flight, dossier, workshop | absent |
| Aerodynamics | `aero` | `AeroFigures` | inside `Vehicle` (part choices) | flight, dossier, workshop | done |
| Propulsion | `propulsion` (new, extracted) | `PropulsionFigures` | inside `Vehicle` | flight, dossier, workshop | present, unowned |
| Avionics | `avionics` (new, extracted) | `AvionicsFigures` | inside `Vehicle` | program validation, flight lock, dossier | present, unowned |
| Programming | `program` | `Program`, validation issues | `programs/missile.json` | flight, trial, dossier | done |
| Trials | `flight` | `FlightReport` | evidence in contracts | contracts, campaign, UI | done |
| Campaign | `contracts`, `dossier` | offers, evidence, dossier | `contracts/missile.json`, `campaign.json` | UI, research gate | partial |
| Research | `research` | tree, state, grants | `campaign.json` | workshop, contracts, campaign | done |

### 3.1 Workshop

Owns the attachment tree and part specs. It is the only writer of `Vehicle`. Everything else reads
it. No subsystem may add a parallel "component list".

### 3.2 Structure (target)

Abstract sections and elements, not FEA:

- Leaf parts mounted on the body: bulkhead, frame, stringer — each with a material/layout choice
  and a mass. Later (not in the first lot) a body *section* that groups them.
- `StructureFigures { mass_kg, stiffness, load_margin, section_count, warnings }`, computed once
  from the tree like `AeroFigures`.
- One flight consumer: the peak dynamic pressure already derivable from the flight samples is
  compared with the design's abstract `load_margin`; exceeding it records **one** seeded structural
  anomaly. A design without structural parts keeps the current behaviour.
- No truss solver, no buckling, no fatigue. `omega-sim-core` stays unlinked.

### 3.3 Aerodynamics

Already complete for this architecture. Future lots may add tables and requirements, never a
second aggregation.

### 3.4 Propulsion (target module)

Today the coefficients live in `vehicle.rs`, the stage aggregation in `assembly.rs` and the burn
logic in `flight.rs`. Target: move the formulas and the stage aggregation into
`omega-rules::propulsion` with:

```rust
pub struct PropulsionFigures {
    pub stages: Vec<StageReport>,     // re-used, unchanged shape
    pub total_wet_mass_kg: f64,
    pub total_propellant_kg: f64,
    pub air_breathing: bool,
}
pub fn figures(vehicle: &Vehicle) -> PropulsionFigures;
```

Pure refactor: identical numbers, identical save schema, identical flight report.

### 3.5 Avionics (target module)

```rust
pub struct AvionicsFigures {
    pub computer: Option<ComputerClass>,
    pub radar: Option<RadarSpec>,
    pub sensors: SensorFit,          // pitot / barometer / radar classes fitted
    pub blocks: BTreeSet<&'static str>,
    pub lock_probability: Option<f64>,
}
pub fn figures(vehicle: &Vehicle) -> AvionicsFigures;
```

Consumers: `program::validate_program` (threads, memory, hardware), `flight` (lock roll, radar
survival), workshop panel. Again a refactor first, new behaviour only in a later lot.

### 3.6 Programming

Keep threads and blocks. Growth happens through a **block registry** (key, label, hardware need,
memory cost, editable parameter) so a new block is data plus one enum variant, and the editor
builds its palette from the registry rather than a hard-coded match.

### 3.7 Trials

`FlightReport` grows one summary per subsystem, produced by the same run:

```rust
pub struct FlightReport {
    pub samples: Vec<Sample>,
    pub events: Vec<(f64, String)>,
    pub anomalies: Vec<String>,
    pub aero: AeroSummary,
    pub propulsion: PropulsionSummary,   // new
    pub structure: Option<StructureSummary>, // new
    pub avionics: AvionicsSummary,       // new
    // existing scalars unchanged
}
```

New fields are additive; the CLI report and dossier evidence read them from one place.

### 3.8 Campaign

Keep offers, evidence, dossier and delivery in `contracts`/`dossier`/`procurement`. Add only
abstract contract pressure (deadline outcome, a contract rating). No factories, inventory,
staffing or island economy.

### 3.9 Research

Move the tree to `crates/omega-rules/src/research_tree.json` (embedded, validated like
`aero_effects.json`) before adding structure technologies, so a subsystem lot edits data, not
rules code. Grants stay capability strings; `Grant::ExtraActiveProjects` stays the only
non-capability grant.

---

## 4. Interfaces between modules

These are the seams future lots implement. They are deliberately **additive**: existing tests,
the CLI and save files keep working while each seam lands.

### 4.1 One design-figures façade

```rust
// omega-rules/src/design.rs (target)
pub struct DesignFigures {
    pub aero: AeroFigures,
    pub propulsion: PropulsionFigures,
    pub structure: StructureFigures,
    pub avionics: AvionicsFigures,
}

pub fn figures(vehicle: &Vehicle) -> DesignFigures;
```

Rules:

- Computed **once per edit or run**, never per flight step.
- Each member is owned by its module; the façade only calls them in a fixed order.
- A module that needs another subsystem's figure receives it as an argument; it does not
  recompute it. This is what keeps `flight` from growing four copies of "what the design is".

### 4.2 Flight consumes figures

```rust
// unchanged, kept as the single-argument wrapper used by tests and the CLI
pub fn simulate(vehicle: &Vehicle, program: &Program, config: FlightConfig)
    -> Result<FlightReport, Vec<String>>;

// new entry point; `simulate` becomes a thin wrapper over it
pub fn simulate_with(
    vehicle: &Vehicle,
    program: &Program,
    figures: &DesignFigures,
    config: FlightConfig,
) -> Result<FlightReport, Vec<String>>;
```

Invariant: `simulate(v, p, c)` must equal `simulate_with(v, p, &design::figures(v), c)` for every
design. A test pins it.

### 4.3 Capabilities

```rust
impl PartSpec {
    /// Everything research must grant before this part (or this build) can be placed.
    pub fn capabilities(&self) -> Vec<&'static str>;
}
```

Replaces the current pair `required_capability()` + `extra_capabilities()`. The workshop palette
uses the first entry for the locked label, the save/import path uses the whole list.

### 4.4 Evidence and requirements

`MissileTrial` is the evidence carrier; each subsystem adds one optional block with
`#[serde(default)]`, and `ContractOffer::missile_requirements()` keeps one requirement per metric
with `VerificationMethod::Test`.

```rust
pub struct MissileTrial {
    // existing fields…
    pub structure: Option<StructureTrialFigures>,
    pub avionics: Option<AvionicsTrialFigures>,
}
```

Rules:

- Proof ids stay stable strings (`RANGE`, `SPEED`, `MASS`, `LIFTOFF`, `SEPARATION`, `LOCK`,
  `STABILITY`, `LIFT`, `AUTHORITY`, plus future `STRUCTURE_MARGIN`, `SENSOR_FIT`).
- Every proof is bound to the vehicle hash and the program hash; a design edit makes it stale.
- Adding a proof never tightens an existing contract unless the offer's requirements say so.

### 4.5 Sessions and UI actions

The application keeps one session resource per editable system (`MissileSession`,
`MissileProgramSession`, `TrialSession`, `Procurement`, `RevisionLog`) exposing:

```rust
fn apply(&mut self, action: XAction, owned: &BTreeSet<String>) -> String;
```

- Returns the message to show; empty means "nothing to say".
- Never saves directly: the board calls `persist_*` after the action, so one path owns writes.
- Rendering systems (`sync_*`, `draw_*`) never mutate rules state.

A new subsystem adds an action variant to `UiAction`, a match arm, and a board panel — never a
second input loop.

### 4.6 Persistence

| File | Model | Schema today | Bump rule |
|---|---|---|---|
| `saves/slot_1/manifest.json` | `Manifest` | 2 | slot layout changes only |
| `campaign.json` | credits, research, day | 2 | campaign fields |
| `contracts/missile.json` | offers, active, completed, evidence | 2 | offer/evidence fields |
| `designs/vehicle.json` | `Vehicle` | 4 | new part variants or attachment data |
| `designs/revisions.json` | revision log | 3 | revision record shape |
| `programs/missile.json` | `Program` | **none** | needs a version wrapper (risk §7.3) |

Migration rules already in force: read older schemas, refuse newer, rewrite on next save; never
reinterpret silently; every persisted struct is round-tripped by a test.

### 4.7 Direction of dependencies

```
omega-data ──▶ omega-rules ──▶ omega (Bevy)
     ▲              │
     └──────────────┘  (hash/slot types only)
```

- No rule module imports another *application* module or Bevy.
- A subsystem module may import `assembly`/`vehicle` (the design) and `omega-data` (hashes), never
  `contracts` or `flight`; `contracts`/`flight` import the figure modules, not the reverse.
- The application imports every rule; rules import none of it.
- `omega-sim-core` stays out of the game's dependency graph.

---

## 5. Delivery roadmap

Nine independent lots, in dependency order. Each is small enough for one PR, testable headlessly,
and leaves a green workspace. A lot may be delayed without blocking the others below its
dependency.

| # | Lot | Depends on | Touches | New save schema |
|---|---|---|---|---|
| A0 | Target architecture and roadmap | — | docs only | no |
| R1 | Research tree as data | — | `research.rs`, `research_tree.json` | no |
| P0 | Version the program file | — | `program.rs`, `program_editor.rs` | program schema 1 |
| P1 | Propulsion module (refactor) | — | `propulsion.rs` (new), `vehicle.rs`, `assembly.rs`, `flight.rs` | no |
| V1 | Avionics module (refactor) | — | `avionics.rs` (new), `program.rs`, `flight.rs` | no |
| S1 | Structure elements and figures | R1 | `structure.rs` (new), `vehicle.rs`, `flight.rs` | vehicle 5 |
| T1 | Subsystem trial report and evidence | P1, V1, S1 | `flight.rs`, `contracts.rs`, `headless.rs` | no (additive `MissileTrial` fields) |
| W1 | Workshop structure palette | S1, T1 | `missile.rs`, `campus/board.rs`, `workshop.rs` | no |
| S2 | Structural contract requirements | S1, T1 | `contracts.rs`, research data | contracts 3 |
| C1 | Contract pressure and rating | S2 | `procurement.rs`, `dossier.rs` | campaign 3 / contracts 4 |

### A0 — Target architecture and roadmap (this lot)

**Deliverable:** this document, the roadmap pointer, the changelog entry.

**Acceptance:** every claim matches the source tree; no code, save or behaviour change; the
existing workspace checks stay green; no new dependency.

### R1 — Research tree as data

**Deliverable:** `research_tree.json` embedded with `include_str!`, parsed once, validated against
the `Grant`/`Department` enums; `default_tree()` reads it.

**Acceptance:** the 18 technologies (ids, costs, 63 000 cr, 67 days, prerequisites, grants) are
identical to today, pinned by the existing budget test plus a JSON round-trip test; a missing key
is a hard error (same rule as `aero_effects.json`); no save change.

**Risk:** ordering/iteration of grants (use `BTreeSet`/sorted keys, never `HashMap` order in a
score).

### P0 — Version the program file

**Deliverable:** `Program` gains a versioned wrapper (`schema_version`, defaulted on read for the
current bare form), so a future block addition is refused by version instead of by an opaque serde
error.

**Acceptance:** an existing bare `{"threads": [...]}` loads, is rewritten with the version on the
next edit, and a newer schema is refused with a readable message; the program editor round-trip
test stays green.

### P1 — Propulsion module

**Deliverable:** formulas and stage aggregation moved to `omega-rules::propulsion`; `vehicle.rs`
keeps part specs and delegates `stats()`; `assembly::stage_reports` delegates.

**Acceptance:** stage reports are **bit-identical** for every test fixture; two recorded designs
produce an identical `--trial` JSON (golden file comparison, allowing only the version string);
`--aero-report` unchanged; no schema change; one new test that `stats()` and `figures()` agree.

**Risk:** floating-point reassociation — do not "simplify" formulas in this lot.

### V1 — Avionics module

**Deliverable:** `AvionicsFigures` + `figures()`; `program::validate_program` and the flight lock
read it.

**Acceptance:** every validation message string is unchanged (tests pin them); lock probability and
seed roll unchanged for both radar classes; a design without a computer still fails with the same
message; no schema change.

### S1 — Structure elements and figures

**Deliverable:** three structural leaf parts (bulkhead, frame, stringer) with one material choice
each, `StructureFigures { mass_kg, stiffness, load_margin, warnings }`, `vehicle.json` schema 5
(additive), flight term: peak dynamic pressure above `load_margin` records one seeded structural
anomaly and caps the path.

**Acceptance:** schema 4 loads and rewrites as 5; a design without structural parts has a
byte-identical flight report to today (golden fixture); every score is finite and bounded under
extreme spans/materials; the anomaly is a warning-level outcome, never a refusal; the workshop
shows a missing-research message, not an internal id; no change to aero figures.

**Risk:** the flight term must not change existing designs — gate it on `section_count > 0` and
pin it with a test.

### T1 — Subsystem trial report and evidence

**Deliverable:** `FlightReport` carries propulsion/structure/avionics summaries; `MissileTrial`
carries the structural and avionics figures (serde-defaulted); `--trial` prints a `subsystems`
block; evidence gains `STRUCTURE_MARGIN`/`SENSOR_FIT` only when a requirement asks for it.

**Acceptance:** old contract files still load (missing fields default); an old `MissileTrial` never
gains a proof silently; stale-evidence behaviour unchanged (edit ⇒ `STALE`, score 0, delivery
disabled); one integration test flies a design, records evidence and delivers a dossier.

### W1 — Workshop structure palette

**Deliverable:** structural palette row, inspector choices, 3D meshes and picking shapes, a
structure panel quoting `StructureFigures`, a preset that shows a braced airframe.

**Acceptance:** parts are placed, dragged, mirrored, undone and deleted like any other leaf; a
locked part names its technology; the inspector stays on screen with a fully equipped design at
1600 × 960; this lot is checked on screen (the only lot that requires it).

### S2 — Structural contract requirements

**Deliverable:** one soft requirement per relevant family (for example `STRUCTURE ≥ 30` on
ballistic high-difficulty offers), fed by the trial.

**Acceptance:** the requirement is soft (weight 0.5, `hard: false`) and can never refuse a dossier
by itself; evidence is hash-bound; offers without the requirement score exactly as today.

### C1 — Contract pressure and rating

**Deliverable:** deadline outcome (expired offers leave the board with a message instead of only
refusing delivery), an abstract delivered-contract rating shown in the bar, one retry rule.

**Acceptance:** no production, inventory, staffing or island system; every state is persisted with
a schema bump and a migration test; a campaign saved before the lot keeps its credits, day,
research, active contracts and completed deliveries.

---

## 6. Validation standard for every lot

Every PR runs and reports:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

plus, depending on the lot:

1. **Behaviour preservation** — a golden `--trial` JSON committed under `tests/fixtures/` for two
   designs (a plain stack and a winged design), compared field by field; refactor lots must match
   exactly, content lots may only change the fields they own.
2. **Save compatibility** — a fixture of the previous schema read by the new build, a round-trip
   test, and a refusal test for a newer schema.
3. **Determinism** — same vehicle, program, profile and seed give the same report; no `HashMap`
   iteration in a scoring path.
4. **Interface** — one test per new public function named in §4.
5. **Scope** — no new dependency, no second owner for a rule, no civil/industrial system, no real
   guidance/structural physics.
6. **Documentation** — README/architecture/changelog updated in the same PR; generated reports
   (`docs/AERO_BALANCE.md`) regenerated rather than hand-edited.

UI lots additionally list what was checked on screen and at which window size. Headless lots state
explicitly that nothing was checked on screen.

---

## 7. Regression and compatibility risks

### 7.1 Save compatibility

- Adding a `PartSpec` variant is readable by the new build but not by an older one; bump
  `VEHICLE_SCHEMA` and keep the older-schema reader, as schema 3 → 4 already does.
- Additive evidence fields must use `#[serde(default)]`; a missing field must never create a proof.
- The manifest verifies every file in the slot, so a file the game no longer reads can still block
  every later save if it is damaged. A legacy import must therefore carry only the files a current
  build actually reads (`campaign.json` from the 0.21 layout).

### 7.2 Evidence and hashes

- Any change to figures or flight changes the report, so existing dossiers go `STALE`. That is
  correct, but it means a "refactor" that drifts numerically silently invalidates every saved
  contract. Golden fixtures are the guard.
- `ContentHash` is provenance, not proof of physical validity; do not reuse it as a version.

### 7.3 Program file versioning

`programs/missile.json` has no schema field. A future block variant would fail deserialisation with
an opaque serde error, and two builds could disagree without a readable message. P0 fixes this
before any block work.

### 7.4 Determinism

The lock roll, the dispersion and the surface fault all derive from the seed. Any new subsystem
must use one of the existing seeded helpers or a new one with a fixed salt, never wall-clock time
or iteration order.

### 7.5 Duplication

The main drift risk is a second implementation of a rule already owned elsewhere: stage figures in
the UI, a second aerodynamic score, a second lock probability. The façade in §4.1 and the "one
owner" table in §3 are the rule against it.

### 7.6 Category creep

Do not add "air-to-air ballistic" style combinations as new enums. The matrix is 4 × 3 plus
difficulty; new content is a capability and an offer branch.

### 7.7 Release and CI

- CI (`.github/workflows/ci.yml`) runs fmt, clippy with `-D warnings` and the workspace tests; a
  red main is a stop line for every lot.
- The release workflow requires a `v<version>` tag equal to `Cargo.toml`; the version bump belongs
  to the lot, the tag belongs to the release, and `main` currently has no tag for 0.33.0.
- Bevy system order is explicit and chained (`main.rs`); a new system is appended to the right
  chain, not inserted arbitrarily.

### 7.8 Environment limits

The development machine used for these audits has no display: UI lots can only be validated
headlessly here (actions, persistence, rules) and must be marked "not checked on screen" unless a
graphical session is available.

---

## 8. Out of scope for the whole programme

- Real weapon engineering, guidance, navigation, tracking, seeker or warhead modelling.
- Real structural analysis: no FEA/truss coupling, no buckling, fatigue or dynamic structural
  mechanics in the game (the standalone `omega-sim-core` stays unlinked).
- Manufacturing, factories, inventory, staffing, island economy or logistics.
- Multiplayer, networking or a second front end.
- Any subsystem that cannot name its consumer in §1.
