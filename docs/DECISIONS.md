# Initial technical decisions

## Rust + Bevy

Bevy provides a Rust-native ECS and 3D renderer. It is suitable for a data-oriented project where design objects, facilities, jobs and UI state can be represented as components/resources. Bevy is evolving quickly, so the dependency is pinned to the current 0.20.1 release line and upgrades should be deliberate.

## Separate long-running jobs from Bevy systems

Long calculations run on a dedicated worker thread. Rayon parallelizes the numerical loop across available CPU cores (capped at 16 in this prototype). The Bevy update loop only enqueues work and consumes progress events.

## JSON first, database later

JSON is easy to inspect and version for early prototypes. SQLite may become useful once the project stores large catalogues of parts, jobs, results and contracts. Avoid introducing a database before the schemas stabilize.

## Scientific validity is not implied by computational cost

A calculation taking five minutes is not automatically accurate. Every future solver needs unit checks, convergence tests, documented assumptions and independent reference cases. The current finite-volume heat solver has explicit SI dimensions, stability checks and energy accounting, but it has not yet been independently validated against experimental datasets or coupled to the assembly geometry. It must not be treated as a certified engineering solver.
