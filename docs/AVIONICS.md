# Onboard electronics

The avionics lot gives the player a second stack to design: the electronics that decide how well
the vehicle is commanded, where it thinks it is, what it can tell the ground, and whether the
airframe is healthy. Everything is fictional and abstract — a device is a mass, a paper cost, a
power draw or supply, a complexity and a reliability shift, never a real component or signal.

## One shape for every device

`omega-rules::avionics` owns the family. Seven categories share one description:

| Category | Parts |
| --- | --- |
| Computers | basic, standard and advanced flight computer (legacy parts) |
| Control modules | control augment |
| Sensors | pitot tube, barometer, basic radar, active radar (legacy parts) |
| Navigation | inertial platform |
| Communication | datalink |
| Power | power cell |
| Diagnostics | self-test suite |

Every device answers the same questions through `AvionicsSpec`:

- `category` — where the workshop shows it and what the inspector names;
- `mass_kg` — the mass the vehicle actually carries (the legacy parts keep reading `stats()`);
- `cost_cr` — abstract design cost, summed and shown, never spent from the campaign wallet;
- `power_w` — positive draws from the mission bus, negative supplies it;
- `complexity` and `reliability` — the same abstract quantities the aerodynamic surfaces use;
- `authority_bonus`, `lock_bonus`, `fault_reduction` — the effects the trial applies.

`AvionicsFigures` aggregates the tree once per read: devices, mission modules, mass, cost, draw,
supply, margin, complexity, reliability, the three capped bonuses and one flag per module. `Vehicle`
exposes it as `Vehicle::avionics()`, so the workshop panel, the trial and the headless report all
read the same numbers.

## The five mission modules

| Module | Mass | Cost | Power | Complexity | Reliability | Effect |
| --- | --- | --- | --- | --- | --- | --- |
| Control augment | 9 kg | 1 400 cr | 25 W draw | 0.35 | −0.05 | control authority +15 % |
| Inertial platform | 7 kg | 1 800 cr | 18 W draw | 0.20 | +0.05 | lock chance +6 points |
| Datalink | 4 kg | 1 200 cr | 12 W draw | 0.25 | 0 | lock chance +4 points |
| Power cell | 14 kg | 900 cr | 60 W supply | 0.30 | −0.02 | feeds the mission bus |
| Self-test suite | 5 kg | 1 100 cr | 10 W draw | 0.20 | +0.10 | fault exposure −50 % |

The bonuses are capped at +20 % authority, +15 lock points and −65 % fault exposure, so a stack
cannot make the trial free. Only the power cell may be duplicated; stacking cells adds capacity.

## Power budget

The mission bus is drawn by the mission modules only. The flight computer and the legacy sensors
run on the vehicle bus, so a design without mission electronics never needs a cell and every
pre-avionics save stays valid. A design with a drawing module and no cell is refused with
`the mission electronics draw N W but no power source is fitted`; a deficit names both figures.
Supply is not silently shared with the rest of the vehicle: the cell is a mission-bus cell.

A fitted stack with a margin below 25 % of its supply gets a warning, not a refusal, so a player can
finish a design and add a cell later.

## Dependencies and compatibility

Errors (they block a trial, like every other validation error):

- a control augment needs a **standard** flight computer;
- a datalink needs a navigation module to point at;
- a non-power module may not be fitted twice;
- drawing modules need enough supply (see above).

Warnings:

- a mission module that would be dropped before the last stage (keep it with the computer);
- a mission module mounted on a radial node (electronics mount on the axis);
- thin power headroom.

Legacy rules still apply: exactly one flight computer, in the last stage, and the hardware gates of
the program editor are unchanged.

## Research

Two technologies unlock the family and sit in the existing departments:

- **Flight control electronics** (Onboard computing, 6 500 cr, 9 days, after the standard computer)
  grants `avionics:control`, `avionics:power` and `avionics:diagnostics`;
- **Navigation and datalink** (Sensors and electronics, 7 500 cr, 10 days, after the basic radar)
  grants `avionics:navigation` and `avionics:datalink`.

The tree is now 20 technologies for 77 000 cr and 86 serial days; the budget test pins those
figures. Locked modules appear greyed in the palette and name their technology.

## Effects in a trial

`flight::simulate` reads `AvionicsFigures` once and applies them to the existing integration:

- the lock probability of the fitted radar gains the navigation and datalink bonus (capped at 1.0);
- the aerodynamic authority is multiplied by `1 + authority_bonus`;
- the seeded surface-fault chance is multiplied by `1 - fault_reduction`;
- `FlightReport` carries `avionics` (the summary figures) and `surface_fault` (whether the fault
  fired), and `omega --trial` prints both, so a screenless run can prove the effect.

No new guidance, sensor or warhead model is added. A design without mission modules keeps every
previous figure: multipliers are 1, the bonus is 0, and the seeded rolls are untouched.

## Workshop

- The palette's AVIONICS section now holds the three computers and the five modules, still grouped
  by the rules' categories.
- The inspector quotes the electronics category for every device, and for a module its cost, draw
  or supply, complexity, effect and missing dependency.
- The stage panel adds `Electronics N devices | draw/supply W, margin W | cost cr`, in warning
  colour when the budget is short.
- Validation rows and the existing select shortcut make every avionics error a jump to its part.

## Persistence

`designs/vehicle.json` moves to **schema 5** with the new `PartSpec::Avionics` variant. Schema 3 and
4 files load unchanged and rewrite as 5 on the next save; a newer schema is still refused. The
design hash covers the modules, so adding or removing one invalidates old trial evidence and
dossiers through the normal check.

## Tests

`crates/omega-rules/src/avionics.rs` pins category reachability, the aggregated figures, the caps,
power stacking, each dependency and the duplicate rule. `assembly.rs` pins schema-4 migration and
that a module without a cell is refused while the powered design validates and round-trips.
`flight.rs` pins the +6 lock points of a navigation stack, the halved fault exposure (for a seed
that faults unprotected, it never faults protected) and the no-module invariants. `src/missile.rs`
pins the workshop path: a module is refused without power, then validates with a cell.

## Not modelled

No device carries a frequency, a waveform, a protocol, a coordinate, a seeker, a warhead or a real
power figure. Power, cost and effects are gameplay values chosen for readable trade-offs.
