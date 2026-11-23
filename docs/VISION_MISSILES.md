# Omega — missile design loop (fictional, abstract)

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
- No guidance laws, no tracking or seeker algorithms: a target lock is a single seeded probability roll that depends on the radar class and the speed at that moment. A seed's only other effect is a small, symmetric manufacturing spread (±3 % thrust and solid-motor burn time, ±5 % drag); the flight stays deterministic for a given vehicle, program and seed.
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
- **Missile workshop:** a staged, resizable vehicle built from the parts of `omega-rules::vehicle`
  (up to four stages, one flight computer in the last stage), with live mass, carried mass, thrust,
  burn time, thrust-to-weight and delta-v per stage, the computer class and limits, the programming
  blocks the installed sensors unlock, missing research and design validation. The vehicle is saved
  to `designs/vehicle.json` on every change and reloaded at startup.

Implemented and tested without any UI (`omega-rules`): `vehicle` (parts, stages, TWR, delta-v,
computer limits, sensor-to-block mapping, research requirements), `program` (seven blocks, threads,
validation against hardware and computer limits), `flight` (seeded abstract flight test with events,
anomalies and a lock roll), `research` (tree aligned with the loop), plus `requirement`, `dossier`,
`metrics` and `diff` for the civil verification workflow.

Not implemented yet in the application:

- The missile program editor, trial screen, seeded batches and A/B comparison are playable, and the
  CONTRACTS tab runs missile offers whose dossier is bound to the vehicle, program and trial hashes.
- Research gates the contract office: an offer only appears for a researched mission family (and
  vehicle family), and `Parallel project management` raises the number of active contracts.
- The civil loop, the assembly bench, the instrument profile, the chamber sequencer and the
  scientific studies are gone; `omega-sim-core` remains a standalone library.

## Roadmap
- 0.25 Research: game clock, research board, persistence, capability checks. **Done.**
- 0.26 Workshop v2: missile parts, stages, resize, derived-stats panel, locked parts. **Done.**
- 0.27 Programming v2: palette from hardware, thread view, computer limit meters, program stored with the design revision.
- 0.28 Trials: run the flight test from the UI, graphs, events, anomalies, seeds, comparison of designs. **Done.**
- 0.29 Contracts v3: missile families and requirement sets, plan delivery with trial evidence, deadlines, research-gated families. The civil loop and the legacy bench are removed. **Done.**
- 0.30 Balancing, tutorial contract, performance.
