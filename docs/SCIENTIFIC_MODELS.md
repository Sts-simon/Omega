# Scientific models

## 0.11.0: linear-elastic 2D civil trusses

A second headless model solves the static equilibrium of small, pin-jointed planar trusses. Nodes have X/Y coordinates; members are straight axial bars with constant cross-sectional area and Young's modulus; supports restrain selected translations; and concentrated nodal loads act in newtons. The model assembles the sparse global stiffness matrix and solves the free degrees of freedom with Jacobi-preconditioned conjugate gradients. Matrix-vector products are parallelized by row through Rayon while each row retains a fixed summation order.

For each member, the report contains axial extension, strain, signed axial force and signed axial stress. Positive force/stress denotes tension; negative denotes compression. For each node, it records displacement and the residual force vector, which corresponds to support reaction at restrained degrees of freedom. A sparse matrix-vector work budget bounds the requested iteration cost. Diagnostics compare linear-elastic strain energy with half the external load-displacement work and report the solver residual.

This is an educational **pin-jointed truss** model, not a general-purpose finite-element package. It assumes small displacements, linear elasticity, straight two-force members and ideal pins. It does not represent bending, shear deformation, local/global buckling, joint eccentricity, plasticity, fatigue, contact, dynamics, shells or 3D solids. It cannot certify a real structure. Conjugate-gradient failure checks are not a full eigenvalue stability study, so an unloaded mechanism outside the applied load subspace may not be detected. The bundled input is a small civil bridge truss.

Run it independently of Bevy:

```bash
cargo test -p aerolab-sim-core mechanics
AEROLAB_WORKER_THREADS=16 cargo run --release -p aerolab-sim-core --example truss_study -- crates/sim-core/examples/civil-bridge-truss.json
```

## 0.10.0: transient solid heat conduction

The former unitless five-point heat-diffusion demonstration has been replaced by a Bevy-independent finite-volume solver in `crates/sim-core`. The solver models transient conduction in a stationary 2D solid slab with explicit SI dimensions, constant slab thickness, material properties, an optional per-cell material map, a uniform volumetric source and insulated or fixed-temperature edges.

The equation is `rho * cp * dT/dt = div(k * grad(T)) + q_vol`. Internal face conductivity uses a harmonic mean. The explicit time step is checked against a stability bound computed from each cell's actual conductance sum and heat capacity. A time step that exceeds the bound is rejected rather than silently clamped.

Each report retains the full temperature field, configuration, model and solver versions, input fingerprint, min/max/mean temperature, initial/final energy, integrated boundary/source energy and energy-balance residual. The solver is a general materials-research model; it is not coupled to flight systems or weapons, and it is not yet coupled to the 3D assembly geometry.

See [SCIENTIFIC_ENGINE.md](SCIENTIFIC_ENGINE.md) for equations, assumptions, diagnostics, tests and limitations.

## Contract-linked research (0.10.0)

The computation-lab job now calls the independent `aerolab-sim-core` library through a thin compatibility module. A contract ID is still carried by the job manager for campaign progression. The solver run itself is a generic material thermal-screening task and must not be interpreted as a qualification of the complete assembly. The 1,000-sample field test remains a deterministic gameplay model, not a statistical reliability certification.

## Sequence-editor chamber

The sequence editor still contains a small first-order thermal chamber model for demonstrating events, timers, sensor reads, conditions and actuator outputs. It is a gameplay/control-flow demonstration, not the finite-volume solver and not a validated physical chamber model.


## Unstructured planar plate heat conduction (0.13.0)

This model is distinct from the rectangular finite-volume solver. It uses a triangular mesh, linear shape functions and a lumped thermal capacity matrix. It accepts only non-overlapping, coplanar plate regions with per-element material and thickness. The mesh exporter currently maps workshop `Platform` and `Panel` dimensions X/Z into a 2D plane, and part dimension Y into thickness. The global conductivity matrix is assembled in CSR form; the solver uses explicit time stepping with a conservative stability bound. Outer mesh nodes may be fixed to a prescribed temperature; otherwise all boundaries are insulated. It is suitable for demonstrating geometry-to-mesh-to-report workflow, not for qualification of real components.


### Workshop-derived planar thermal meshes (0.13.0)

The first geometry adapter is intentionally narrow: horizontal rectangular panels/platforms only, with yaw rotation and a constant slab thickness per part. It does not infer a mesh for beams, enclosures, cylinders, tilted parts, stacked parts or arbitrary curved geometry. Regions may be disjoint or share a complete matching edge. Positive-area overlap, point-only contact and partial edge contact are rejected. Each triangle retains material ID, thickness and source-part ID.

This makes the result traceable to workshop components but does not make it a validated product simulation. Material catalogue values are illustrative, the slab model is two-dimensional, and inter-part contact resistance is not represented.


## Conforming local triangular mesh refinement (0.14.0)

The mesh core supports longest-edge bisection of selected triangles. A selected edge is represented once by a shared midpoint node, and every adjacent triangle containing that edge is split on the same midpoint. This edge propagation avoids hanging nodes across the shared edge, including between adjacent material regions. Regional refinement selects triangles by their centroid within an axis-aligned rectangle and repeats until their longest edge is below the target or the configured round limit is reached.

The operation preserves per-triangle material, thickness and source-part metadata. It checks that total area and volume remain within tolerance, validates the refined mesh and reports minimum angle, maximum circumradius-to-inradius ratio, maximum edge length and minimum triangle area. This is a geometry-quality diagnostic, not a posteriori solution-error estimation. The centroid-based region selection is a prototype; it is not yet a general adaptive-refinement strategy driven by numerical error indicators.
