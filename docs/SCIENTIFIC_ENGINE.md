# AEROLAB Scientific Engine 0.18.0

## Purpose and scope

Version 0.18.0 adds a mouse-first navigation layer and contextual tool panels. It does not change scientific solver behavior or saved data schemas.

Version 0.17.0 adds polygon vertex dragging and snapping to the 0.16.0 interactive Workshop sketch editor. Version 0.16.0 adds an interactive Workshop sketch editor and a multi-region material-interface adapter. Version 0.15.0 adds constrained triangulation for concave planar polygons with interior holes. Version 0.14.0 added conforming local longest-edge refinement and geometric quality diagnostics to the 0.13.0 triangular plate mesher. Version 0.12.0 retains the numerical solvers from 0.11.0 and adds independent HTML/SVG and CSV result visualization. Version 0.11.0 retains the 0.10.1 exact `f64` JSON round-tripping and adds a separate 2D civil truss solver. Version 0.10.1 retained the 0.10.0 numerical model and added exact `f64` JSON round-tripping through serde_json's `float_roundtrip` feature. Version 0.10.0 extracted the numerical code into the independent Rust library crate `crates/sim-core`. This crate has no Bevy dependency. It can be tested headlessly and can later be reused by a CLI, benchmark runner or another front end. Solver revision 2.1 precomputes cell heat capacities, internal-face conductances and boundary conductances once per study, rather than recalculating material lookups and harmonic means on every time step.

The first substantial numerical prototype in this project is a transient 2D finite-volume solver for heat conduction in a stationary solid slab. It is a general materials/industrial research model, not a model of flight, propulsion, guidance or weapon performance. It is not yet coupled to the player's 3D assembly geometry.

## Mesh refinement in 0.14.0

The unstructured triangular mesh can be refined locally by repeatedly bisecting the longest edge of triangles whose centroid falls inside an axis-aligned region and whose longest edge exceeds a requested target. Every triangle sharing a selected edge is split on that same edge, so the operation does not leave hanging midpoint nodes at a conforming two-triangle interface. Element material IDs, thickness and source-part identifiers are copied to child triangles.

Refinement is bounded by a maximum of 16 rounds and the global node/element limits. The mesh validator runs after each round. The operation also checks preservation of total planar area and volume. Quality diagnostics report minimum angle, maximum circumradius-to-inradius ratio (1 for an equilateral triangle), maximum edge length and minimum triangle area. These indicators are diagnostic only: they do not establish interpolation error or solution convergence.

The in-game `Shift+M` demonstration refines a central box of the workshop mesh; the headless `mesh_refinement_study` example shows the core API. The current region selection uses triangle centroids and axis-aligned bounds. This is not yet a general error-estimator-driven adaptive mesh refinement algorithm.

## Physical model

The solver approximates:

```text
rho * cp * dT/dt = div(k * grad(T)) + q_vol
```

where:

- `T` is temperature in kelvin (K);
- `rho` is density in kilograms per cubic metre (kg/m³);
- `cp` is specific heat capacity in joules per kilogram-kelvin (J/(kg·K));
- `k` is isotropic thermal conductivity in watts per metre-kelvin (W/(m·K));
- `q_vol` is a uniform volumetric source in watts per cubic metre (W/m³).

The domain is a rectangular, uniform, cell-centred grid. `cell_size_x_m`, `cell_size_y_m` and `thickness_m` define actual cell dimensions and volume. The thickness is constant and turns the 2D cross-section into a slab so that energy can be reported in joules.

A material catalogue and an optional row-major `cell_material_ids` map allow different cells to use different materials. Conductivity at an internal face is the harmonic mean of the two neighbouring cell conductivities. This is appropriate for a face-normal heat path through two half-cells in series and prevents a high-conductivity cell from dominating the interface artificially.

## Numerical method

The implementation uses an explicit, conservative finite-volume update (forward Euler in time). Each cell's net power is the sum of four face heat rates and its volumetric source contribution. An internal face is represented by the same conductance in both adjacent cells, so the internal heat transfer cancels in the global energy balance up to floating-point roundoff.

Supported edge conditions:

- `insulated`: zero heat flux;
- `fixed_temperature`: prescribed edge temperature, located half a cell beyond the outermost cell centre.

The solver computes a local stability bound from the sum of conductances divided by each cell's heat capacity. The coefficient arrays are laid out in compact row-major face order and reused by every Rayon-parallel time step; this keeps the hot loop to neighbour reads, multiply-adds and one capacity division per cell. Compensated Kahan summation is used for whole-domain energy and boundary-power totals to reduce accumulation error. The reported limit includes a 10% safety factor. A requested time step above this limit is rejected before allocating the simulation field. The solver also rejects studies whose grid-cell count multiplied by step count exceeds five billion updates. These are deliberate fail-fast rules; the solver does not silently clamp the user's inputs.

## Diagnostics and validation

Every result contains:

- the complete input configuration;
- solver/model identifiers and versions;
- a deterministic input fingerprint (FNV-1a for bookkeeping, not security);
- the final temperature field in row-major order;
- min/max/mean temperature;
- initial and final thermal energy;
- signed boundary energy transfer;
- integrated source energy;
- absolute and relative energy-balance residual;
- simulated duration, wall-clock duration and stable time-step limit.

A JSON input deck can be passed to the standalone example, allowing the scientific crate to run without Bevy. The example writes the full result to a uniquely named JSON report under `results/`. `crates/sim-core/examples/material-interface-study.json` demonstrates a two-material grid. Scientific JSON reports use precise `f64` deserialization so persisted numerical fields reload bit-for-bit. A completed result is not automatically a validated engineering result. Energy-balance residuals and numerical convergence are useful checks, but they do not establish that the chosen model represents a real object accurately.

## Test suite

The independent crate contains tests for:

1. cooling of a hot patch through fixed-temperature boundaries;
2. energy conservation with all boundaries insulated and no source;
3. invariance of a uniform-temperature field when the boundaries match;
4. the analytical temperature rise from a uniform volumetric source;
5. rejection of a time step above the computed stability limit;
6. heterogeneous material maps;
7. bitwise-identical output with one versus four Rayon threads;
8. cooperative cancellation;
9. rejection of an invalid material map;
10. JSON round-tripping of complete reports;
11. mixed fixed/insulated edge behavior and energy accounting;
12. rejection of studies over the cell-update budget;
13. parsing and validation of the bundled two-material JSON deck;
14. unit validation and metadata fingerprint behavior.

Run them from the repository root:

```bash
cargo test -p aerolab-sim-core
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
AEROLAB_WORKER_THREADS=16 cargo run --release -p aerolab-sim-core --example thermal_study
AEROLAB_WORKER_THREADS=16 cargo run --release -p aerolab-sim-core --example thermal_study -- crates/sim-core/examples/material-interface-study.json
```

## Current limitations

- 2D slab only; no full 3D mesh yet.
- Isotropic conductivity only; no tensor or temperature-dependent properties.
- Uniform volumetric source only; no per-cell source map yet.
- Explicit time integration; high-diffusivity or very fine grids may require small time steps.
- No contact resistance, radiation, convection boundary condition or phase change.
- No automatic mesh generation from the Bevy assembly editor.
- No independent experimental dataset has been used to validate material parameters.

The next numerical milestones should be analytical benchmark cases, grid/time-step convergence reports, a temperature-dependent material interface, and a separate implicit solver for stiff thermal problems. Coupling to 3D assembly geometry should wait until the discretization and validation pipeline are trustworthy.


## Concave polygon regions and holes (0.15.0)

`mesh_planar_polygon` accepts one `PlanarPolygonRegion` with an outer contour, zero or more hole contours, one material ID, one slab thickness and a source-part ID. Coordinates are metres in a common 2D plane. Rings may be supplied in either winding direction; the mesher normalizes outer and hole winding before calling `earcutr` constrained triangulation.

Before triangulation, the mesher checks finite bounded coordinates, removes a repeated closing point and collinear vertices that lie between their neighbours, rejects degenerate or self-intersecting rings, and verifies that every hole is strictly inside the outer contour. Hole boundaries may not touch/cross the outer contour or each other, and nested/overlapping holes are rejected. The total input vertex count is limited to 2,048 and the hole count to 64.

