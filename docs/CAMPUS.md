# Island campus and contract campaign (0.18.0)

The island is the application hub. Three tagged 3D buildings can be entered by clicking them:

- **Contract Office:** randomized civilian/scientific offers, active project selection, delivery and payment.
- **Assembly Workshop:** modular assembly editor, instrument-profile configuration and the visual automation sequencer.
- **Computation Lab:** background material heat-conduction studys and the 1,000-sample instrument field-test stage.

## Navigation

Version 0.18.0 adds a persistent top navigation bar. Click **ISLAND**, **CONTRACTS**, **WORKSHOP** or **LABORATORY** to switch views directly. The contextual tool shelf appears on the left in the Contract Office, Workshop and Computation Lab. The Workshop shelf is vertically scrollable and exposes mouse-clickable commands for adding, selecting, translating, rotating, resizing, linking, saving and submitting components.

Mouse camera controls are shared by the Island and Workshop: right-drag orbits, middle-drag pans and the wheel zooms. Touchpad pixel scrolling is normalized separately from line-based mouse-wheel input. The camera does not react while the pointer is over an interactive UI panel. `Escape` returns to the island; if the sequencer graph or polygon sketch tool is active, close that tool first.

On the island:

- Click a tagged building, or use F1 (contracts), F2 (construction), F3 (research).
- WASD / arrows move the camera; Q/E adjust camera height.

Inside a facility, Escape returns to the island. In the workshop, Escape first closes the sequence graph or instrument-profile editor.

## Randomized contract board

Each generated board contains one offer from each of five shuffled, non-weaponized domains:

1. **Scientific sounding instrumentation:** recoverable atmospheric measurement packages.
2. **Long-duration observation:** autonomous environmental recorders and low-power observation instruments.
3. **Ground weather networks:** compatible outdoor meteorological stations.
4. **Cooperative civil-air safety:** civil telemetry and equipment-state logging for research fleets.
5. **Environmental air-ground mapping:** georeferenced environmental survey and sampling instruments.

The generator randomizes client, title, difficulty, required production quantity, unit mass limit, power budget, operating-temperature interval, uptime, data completeness, storage, unit production budget, total payout and deadline. Difficulty tightens selected technical requirements. A seeded local PRNG ensures reproducible generation for a fixed seed.

Press `N` to refresh all five offers. Already accepted contracts remain active. Up to two projects can be active at once. Press `1`–`5` to accept an offer. Use `Tab` / `Shift+Tab` to select an active project in the Contract Office.

## Project workflow

A contract now follows a playable end-to-end prototype loop:

1. **Accept:** take an offer from the Contract Office. The first accepted project is selected automatically.
2. **Design:** open the Assembly Workshop. `Ctrl+Tab` / `Ctrl+Shift+Tab` switches the selected active project. Edit the generic mechanical assembly.
3. **Configure instrumentation:** press `F11` to open the instrument-profile editor. These controls describe a civilian sensor/recorder package, not a weapon subsystem.
4. **Submit:** press `Ctrl+Enter` in the workshop. The game validates the assembly, estimated mass, power draw, operating-temperature envelope, uptime target, data completeness, storage and estimated per-unit production cost. Any unmet requirements are listed in the HUD.
5. **Scientific analysis:** open the Computation Lab and press `R`. A background material heat-conduction study is queued and associated with the selected contract. When it completes, the project advances to field-test eligibility.
6. **Field test:** press `T` after the analysis has completed. The prototype runs a deterministic 1,000-sample reliability/data-completeness check using the configured instrument profile and writes a JSON report under `results/`.
7. **Deliver:** return to the Contract Office and press `V`. The game checks available funds, deducts production cost for the required quantity, credits the client payout and moves the project into the completed-contract history.

A design or instrument-profile edit after submission invalidates that contract's stored project snapshot. Submit it again and repeat the analysis. If a background analysis is interrupted by closing the game, its contract is loaded in an analysis-failed state and can be retried.

## Instrument-profile controls

Open the editor with `F11` while in the Assembly Workshop. Press `F11` or `Escape` to close it.

| Key | Action |
|---|---|
| F1 / F2 | Decrease / increase power draw by 2 W |
| F3 / F4 | Decrease / increase storage by 16 GB |
| F5 / F7 | Decrease / increase uptime target by 1% |
| F8 / F9 | Decrease / increase data completeness by 1% |
| F10 | Cycle the supported operating-temperature envelope |

