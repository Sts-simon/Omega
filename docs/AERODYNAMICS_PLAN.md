# Lot 7 — Aerodynamics: wings, fins and control surfaces

Status: **delivered** (0.32.0). This document is the design record of the lot; the accepted values and
the trade-offs are implemented in `crates/omega-rules/src/aero.rs` and recorded in
`docs/DECISIONS.md`. It stays a game model: abstract figures, no CFD, no real aerodynamics.

## Why

The assembly tree could already place radial boosters and mirror them, but every airframe flew the
same: only diameter and mass changed the drag. Surface-to-surface and air-to-surface vehicles now
benefit from fins and control surfaces, and cruise vehicles need wings before the air-breathing
engine is worth its mass. The shape of the missile is a design decision without turning the game into
a fluid-dynamics simulator.

## Parts

All three are **leaf** parts mounted on the radial nodes of a cylindrical body. A drop with `X`
symmetry places paired/crossed sets on consecutive radial slots with a shared group id.

| Part | Capability | Parameters | Discrete choices | Role |
| --- | --- | --- | --- | --- |
| `Wing` | `part:wing` | span, chord, sweep | planform, profile, standard/light | Lift for cruise; mass and drag at speed |
| `FinSet` | `part:fin` | span, chord | stabiliser role, layout, standard/light | Passive stability: less dispersion, steadier altitude hold |
| `ControlSurface` | `part:control_surface` | span, chord | control family, standard/light | Authority: tighter speed/altitude tracking |

- Planforms: straight, swept, delta, trapezoidal, variable geometry.
- Profiles: symmetric, classic, low drag, high lift, experimental.
- Stabiliser roles: rear, front, blended (no independent fin), combined.
- Fin layouts: symmetric, asymmetric (experimental: stability traded for authority).
- Control families: classic, integrated, combined, fixed (stability only, no authority).
- Mass is span × chord × an areal density (wing 3.2, fin 4.8, control 5.6 kg/m²), times a planform
  factor; the light build is 0.70 of that at the cost of complexity and reliability.

Ranges: span 0.2–4.0 m, chord 0.1–2.0 m, sweep 0–60°. A surface wider than twice the body diameter
is a **warning** (ground clearance and launcher fit), not a refusal.

## Rules (`omega-rules::aero`)

`aero::figures(&Vehicle)` walks the tree once (not per flight step) and returns `AeroFigures`:

- `reference_area_m2`: largest body cross-section (surfaces excluded).
- `wing_area_m2`, `fin_area_m2`, `control_area_m2`, and `mean_sweep_deg` (area-weighted).
- `stability` (0..100): 55 % fin effectiveness (`fin_area / 0.30 m²`, clamped) + 45 % CG/CA margin
  score.
- `control_authority` (0..100): moving control area against the same 0.30 m² reference, with a small
  wing contribution.
- `lift` (0..100): wing area × planform lift × profile lift, normalised so one default wing reads
  about 50 and a pair saturates.
- `drag_index` (0..100): summed surface drag area (shape, profile, sweep factor) against 0.60 m².
- `complexity` and `reliability` (0..100): averaged choice effects, more surfaces and the light build
  cost reliability.
- `mass_kg`: summed aerodynamic dry mass, already included in the vehicle mass.
- `cg_y_m`, `ca_y_m`, `margin_m = cg - ca`, `balance`, `length_m`, `stable_band_m()`:
  - CG is mass-weighted over all parts at their placed positions.
  - CA is area-weighted over the surfaces only (wing 1.0, fin 0.55, control 0.25); with no surfaces
    it equals the CG, so the margin is zero.
  - The acceptable band puts the CA between 2 % and 10 % of the vehicle length **behind** the CG.
    Ahead of the CG is unstable and is a warning, never a refusal.

Every coefficient lives in one `Effect` table per choice family in `aero.rs`; no multiplier is
duplicated in the app.

## Compatibility and validation

- A surface mounts on a radial node only and cannot carry other parts (error).
- Moving surfaces need a compatible computer: integrated → standard, combined → advanced (error).
  Classic and fixed surfaces work with the basic computer every vehicle already needs.
- `Vehicle::missing_research` also checks the advanced-profile and light-structure capabilities.
- Warnings: oversized span, aerodynamic mass above 20 % of launch mass, a control surface with no
  wing or fin, an asymmetric fin set, and a CA ahead of the CG.

