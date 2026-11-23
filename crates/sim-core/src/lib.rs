//! Numerical simulation library used by Omega.
//!
//! This crate deliberately has no Bevy dependency. It contains validated physical
//! quantities, result provenance and deterministic scientific solvers that can be
//! tested headlessly or reused by another application.

pub mod mechanics;
pub mod mesh;
pub mod mesh_thermal;
pub mod metadata;
pub mod thermal;
pub mod units;
pub mod visualization;

pub use mesh::{
    mesh_planar_polygon, mesh_planar_rectangles, refine_tri_mesh_local,
    refine_tri_mesh_longest_edges, MeshError, MeshNode2D, MeshQualityStats,
    MeshRefinementResult, MeshStats, MeshTriangle2D, PlanarPolygonRegion,
    PlanarRectangle, TriMesh2D,
};
pub use mesh_thermal::{simulate_mesh_thermal, simulate_mesh_thermal_cancellable, MeshThermalConfig, MeshThermalDiagnostics, MeshThermalError, MeshThermalResult};
pub use mechanics::{
    TrussConfig, TrussDiagnostics, TrussElement, TrussElementResult, TrussError, TrussLoad,
    TrussNode, TrussNodeResult, TrussResult, TrussSupport, solve_truss,
    solve_truss_cancellable,
};
pub use metadata::SimulationMetadata;
pub use thermal::{
    BoundaryCondition, BoundaryConditions, HeatPatch, Material, ThermalConfig,
    ThermalDiagnostics, ThermalError, ThermalResult, simulate, simulate_cancellable,
};
pub use units::{Density, Duration, Length, Mass, Temperature, UnitError};

pub use visualization::{
    VisualizationError, VisualizationFiles, export_mesh_thermal_visualization, export_thermal_visualization, export_truss_visualization,
};