After triangulation, every triangle is oriented counter-clockwise and checked for positive area and valid indices. The summed triangle area must agree with `abs(outer area) - sum(abs(hole area))` within a combined tolerance of `1e-8 * max(expected area, 1 m²)`. Material ID, thickness and source-part provenance are copied to every output triangle. Mesh validation then checks manifold edge use and connectivity of all generated nodes. The triangular thermal FEM can consume this `TriMesh2D` directly; its current fixed-temperature boundary option applies to all boundary loops, including hole boundaries.

The bundled `polygon_mesh_study` example reads `crates/sim-core/examples/perforated-panel-polygon.json` by default or accepts another `PlanarPolygonRegion` JSON file as its first argument. It runs the existing transient slab thermal FEM and exports JSON, HTML/SVG and CSV reports. The geometry is a civil/industrial demonstration, not an engineering qualification model.

Current scope: the core `mesh_planar_polygon` primitive still triangulates one region per call, with one material and thickness per region. The AEROLAB 0.16 application adapter combines several such outputs and welds coincident complete shared edges. Curved boundaries, automatic mesh sizing, 3D solids and stronger CAD topology repair remain future work.

## 2D civil truss statics (0.11.0)

`crates/sim-core/src/mechanics.rs` implements a linear-elastic, pin-jointed planar truss. Each node contributes two translational degrees of freedom. Each member contributes the standard axial-bar stiffness `EA/L * bᵀb`, where `b = [c, s, -c, -s]`. Prescribed support degrees of freedom are eliminated from the solve.

The reduced sparse stiffness matrix is assembled into sorted row maps and converted to compact row storage. A Jacobi-preconditioned conjugate-gradient iteration solves `K u = F`. Matrix-vector products run across independent rows in the configured Rayon pool; dot products use deterministic compensated summation. The residual is recomputed every 32 iterations and the Krylov direction is restarted after residual replacement. A sparse-entry × iteration budget rejects accidental excessive work before solving. The solver returns a clear error if the iteration encounters non-positive curvature, the diagonal reveals a free degree of freedom without positive stiffness, cancellation is requested, or the iteration limit is reached. This is not a complete eigenvalue-based stability analysis: an unloaded mechanism that is not excited by the applied load can remain undetected.

Member force is positive in tension and negative in compression. Member stress is force divided by cross-sectional area. Nodal reactions are recovered from the assembled internal member forces minus applied nodal loads. For this linear model, strain energy `Σ(½ N ΔL)` should equal external load-displacement work `½ Fᵀu`; the report includes the relative difference as a numerical consistency diagnostic.

The bundled `civil-bridge-truss.json` is a small teaching example. The solver does not include bending, buckling, plasticity, fatigue, load combinations, safety factors, dynamics or 3D continuum behavior. A small residual and energy agreement establish numerical consistency for the stated model, not physical correctness or code compliance.


## Visualization exports (0.12.0)

`aerolab-sim-core::visualization` writes reports without a Bevy dependency:

- `export_thermal_visualization` creates an SVG heatmap embedded in a responsive HTML report and a full-resolution CSV (`x_index`, `y_index`, cell-centre coordinates, temperature). The SVG uses block averaging and is capped at 144 × 144 display bins; the CSV retains the solver's full field.
- `export_truss_visualization` creates an SVG with undeformed and deformed truss geometry, member tension/compression colors, supports, loads and node labels, plus a combined node/member CSV. The deformation is exaggerated only in the SVG; its scale factor is stated in the report. All CSV displacements and stresses are the solver's original SI values.
- Both reports contain solver metadata, input fingerprint and numerical diagnostics. They are diagnostic artifacts, not design certification.

The Computation Lab queues a default civil truss study with `G` and opens the most recent report with `O`. Thermal jobs continue to use `R`. JSON, HTML and CSV outputs are stored under `results/`.


## 0.13.0 unstructured planar thermal FEM

