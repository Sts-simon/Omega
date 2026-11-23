> **Archived document.** Written before the 0.26 missile-only direction and kept for historical
> reference only. It no longer describes the current project: see `README.md`,
> `docs/ARCHITECTURE.md` and `docs/ROADMAP_OMEGA.md`.

# Laboratory sequence editor — prototype 0.6


The sequence editor now has two synchronized views over the same typed program model: the original keyboard-driven list and a draggable visual graph. The graph is a visualization of the program's execution flow, not a separate runtime. Saving the program stores the node positions and breakpoints in the same JSON file.

## Data model

A `Program` contains ordered `ProgramBlock` values. Every block has a stable `u32` identifier, a persisted 2D canvas position and an optional breakpoint. Branches target block IDs, so moving cards around the canvas or reordering the list does not change the destination.

Supported block kinds:

- `Start`: marks the first instruction.
- `Wait`: advances simulated time by a configured duration.
- `SetHeaterPower`: sets the generic chamber heater output from 0 to 1.
- `ReadTemperature`: samples the chamber temperature into the runtime sensor register.
- `BranchTemperature`: compares the last sampled temperature with a threshold and jumps to a block ID when true.
- `SetVariable`: assigns a numeric runtime variable.
- `SetVariableExpression`: evaluates a bounded arithmetic expression and stores its result.
- `AddVariable`: increments a numeric runtime variable.
- `BranchVariable`: compares a numeric runtime variable with a threshold and jumps when true.
- `Record`: appends a message to the execution log.
- `End`: marks successful completion and switches the heater output off.

Variable names must start with a letter or underscore and contain only ASCII letters, digits or underscores, with a maximum length of 32 characters. The expression evaluator supports numeric literals, variable names, parentheses and `+`, `-`, `*`, `/` with standard precedence and unary signs. It rejects undefined variables, division by zero and non-finite results. An increment to an undefined variable starts that variable at zero. A branch that reads an undefined variable fails with a clear runtime error.

The first and last blocks must be `Start` and `End`. IDs must be unique. Branch targets must exist. Durations, outputs, thresholds, variable names and canvas positions are validated. The editor currently limits programs to 128 blocks.

## Visual graph editor

Press `F6` to open or close the graph canvas. Sequencer editing shortcuts are active only while this canvas is open, so they do not collide with the assembly editor. Click a node to select it and drag it to reposition it. Node positions are saved with the program. Teal lines show sequential flow; orange lines show the TRUE branch of a condition. The FALSE path of a condition follows the next block in the ordered program.

The graph view is deliberately a first step: it visualizes typed control-flow links and supports dragging nodes, but does not yet allow arbitrary port-to-port wiring. Conditions use stable block IDs, and their jump destination can be changed with `T` / `Shift+T` in the list editor.

The graph canvas is placed at a fixed screen offset intended for the default 1600×960 window. The list and research HUD overlays are hidden while the graph is open so they do not cover nodes. The node positions themselves are stored relative to the canvas, so they survive window movement and save/load.

## Runtime and debugging

Single-step executes one block. A Wait block advances the complete remaining wait duration in one step. Run mode advances waits with the game's virtual clock. The chamber model uses the closed-form solution of a first-order thermal response, so its result does not depend on the number of rendering frames used to span a wait.

The runtime handles up to 64 non-wait instructions per frame and stops after 10,000 instructions in one run. This prevents a user-created branch loop from monopolizing the application. A breakpoint pauses immediately before its block; resuming skips that breakpoint once and executes the block. Editing a program while playback is active is refused.

Executed instructions are recorded in a bounded in-memory trace (up to 20,000 entries). Press `Ctrl+F7` to export it to `results/sequence_trace.json`. Each row contains the block ID, simulated time, chamber temperature, heater output and a snapshot of numeric variables. Completed Wait blocks are included when their simulated duration finishes, not on every frame.

The chamber model remains a gameplay demonstrator, not a validated heat-transfer solver. The separate research job system remains responsible for computational research examples and their provenance reports.

## Controls

| Input | Action |
|---|---|
| `F6` | Open / close visual graph editor |
| Left mouse click + drag | Select and move a node in graph view |
| `Space` | Run / pause |
| `N` | Execute one block while paused |
| `B` | Toggle breakpoint on selected block |
| `F1` | Insert Wait (2 s) |
| `F2` | Insert temperature read (`chamber_01`) |
| `F3` / `Shift+F3` | Insert temperature condition (`>=` / `<`, target defaults to End) |
| `F4` | Insert heater output (50%) |
| `F5` | Insert numeric variable assignment (`sample_count = 0`) |
| `F7` | Insert numeric variable increment (`sample_count += 1`) |
| `F8` | Insert numeric variable condition (`sample_count >= 3`) |
| `F9` | Insert expression assignment (`derived_value = (sample_count + 1) * 2`) |
| `F11` | Insert Record block |
| `Shift+F12` | Remove selected block (Start/End and referenced targets are protected) |
| `PageUp` / `PageDown` | Select previous / next block |
| `Shift+PageUp` / `Shift+PageDown` | Move selected block in the ordered program |
| `T` / `Shift+T` | Change selected condition destination forward / backward |
| `-` / `=` or numpad `-` / `+` | Decrease / increase selected numeric parameter |
| `Ctrl+F7` | Export execution trace to `results/sequence_trace.json` |
| `Ctrl+F8` | Save to `saves/lab_sequence.json` |
| `Ctrl+F9` | Load the saved program |
| `Ctrl+F10` | Reset runtime |
| `Escape` | Close graph view |

The numeric edit increments are 1 second for Wait, 10 percentage points for heater output, 1 °C for a temperature threshold, and 1 unit for variable values / thresholds. The variable increment block changes by 1 unit. The graph view displays runtime state and current variables in its inspector footer.

## Persistence and migration

Program schema version 2 stores canvas positions and breakpoint flags. Version 1 programs are migrated on load by assigning a default grid layout and disabling breakpoints. Example `examples/chamber-thermal-test.json` intentionally remains schema 1 so the migration path can be tested. `examples/variable-expression-demo.json` demonstrates a current schema 2 program with numeric expressions and a breakpoint.

## Deliberate limitations

This is not a general-purpose scripting language or arbitrary signal-flow graph. There are no user-defined functions, analog signal ports, external I/O drivers, subroutines or arbitrary port-to-port connections. The arithmetic expression evaluator is intentionally limited to basic scalar arithmetic. The graph visualizes sequence order and conditional jumps. These limitations keep the current editor focused on generic laboratory automation.