Changes to this profile increment its revision. The workshop requires a new submission after any profile adjustment.

## Facility controls

### Contract Office

- `1`–`5`: accept an available offer.
- `Tab` / `Shift+Tab`: select an active contract.
- `V`: deliver a project that passed its field test.
- `N`: generate a fresh randomized board.

### Assembly Workshop

The left tool shelf mirrors common editor commands so the basic workflow does not require memorizing shortcuts. Its **MOVE** buttons translate by 0.10 m, **ROTATE** buttons rotate by 15°, and **DIMENSIONS** buttons change one axis by 5%. The shelf scrolls vertically to keep the viewport usable.

- `Ctrl+Tab` / `Ctrl+Shift+Tab`: switch the selected active project.
- `Ctrl+Enter`: evaluate and submit the current design.
- `F11`: open the instrument-profile editor.
- `F6`: open the visual automation sequencer.
- `Ctrl+S` / `Ctrl+L`: save / load the assembly.
- `V`: toggle planar polygon sketch mode on the assembly pad.
- Left-click: add a snapped contour vertex; `Enter` closes the outer contour or current hole.
- `N`: begin a new material region; `H`: toggle hole drawing for the selected region.
- `Tab`: select the next region; `M`: cycle material; `[` / `]`: adjust thickness by 1 mm.
- `Backspace`: remove the last pending point; `Delete`: delete the selected region; `F`: finish editing.
- `Escape`: cancel the pending contour and leave sketch mode without deleting saved regions.

In new-region mode, material and thickness keys change the defaults for the next region. When an existing region is selected, they edit that region and save it. The sketch pad uses X ±4.5 m and local Z ±3.5 m with 5 cm snapping. It supports planar contours only and does not yet provide vertex dragging, boolean operations or geometric constraints.

The regular component-editor shortcuts remain available while the instrument-profile editor, sequence graph and polygon sketch mode are closed.

### Computation Lab

- `R`: queue the selected contract's material heat-conduction study. If no contract is selected, queues the generic finite-volume material-conduction study.
- `T`: run the contract's field test after a successful analysis.
- `M`: mesh the workshop's supported flat plates/panels and queue a thermal study.
- `Shift+M`: mesh the same geometry, refine a central region locally, then queue the thermal study. The report includes mesh-quality indicators.
- `C`: request cancellation of the newest active background job.

## Campaign persistence and limitations

Campaign state is saved atomically to `saves/campaign.json`, including credits, generated offers, accepted projects, completed-contract history, instrument profile, selected project and PRNG state. Assembly and sequence data remain in their existing save files. Completed reports are written to `results/`.

This is an early gameplay integration, not a validated engineering tool. The background thermal job uses the new generic 2D finite-volume solid-conduction solver; it is linked to a contract for progression but is not yet a geometry-specific solver. The 1,000-sample field test is a deterministic gameplay model of instrument uptime and data completeness, not a real qualification standard. Production is represented by an immediate cost deduction and payout when the player delivers a passed project. Live deadline countdowns, reputation, staffing, inventory and production scheduling remain future work.

These contract families are civilian and scientific. The system deliberately excludes realistic weapon roles and requirements for targeting, interception, weapon propulsion, guidance, range, radar signature or attack performance.

## Rendering and system visibility

Assembly component visuals are spawned only while the workshop is open. Editor, instrument-profile, sequencer and research HUDs are shown only in their corresponding states. The island scene and building collision bounds remain active in the hub view.


## Five-facility layout (0.21.0)

The island has five clickable facilities and matching navigation destinations:

- **Contract Office:** offers, active projects, delivery and campaign finances (F1).
- **Construction Workshop:** 3D assembly editor and instrument configuration (F2).
- **Research Centre:** technology departments for vehicle families, materials, scientific sensors, embedded computing and manufacturing (F3). Persistent research and unlock logic are planned for 0.22.0.
- **Programming Studio:** visual sequence editor and isolated abstract measurement/data-processing runtime (F4). The graph editor has moved out of the construction workshop; F6 opens it here.
- **Computation Centre:** scientific worker jobs, meshing, field tests and reports (F5).

The views are separated at the navigation, HUD and tool-panel level while continuing to share the campaign and application world. Separate interior 3D scenes are not part of this release.
