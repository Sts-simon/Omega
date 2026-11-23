# AEROLAB — missile design loop (fictional, abstract)

## The loop
1. **Contract.** The player picks an offer for one missile family: ballistic, cruise, anti-ship, surface-to-air, air-to-air. Offers state limits on abstract metrics (path length, top speed, flight time, mass, lock probability, computer class).
2. **Workshop.** Build the vehicle from parts, KSP-style, in stages. Parts are resizable: lengthen a tank, enlarge an engine, make a solid motor burn faster or slower (more thrust and a heavier casing versus a longer burn). The panel shows mass, thrust, burn time, thrust-to-weight ratio and delta-v per stage.
3. **Sensors.** Each sensor unlocks programming blocks: a pitot tube gives *Hold speed*, a barometer gives *Hold pressure* (for example 1013 hPa), a radar gives *Lock target (final phase)*. Radar class changes the lock chance and the speed limit at which a lock is possible.
4. **Programming.** A Scratch-like sequencer: wait, ignite stage, separate stage, throttle, hold speed, hold pressure, lock target. The flight computer limits the number of parallel threads and the number of blocks (memory).
5. **Trials.** Seeded flight tests with graphs, events and anomalies (for example a stage dropped with propellant unburnt, a lock attempted above the radar limit, a vehicle too heavy to lift off). Batch trials come from research.
6. **Delivery.** The player delivers a *plan* (design + program + trial evidence). Nothing is produced or built.
7. **Research.** Credits and days unlock parts, sensors, computers and missile families, which in turn unlock workshop parts, programming blocks and contract types.

## Guardrails (what the game deliberately does not model)
- No real missile names, designations or performance data; every number is a gameplay value.
- No guidance laws, no tracking or seeker algorithms: a target lock is a single seeded probability roll that depends on the radar class and the speed at that moment.
- No warhead modelling: the mission module is only a mass.
- No countermeasures, no targeting of real places or forces.
- The flight model integrates speed along the path (thrust, drag, gravity during a vertical launch); there is no steering or trajectory optimisation.

## Direction

The finished game rests on **missile design alone**: contract, staged vehicle, program built from
installed hardware, seeded trials and delivery of a plan. The civil assembly bench, the instrument
profile and the civil contract loop are transitional tooling kept only until missile-family
contracts, trials and plan delivery exist; they are not part of the target game and are removed
when the missile loop replaces them. The project is intended to be published as a new GitHub
repository; this workspace is not a git repository yet.

## Status (0.26)

Playable in the application (see `docs/CHANGELOG.md` and the README):

- **Research Centre:** the 13-node, five-department tree with costs, durations, prerequisites and
  one active project; **NEXT DAY** advances the clock (day counter in the navigation bar); credits
  are deducted at start; research state and day are saved in the campaign. Completed research grants
  capabilities and gates workshop parts.
- **Missile workshop:** a staged, resizable vehicle built from the parts of `aerolab-rules::vehicle`
  (up to four stages, one flight computer in the last stage), with live mass, carried mass, thrust,
  burn time, thrust-to-weight and delta-v per stage, the computer class and limits, the programming
  blocks the installed sensors unlock, missing research and design validation. The vehicle is saved
  to `designs/vehicle.json` on every change and reloaded at startup.

Implemented and tested without any UI (`aerolab-rules`): `vehicle` (parts, stages, TWR, delta-v,
computer limits, sensor-to-block mapping, research requirements), `program` (seven blocks, threads,
validation against hardware and computer limits), `flight` (seeded abstract flight test with events,
anomalies and a lock roll), `research` (tree aligned with the loop), plus `requirement`, `dossier`,
`metrics` and `diff` for the civil verification workflow.

Not implemented yet in the application:

- No program editor for the missile blocks: the Programming Studio still hosts the 0.6 chamber
  sequencer, and `program` is only exercised by tests. No block palette, thread view or computer
  limit meters.
- No trial screen: `flight` is headless only; no graphs, anomaly display, seeds, batch trials or
  design comparison in the UI. The `analysis:monte_carlo` grant exists but nothing consumes it.
- Contracts are still the civil ones and do not use the missile vehicle; missile designs are not yet
  part of design revisions or verification evidence, and delivery of a plan does not exist.
- Research does not gate contract families or the number of active projects yet: the
  `family:*` capabilities and the `ExtraActiveProjects` grant are unused.
- The legacy civil loop (assembly bench, instrument profile, field test, dossier) remains the only
  complete contract path.

## Roadmap
- 0.25 Research: game clock, research board, persistence, capability checks. **Done.**
- 0.26 Workshop v2: missile parts, stages, resize, derived-stats panel, locked parts. **Done.**
- 0.27 Programming v2: palette from hardware, thread view, computer limit meters, program stored with the design revision.
- 0.28 Trials: run the flight test from the UI, graphs, events, anomalies, seeds, comparison of designs.
- 0.29 Contracts v3: missile families and requirement sets, plan delivery with trial evidence, deadlines. Remove the civil loop and the legacy bench.
- 0.30 Balancing, tutorial contract, performance.
