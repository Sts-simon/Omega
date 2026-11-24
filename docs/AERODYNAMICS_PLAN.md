# Lot 7 — Aerodynamics: wings, fins and control surfaces

Status: **planned** (target 0.32.0). No code from this lot is implemented yet; the values below are
proposals to settle during the first phase, and the open ones are listed at the end.

## Why

The assembly tree can already place radial boosters and mirror them, but every airframe flies the
same: only diameter and mass change the drag. Surface-to-surface and air-to-surface vehicles should
benefit from fins and control surfaces, and cruise vehicles need wings before the air-breathing
engine is worth its mass. This lot makes the *shape* of the missile a design decision without
turning the game into a fluid-dynamics simulator.

The model stays abstract and deterministic, in the spirit of `docs/VISION_MISSILES.md`: one
vehicle-level set of aerodynamic figures derived from the parts, used by the existing 1-D flight
integration. No guidance law, no CFD, no per-frame geometry.

## New parts

All three are **leaf** parts mounted on the radial nodes of a cylindrical body (they have no
`Top`/`Bottom`/`Radial` nodes of their own). A drop with `X` symmetry places paired/crossed sets on
consecutive radial slots, reusing the existing `symmetry_slots` and group ids.

| Part | Capability | Main parameters | Role |
| --- | --- | --- | --- |
| `Wing` | `part:wing` | span (m), root chord (m), sweep (°) | Lift for cruise flight; mass and wave drag at high speed |
| `FinSet` | `part:fin` | span (m), chord (m) | Passive stability: less dispersion, steadier pitch-over and altitude hold |
| `ControlSurface` | `part:control_surface` | span (m), chord (m) | Authority: tighter speed/altitude tracking, better terminal figures |

Mass: each surface has an areal density (kg/m²) so span × chord is the design lever. A wing has a
lower areal density than a fin (thin skin), a control surface is in between. Exact constants are
open values (see `docs/DECISIONS.md`).

Validation ranges (proposal): span 0.2–4.0 m, chord 0.1–2.0 m, sweep 0–60°. A surface wider than
twice the body diameter is a **warning** (ground clearance and launcher fit), not a refusal, like
the existing diameter step.

## Aggregation (`omega-rules::aero`)

A new Bevy-free module computes one `AeroFigures` value per vehicle from `Vehicle` directly (walk the
tree once, not per flight step):

- `reference_area_m2`: largest body cross-section, as today.
- `wing_area_m2`, `fin_area_m2`, `control_area_m2`, each summed per stage and total.
- `mean_sweep_deg`, area-weighted.
- `stability`: `fin_area / reference_area`, clamped, plus a small bonus for control surfaces.
- `control_authority`: `control_area / reference_area`, clamped.
- `parasitic_drag_area_m2`: body drag area + surface skin drag (span × chord × a form factor).

The figures are exposed on the flight report (`AeroSummary`) so the workshop can show them live and
the dossier can quote them.

## Flight integration (`omega-rules::flight`)

Keep the current integration and seeded dispersion; add three effects:

1. **Drag** becomes `0.5·ρ·v²·(body_drag_area + surface_drag_area·sweep_penalty)`. The existing
   `DRAG_COEFFICIENT` is folded into the body term so designs without surfaces keep today's figures
   and all current tests keep their numbers.
2. **Lift** applies after pitch-over and in air launch: the lift ratio
   `wing_area / reference_area` lowers the speed at which the air-breathing engine can run
   (`AIR_BREATHING_MIN_SPEED_MS`, floored) and reduces the gravity loss during the pitch-over
   manoeuvre. A heavy, oversized wing still costs more than it saves.
3. **Stability and authority** act on the existing abstract quantities:
   - stability scales down the drag dispersion (±5 % base) and shortens the time to settle the
     altitude target;
   - control authority tightens the speed/altitude controller (smaller steady-state error) and
     slightly reduces the "no control" anomalies, without changing the target-lock probability.

A design with no aerodynamic part must produce **byte-identical** flight reports to the current
build; a regression test compares a set of seeds before/after.

## Save format

`PartSpec` gains three variants, so `designs/vehicle.json` moves to **schema 4**. Schema 3 (and the
earlier migrated forms) load unchanged with no surface parts and are rewritten as schema 4 on the
next edit; a newer schema is still refused. Tests: round-trip schema 4, migration from the current
fixtures, refusal of schema 5, and the existing schema-3 revision fixtures.

## Workshop and 3D

- Palette row **AERO SURFACES** (WING / FIN SET / CONTROL SURFACE), gated by research.
- 3D: a thin flat box lying on the surface, rotated by the attachment angle; wings get a small
  dihedral, fins are vertical. They reuse the part colour and the selection outline.
- Picking: `src/picking.rs` gains a box raycast; the current cylinder path stays for bodies. A part
  carries its own pick shape, so `nearest_part` keeps one implementation.
- Inspector: span / chord / sweep with `-`/`+` and the wheel (span first), symmetry via `X`, drag to
  move the subtree (a surface is a leaf), `Delete`.

## Research and contracts

- New tech nodes under the existing aerodynamics branch: `Aerodynamics` (`part:wing`),
  `Fin stabilisation` (`part:fin`), `Control surfaces` (`part:control_surface`). The cruise family
  keeps gating cruise offers; wings are not a hard requirement there, but the offer's range target is
  out of reach without them.
- Phase 2 of the offer model: an optional `min_stability` / `min_control` requirement derived from
  mission family (surface-to-surface wants fins, cruise wants lift), quoted from `AeroSummary`.
  Until then the existing mass/range/speed/lock requirements remain the only gates.

## Steps (each one shippable)

1. **Rules and data** — `PartSpec` variants + validation + mass, `aero.rs` aggregation, schema 4 and
   migration tests. No UI, no flight change.
2. **Flight** — drag area, lift floor, stability/authority effects, `AeroSummary` on the report,
   no-aero regression test, reference-design figures recorded in `docs/DECISIONS.md`.
3. **Workshop** — palette, 3D meshes, box picking, inspector parameters, symmetry drops, game check.
4. **Contracts and dossier** — optional stability/control requirements and the cruise offer tuning.
5. **Balance** — three reference designs (ballistic without fins, ballistic with four fins, cruise
   with wings + air-breathing engine) measured over fixed seeds; tune constants once, then record
   them.

## Acceptance

- A ballistic design with four fins has a measurably smaller drag dispersion and settles its altitude
  target sooner than the same design without fins.
- A cruise design with wings can run its air-breathing engine and reach the cruise offer's range at a
  lower speed than the wingless design, which cannot complete it.
- A design with control surfaces tracks the program's speed/altitude targets with a smaller error.
- A design with no aerodynamic parts produces the same report as before the lot (fixed seeds).
- Schema 3 saves open, fly and migrate to schema 4; a newer schema is refused with a message.
- Wings/fins/control surfaces are placed, mirrored (`X`), moved, deleted and adjusted with the mouse
  only, with readable refusals for an invalid node or an oversized surface (warning, not refusal).
- `cargo fmt --check`, `clippy -D warnings`, the whole test suite and a game check at 1600×960 pass.

## Out of scope

- Real aerodynamics, CFD, Mach-dependent shock modelling beyond one sweep penalty, flutter analysis.
- Guidance, seeker or warhead modelling; ailerons/elevator animation; per-surface failure modes.
- Autopilot tuning UI: the program stays the only controller input.
