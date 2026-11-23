# Assembly graph and edit history (0.4)

## Connection model

A connection records a directed parent-child relationship between two generic mechanical components. Each child may have at most one parent. Multiple children may share the same parent. The graph must be acyclic.

Connection kinds currently include `rigid`, `revolute` and `slider`. In this version, kinds are descriptive metadata only. The editor does not yet enforce connector-face alignment, solve geometric constraints, or simulate joint motion. The `P` shortcut creates a rigid metadata link from the selected component to the nearest eligible unparented component; `Shift+P` removes all links touching the selected component. Links are rendered as thin green bars between component centres so the topology can be inspected in the 3D view.

This deliberately separates **assembly topology** from the future geometric constraint solver. The graph is already useful for displaying parent-child structure, checking incomplete assemblies and preparing future attachment rules, without pretending that a visual link is a physically solved joint.

## Schema migration

Assembly schema 2 adds a `connections` array. When loading schema 1 projects, `Assembly::migrate` upgrades the in-memory version to schema 2 and supplies an empty connection list. The previous `rotation_rad` default remains supported. Saving writes the current schema.

## Edit history

The editor stores up to 64 assembly snapshots. Every successful structural or parameter edit takes a checkpoint and clears the redo stack. `Ctrl+Z` restores the previous snapshot; `Ctrl+Y` reapplies an undone snapshot. Selection changes are intentionally not part of the assembly history.

## Precision controls

- Normal movement increments: 0.1 m.
- `Alt` + movement: 0.01 m.
- Normal rotation: 15 degrees.
- `Alt` + rotation: 1 degree.
- `Shift` reverses rotation direction.
- Normal axis resize: 5% multiplicative adjustment.
- `Alt` + axis resize: 0.01 m additive adjustment.

The 0.25 m grid snap applies to movement and therefore overrides smaller movement increments when enabled. Disable grid snapping for fine positioning.

## Known limitations

- No visual transform gizmos yet.
- No connector geometry or automatic snapping.
- Connection kinds do not yet impose kinematic constraints.
- Undo history is memory-only and is cleared when a different project is loaded.
- No branching or per-property undo; snapshots store the entire assembly.