## Flight integration (`omega-rules::flight`)

The figures are computed once per run and applied to the existing integration:

1. **Drag** = `0.5·ρ·v²·(body_drag_area + surface_drag_area)·drag_spread`. `DRAG_COEFFICIENT` stays
   folded into the body term, so a surface-less design keeps today's drag exactly.
2. **Lift** lowers the air-breathing minimum speed (`90 m/s × (1 − 0.45·lift)`, floored at 55 %) and
   reduces the pre-pitch-over gravity loss by up to 35 %.
3. **Stability** damps the seeded drag spread by up to 60 % (never removes it).
4. **Authority and stability** raise the altitude-hold rate by up to 50 %.
5. `FlightReport::aero` carries the `AeroSummary` (stability, authority, lift, drag, areas, mass,
   margin, surface count) for the trial screen and the dossier.

A design with no aerodynamic parts leaves every term at its previous value and reports an empty
summary; a regression test pins the determinism and the zero figures.

## Save format

`PartSpec` gains the three variants, so `designs/vehicle.json` is **schema 4**. Schema 3 graphs and
the migrated schema 1-2 stage lists load unchanged with no surfaces and are rewritten as schema 4 on
the next edit; a newer schema is still refused. The design hash changes with any surface edit, so
existing trial evidence and dossiers go stale through the normal hash check.

## Workshop and 3D

- Palette row **WING / FIN SET / CONTROL**, gated by research.
- 3D: a thin plate lying on the mount, rotated by the attachment angle; wings are blue, fin sets
  green, fixed control surfaces grey and moving ones amber.
- Picking: `src/picking.rs` hits plates with an oriented box and bodies with the existing cylinder;
  one nearest-hit rule.
- Inspector: span / chord / sweep with `-`/`+` and the wheel (span first), design choices as cycling
  buttons, symmetry via `X`, drag to move (a surface is a leaf), `Delete`.
- CG (yellow sphere), CA (cyan cube), their link and the acceptable band are drawn from the figures.
- Aero panel: stability, control, lift, drag, complexity, reliability, aerodynamic mass, areas,
  sweep, CG/CA/margin/band and a **SNAPSHOT A** comparison with signed deltas.

## Research and contracts

A dedicated **AERODYNAMICS** department (the sixth, so no existing column overflows the board):
`Aerodynamics` (`part:wing`), `Fin stabilisation` (`part:fin`), `Control surfaces`
(`part:control_surface`), `Advanced profiles` (`aero:advanced_profiles`) and `Light aero structures`
(`aero:light_structures`), with prerequisites so the tree progresses instead of granting everything
at once.

Surface-to-air offers add a **soft** `STABILITY` requirement (20/100) quoted from the trial's
`AeroSummary`; it is a compromise, not a blocking gate. Range, speed, mass, lift-off, separation and
lock remain the existing requirements.

## Delivered sub-lots

1. Rules and data — `PartSpec` variants, validation, mass, `aero::figures`, schema 4 and migration.
2. Flight — drag area, lift floor, stability/authority effects, `AeroSummary`, regression tests.
3. Workshop — palette, 3D plates, box picking, inspector parameters and choices, symmetry, game check.
4. Contracts — soft stability requirement derived from the mission family.
5. Balance — three reference behaviours measured by tests: fins damp dispersion, wings let the
   turbofan run at 70 m/s, control surfaces settle the pressure target earlier.

## Acceptance

- Fins measurably reduce the seeded dispersion of a ballistic design and settle the altitude target
  sooner than the same design without them. (Covered by flight tests.)
- A cruise design with a wing can run its air-breathing engine at 70 m/s; the wingless version cannot.
- Control surfaces reduce the time to reach the pressure target.
- A design with no aerodynamic parts reports zero surfaces, zero surface drag and the same report as
  a second run.
- Schema 3 loads, flies and rewrites as schema 4; a newer schema is refused with a message.
- Surfaces are placed, mirrored, moved, deleted and adjusted with the mouse only, with readable
  refusals and warnings.
- `cargo fmt --check`, `clippy -D warnings`, the whole test suite and a game check at 1600×960 pass.

## Out of scope

- Real aerodynamics, CFD, Mach-dependent shock modelling beyond one sweep factor, flutter analysis.
- Guidance, seeker or warhead modelling; aileron animation; per-surface failure modes.
- Autopilot tuning UI: the program stays the only controller input.
