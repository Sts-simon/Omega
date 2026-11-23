use omega_sim_core::{PlanarRectangle, mesh_planar_rectangles, refine_tri_mesh_local};
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = mesh_planar_rectangles(
        &[PlanarRectangle {
            source_part_id: "civil-test-panel".into(),
            center_m: [0.0, 0.0],
            size_m: [1.2, 0.8],
            rotation_rad: 0.0,
            material_id: 0,
            thickness_m: 0.012,
        }],
        0.20,
    )?;
    let before = base.validate(1)?;
    let refinement = refine_tri_mesh_local(&base, [-0.22, -0.18], [0.22, 0.18], 0.055, 12)?;

    println!(
        "Base mesh: {} nodes, {} triangles",
        before.node_count, before.triangle_count
    );
    println!(
        "Refined mesh: {} nodes, {} triangles; {} rounds; {} elements split; {} edges bisected",
        refinement.stats.node_count,
        refinement.stats.triangle_count,
        refinement.rounds,
        refinement.elements_split,
        refinement.edges_split,
    );
    println!(
        "Quality: minimum angle {:.2}°, maximum radius ratio {:.3}, maximum edge {:.4} m",
        refinement.quality.minimum_angle_deg,
        refinement.quality.maximum_radius_ratio,
        refinement.quality.maximum_edge_length_m,
    );
    println!(
        "Area {:.9} -> {:.9} m²; volume {:.9} -> {:.9} m³",
        before.total_area_m2, refinement.stats.total_area_m2, before.total_volume_m3, refinement.stats.total_volume_m3,
    );

    fs::create_dir_all("results")?;
    let path = unique_path("results", "local_refined_mesh", "json")?;
    fs::write(&path, serde_json::to_vec_pretty(&refinement.mesh)?)?;
    println!("Mesh JSON: {}", path.display());
    Ok(())
}

fn unique_path(directory: &str, stem: &str, extension: &str) -> Result<PathBuf, std::io::Error> {
    for index in 0_u32..10_000 {
        let suffix = if index == 0 { String::new() } else { format!("_{index}") };
        let path = PathBuf::from(directory).join(format!("{stem}{suffix}.{extension}"));
        if !path.exists() {
            return Ok(path);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate output filename",
    ))
}