The mesh milestone connects a constrained subset of workshop geometry to the scientific engine. `crates/sim-core/src/mesh.rs` creates a conforming triangular mesh from non-overlapping, coplanar rectangles. The application adapter only accepts horizontal `Panel` and `Platform` parts, using workshop X/Z as the planar X/Y domain and the part's Y dimension as the slab thickness. Rotation about the vertical axis is supported. Tilted and stacked parts are rejected. Adjacent rectangles share nodes; positive-area overlap is rejected. The mesher records each triangle's material ID, thickness and source part ID.

`crates/sim-core/src/mesh_thermal.rs` solves transient heat conduction on that mesh using first-order triangular finite elements and a lumped heat-capacity matrix. Element stiffness is assembled into a compressed-row sparse matrix. The explicit update is

```text
C_i dT_i/dt = Q_i - sum_j(K_ij T_j)
```

where `C_i` is nodal lumped heat capacity, `Q_i` is the assembled volumetric source power and `K` is the global conductivity matrix. Fixed-temperature conditions apply to all outer boundary nodes when configured; otherwise the boundary is insulated. The explicit stability bound uses a conservative maximum absolute row-rate estimate of the mass-scaled stiffness operator. The study is rejected rather than silently changing the requested time step.

Diagnostics include mesh area/volume, total nodal heat capacity, fixed boundary node count, stable time-step limit, initial/final thermal energy, source energy, estimated boundary reaction energy and global balance residual. Reports show triangle colours based on the mean of each element's three nodal temperatures; CSV retains the full nodal field and element provenance.

The in-game `M` command builds the mesh from the live `DesignSession` and queues it in the existing CPU worker. Only `Panel` and `Platform` parts are included. Other types are reported as skipped. If supported parts are tilted, on different vertical planes, or overlap in plan view, mesh creation fails explicitly. The default mesh size is 5 cm.

This model remains a 2D slab approximation. It has no holes, curved boundaries, automatic 3D tetrahedralization, contact resistance, convection/radiation, anisotropic or temperature-dependent material properties, nonlinear behavior or experimental validation. It is a research-game model, not an engineering certification tool.


### Assembly geometry adapter and mesh conformity (0.13.0)

`src/design/meshing.rs` converts horizontal workshop `Panel` and `Platform` components into `PlanarRectangle` inputs. Workshop X/Z coordinates become the solver X/Y plane; the workshop Y dimension becomes slab thickness. The adapter currently supports rotation around the vertical axis only. It uses a small illustrative material catalogue and records the originating assembly part ID on every triangle. Other component kinds are skipped and reported.

The mesher accepts separate regions and regions sharing one complete matching edge. It rejects positive-area overlaps, point-only contacts and partial-edge contacts. The latter two are important: merging a point or leaving a T-junction can create a false thermal connection or a non-conforming interface. Mesh node, triangle and subdivision limits are checked before the solver starts.

In the Computation Lab, `M` builds a mesh from the live workshop assembly and queues the existing unstructured thermal solver. The `mesh_thermal_study` example can also load `MeshThermalConfig` from a JSON deck without starting Bevy. It writes JSON, HTML/SVG and CSV outputs under `results/`.


## Multi-region polygon sketches (0.16.0)

The Workshop now persists up to 64 planar polygon regions in `saves/polygon-sketch.json`. Each region has an outer ring, zero or more holes, one material ID, one slab thickness and a stable source ID. The user draws snapped points on the workshop pad, closes each ring, and can select material/thickness per region.

The application adapter triangulates every region using the existing constrained polygon mesher, then welds nodes that coincide within a fixed 1e-8 m coordinate tolerance. A complete shared edge therefore becomes one topological interface in the combined mesh, allowing the unstructured thermal solver to exchange energy across different material regions. Regions with overlapping area, point-only contact or partial-edge contact are rejected.

Press `P` in the Computation Lab to queue a background thermal simulation from the persisted sketch. The result uses the existing mesh thermal solver and HTML/SVG/CSV report pipeline. The current sketch editor is a planar 2D tool mapped to the Workshop pad; it is not a 3D CAD kernel and does not provide vertex dragging, boolean operations, snapping to existing edges or geometric constraints.
