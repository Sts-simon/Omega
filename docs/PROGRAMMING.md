# Visual programming

The program editor turns the onboard program into a small, readable language: parallel threads of
generic blocks, a shared set of flags and a mode register, a named library of reusable programs, and
a seeded debugger that shows what actually ran. Everything is fictional and abstract; no block
carries guidance, tracking, seeker or targeting logic.

## Audit of the previous editor

Before this lot the PROGRAM tab offered a single palette of seven blocks (`wait`, `ignite`,
`separate`, `throttle`, `hold speed`, `hold pressure`, `lock target`) in one undifferentiated
list, a single thread-level validation, one autosaved program and no way to reuse a program, draw
its connections or inspect a run. All of that is preserved: the seven blocks and their hardware
rules, the thread/memory limits and the trial integration are unchanged.

## Categories and blocks

`omega-rules::program` owns `BlockCategory` and `Block::category()`. The editor renders one library
section per category from `BlockCategory::ALL`.

| Category | Blocks | Hardware |
| --- | --- | --- |
| Inputs | wait speed, wait pressure | pitot tube / barometer |
| Conditions | wait flag | computer |
| Logic | toggle flag | computer |
| States | set flag | computer |
| Timers | wait | computer |
| Outputs | ignite stage, separate stage, throttle, hold speed, hold pressure, lock target | computer, 2 stages, pitot, barometer, radar as appropriate |
| Modes | set mode, wait mode | computer |
| Diagnostics | mark | computer |

Each block has a one-line description, a label built from its values and a category, so the palette,
the selected-block panel and the debugger all read the same metadata.

## Shared state

- **Four flags** (`F0`–`F3`): `set flag` writes one, `toggle flag` flips one, `wait flag` continues
  once it matches (or reports a timeout).
- **One mode register** (`M0`–`M3`): `set mode` writes it, `wait mode` continues once it matches.
- **Eight diagnostic marks** (`#0`–`#7`): `mark` only adds an event and a trace entry; it never
  changes the flight.

Inputs and conditions are gates: the thread stays on the block while the condition is false, and
after `timeout_s` it records an anomaly and moves on. This keeps the language non-blocking without a
branch/merge instruction set.

## Connections

A thread is a chain. The editor draws a line between consecutive blocks — the connection — and the
rules produce the links (`Program::connections()`). A selected block is the insertion point: a
library click splices a block after it, `DUPLICATE` copies a block after itself, `MOVE` reorders and
`REMOVE` closes the gap. Three connection rules are enforced:

- nothing can follow `Lock target` (it closes its chain, final phase);
- a stage cannot be ignited after it was separated in the same chain;
- only one `Lock target` exists in the whole program.

## Validation

`ProgramIssue { severity, thread, block, message }` is the single source; `program_issues()` fills
it and `validate_program()` keeps the error-only string list used by the flight and the trials.

Errors: missing computer, too many threads, memory overflow, hardware the vehicle does not carry,
out-of-range values, unknown stages, lock placement, invalid connections, and out-of-range flags,
modes, marks and timeouts.

Warnings: an empty thread, a condition nobody sets (flag or mode), a 0 s wait, a program that uses
at least 90 % of the computer memory, and the hardware warnings that already existed.

The editor shows an error count and a warning count as chips, then the first six issues, coloured by
severity and naming their thread and block.

## Hardware compatibility

`Vehicle::available_blocks()` maps the fitted vehicle to the block kinds; the palette greys out
everything the vehicle cannot run and the editor refuses it with a readable message. Adding a
sensor, radar or second stage in the workshop, or removing it, changes the palette on the next
rebuild.

## Reusable library

`ProgramLibrary` holds up to 12 named programs; saving under an existing name replaces it. The
editor's `SAVE COPY` uses the first free default name (`Program 1`, …), and every entry can be
loaded on any vehicle or deleted. Loading a program does not validate it against the current
vehicle: the validation panel immediately reports the hardware or limits it misses, which is the
point of reusing a program.

The library is stored in `programs/library.json` through the same save slot and manifest as the
working program; the working program keeps its own `programs/missile.json`.

## Debugger

`RUN TRACE` flies the current program once with the seeded default configuration and
`FlightConfig::trace = true`. `FlightReport::trace` lists each executed block with its time, thread,
index and label, and flags the conditions that timed out. The panel then shows the executed block
count, the flight time, the maximum speed, the anomalies and the first trace lines. Any edit clears
the report, so the panel never shows a run of an older program.

Trace recording is opt-in: a batch trial does not allocate it.

## Tests

- `program.rs`: every category owns a block, range checks for flags/modes/marks/timeouts, invalid
  connections, warnings (dead condition, empty thread, thin memory), connection links, library save
  / replace / remove / cap / JSON round-trip.
- `flight.rs`: flags, modes and marks execute, a dead condition times out and appears in the trace,
  and a run without `trace` keeps the trace empty.
- `program_editor.rs`: palette hardware gating, thread and memory limits, adjust/flip/insert/
  duplicate, library save/load/delete through the session, debugger trace and stale-trace clearing,
  and the error/warning split.

## Not modelled

No block describes a real control law, a real sensor, a message format, a target or a weapon. Flags,
modes, timeouts and marks are generic gameplay values chosen for readable programs.
