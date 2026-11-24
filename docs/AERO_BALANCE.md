# Aerodynamics effect tables

Generated from `crates/omega-rules/src/aero_effects.json` by `omega --aero-report`. Unlisted fields are neutral: 1 for the multipliers, 0 for complexity and reliability.

## Planforms

| Choice | Area | Lift | Drag | Stability | Authority | Complexity | Reliability | Mass |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| delta | 0.90 | 1.15 | 1.10 | 1.10 | 0.90 | 0.25 | -0.05 | 1.05 |
| straight | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | +0.00 | 1.00 |
| swept | 0.95 | 1.00 | 0.75 | 1.05 | 0.95 | 0.15 | +0.00 | 1.00 |
| trapezoidal | 0.95 | 1.05 | 0.90 | 1.05 | 1.00 | 0.10 | +0.02 | 1.02 |
| variable_geometry | 1.00 | 1.10 | 0.85 | 1.05 | 1.15 | 0.55 | -0.20 | 1.25 |

## Profiles

| Choice | Area | Lift | Drag | Stability | Authority | Complexity | Reliability | Mass |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| classic | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.05 | +0.02 | 1.00 |
| experimental | 1.00 | 1.35 | 0.80 | 0.85 | 1.20 | 0.60 | -0.30 | 1.00 |
| high_lift | 1.00 | 1.25 | 1.25 | 0.90 | 0.90 | 0.20 | -0.05 | 1.00 |
| low_drag | 1.00 | 0.85 | 0.70 | 0.95 | 0.95 | 0.20 | +0.00 | 1.00 |
| symmetric | 1.00 | 0.90 | 0.95 | 1.05 | 1.00 | 0.00 | +0.05 | 1.00 |

## Stabiliser roles

| Choice | Area | Lift | Drag | Stability | Authority | Complexity | Reliability | Mass |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| blended | 1.00 | 0.15 | 0.70 | 0.50 | 0.40 | 0.05 | +0.05 | 0.80 |
| combined | 1.00 | 0.25 | 1.20 | 1.15 | 0.90 | 0.30 | -0.05 | 1.10 |
| front | 1.00 | 0.30 | 1.05 | 0.60 | 0.80 | 0.15 | +0.00 | 1.00 |
| rear | 1.00 | 0.20 | 1.00 | 1.00 | 0.60 | 0.00 | +0.05 | 1.00 |

## Fin layouts

| Choice | Area | Lift | Drag | Stability | Authority | Complexity | Reliability | Mass |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| asymmetric | 1.00 | 1.00 | 1.10 | 0.80 | 1.30 | 0.35 | -0.25 | 1.00 |
| symmetric | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | +0.00 | 1.00 |

## Control families

| Choice | Area | Lift | Drag | Stability | Authority | Complexity | Reliability | Mass |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| classic | 1.00 | 0.15 | 1.00 | 1.00 | 1.00 | 0.05 | +0.00 | 1.00 |
| combined | 1.00 | 0.20 | 1.15 | 1.10 | 1.35 | 0.40 | -0.12 | 1.10 |
| fixed | 1.00 | 0.10 | 1.10 | 1.25 | 0.00 | 0.00 | +0.10 | 1.00 |
| integrated | 1.00 | 0.15 | 0.90 | 1.00 | 1.20 | 0.25 | -0.05 | 1.00 |

