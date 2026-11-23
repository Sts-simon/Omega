mod editor;
mod inspector;
mod meshing;
mod metrics;
mod model;
mod revisions;
mod sketch;
mod storage;

pub use editor::{
    GizmoMode, GizmoState, handle_gizmo_input, handle_keyboard, initialize_editor, pick_part_with_mouse, setup_ui,
    sync_gizmo_visuals, sync_part_visuals, update_gizmo_visuals, update_hud,
};
pub use inspector::{
    InspectorState, handle_input as handle_inspector_input, handle_text_input as handle_inspector_text_input,
    setup_ui as setup_inspector_ui, update_ui as update_inspector_ui,
};
pub use meshing::{build_assembly_mesh_thermal_config, build_polygon_sketch_thermal_config};
pub use metrics::{assembly_content_hash, assembly_metrics};
pub use model::{Assembly, Connection, ConnectionKind, MaterialKind, Part, PartKind};
pub(crate) use revisions::{
    RevisionLog, handle_revision_input, initialize_revision_log, record_revision, restore_revision,
};
pub use sketch::PolygonSketchSession;
pub(crate) use sketch::{
    handle_polygon_sketch_input, initialize_polygon_sketch, initialize_polygon_sketch_visuals, setup_polygon_sketch_ui,
    sync_polygon_sketch_visuals, update_polygon_sketch_drag_visuals, update_polygon_sketch_hud,
};
pub use storage::DesignSession;
