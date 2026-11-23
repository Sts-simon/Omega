# AEROLAB 0.21.0 — UI and mouse controls

## Navigation

The top bar switches directly between **Island**, **Contracts**, **Workshop**, **Research**, **Programming** and **Computation**. The five island buildings remain clickable. `F1` through `F5` open Contracts, Workshop, Research, Programming and Computation respectively; `Escape` returns to the island.

## Camera

| Input | Action |
|---|---|
| Right mouse drag | Orbit around the current target |
| Middle mouse drag | Pan the camera target in the horizontal plane |
| Mouse wheel | Zoom |
| Touchpad two-finger scroll | Smooth zoom; pixel deltas are normalized |
| `W/A/S/D` or arrows | Move camera target |
| `Q/E` | Move camera target down/up |
| `Shift` + movement | Faster camera movement |

Camera navigation is available on the Island and in the Construction Workshop. It pauses while the sequencer graph, instrument-profile editor or polygon sketch tool owns the interaction. Mouse camera movement is also suppressed while the pointer is over an interactive UI panel.

## Workshop tool shelf

The left-side shelf is scrollable. It exposes the most common editor operations as clickable buttons:

- Add Panel, Beam, Enclosure, Platform or Cylinder.
- Select previous/next component; delete the selection.
- Connect the selected component to the nearest eligible component, or disconnect it.
- Move along X/Y/Z in 0.10 m steps.
- Rotate around X/Y/Z in 15-degree steps.
- Increase/decrease each dimension axis by 5%.
- Change material, toggle grid snap, undo/redo, save/load.
- Enter polygon sketch, instrument profile and design submission. The visual sequencer is now in the Programming Studio.
- Switch between translate arrows (`T`) and rotation rings (`R`) with clickable mode buttons.

## 3D transform gizmos

A selected component shows one of two local-axis gizmos in the viewport:

| Input | Action |
|---|---|
| `T` | Show translation arrows |
| `R` | Show rotation rings |
| Left-click + drag red axis | Translate along local X |
| Left-click + drag green axis | Translate along local Y |
| Left-click + drag blue axis | Translate along local Z |
| Left-click + drag a rotation ring | Rotate around that local axis |
| `G` | Toggle 0.25 m position snapping |
| `Ctrl+Z` / `Ctrl+Y` | Undo / redo the completed drag as one edit |

The gizmo highlights the axis under the pointer in yellow. Translation arrows use conical heads. Gizmo size scales with camera distance to stay readable at different zoom levels. A local axis nearly aligned with the camera view cannot be dragged because it has almost no visible screen-space length; orbit the camera to expose it. Dragging temporarily captures the pointer and pauses camera navigation so a single gesture cannot move both the part and the view. Dragging is disabled while a modal editor or the visual sequencer owns input. A completed drag is one undoable edit; grid snapping is applied to absolute assembly-local positions.

The keyboard controls remain available for finer editing: `Alt` changes movement and rotation increments to the fine values, and existing dimension shortcuts remain unchanged.

## UI behavior

- The active facility is highlighted in the top navigation bar.
- Each facility shows only its relevant tool shelf.
- The Workshop shelf scrolls independently of the 3D viewport.
- The general campus status panel is hidden in the Computation Centre; the research panel shows the selected project and queue status instead.
- UI overlays block world picking so clicking a button does not also select a building, part or sketch vertex.



## Numeric inspector (0.20.0)

The component inspector appears on the right side of the Workshop view. It exposes position X/Y/Z in metres, rotation X/Y/Z in degrees, and dimensions X/Y/Z in metres. Click a value, type a number, press Enter to apply, or Escape to cancel. Backspace edits the draft; `.` or `,` inserts a decimal separator; the top-row digits and numpad digits are supported. Each row also has minus/plus buttons: position uses 0.01 m steps, rotation 1 degree, and dimensions 0.001 m. Numeric edits are explicit values and therefore bypass grid snapping.


## Facility ownership in 0.21.0

- **Contracts:** offer board, active projects and delivery.
- **Workshop:** 3D assembly, inspector, polygon sketch and instrument profile.
- **Research:** technology department overview; actual research unlocks are planned for 0.22.0.
- **Programming:** F6 opens the visual sequence graph; Ctrl+F8 saves and Ctrl+F9 loads the program. Escape closes the graph before leaving the facility.
- **Computation:** scientific jobs, mesh studies, field tests and generated reports.

The views have separate navigation and UI ownership. This version does not yet load separate interior 3D worlds for each building.
