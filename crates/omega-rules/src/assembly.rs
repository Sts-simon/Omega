//! Assembly graph for a missile: parts are attached to named nodes of a parent part instead of
//! being piled into a flat list of stages. Stages are derived from the tree (a decoupler, or an
//! explicit separation edge, starts a new stage), so the same design can be built as a stack, with
//! radial boosters, or as a branching vehicle.
//!
//! The stored design is versioned (`vehicle.json` schema 4). Schema 1-2 files (a list of stages)
//! are migrated when read, schema-3 graphs load with no aerodynamic surfaces, and a newer schema is
//! refused instead of being silently reinterpreted.
//! All values remain abstract gameplay figures; see `docs/VISION_MISSILES.md`.

use crate::vehicle::{ComputerClass, G0, Part, PartSpec, RadarClass, RadarSpec, SensorKind, Stage, StageReport};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, BTreeSet};

/// Current `designs/vehicle.json` schema. 1-2 were the flat stage lists of the early workshops;
/// 3 added the attachment graph, 4 the aerodynamic surfaces (schema-3 files load unchanged).
pub const VEHICLE_SCHEMA: u32 = 4;
/// A design may not grow beyond this many derived stages (the old workshop limit).
pub const MAX_STAGES: usize = 4;

/// Attachment points a part offers. `Top` and `Bottom` are on the central axis; `Radial(i)` are the
/// four evenly spaced body mounts, numbered clockwise and stored with the design.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeId {
    Top,
    Bottom,
    Radial(u8),
}

impl NodeId {
    pub fn label(self) -> String {
        match self {
            Self::Top => "top".to_string(),
            Self::Bottom => "bottom".to_string(),
            Self::Radial(index) => format!("radial {}", index + 1),
        }
    }

    pub fn is_radial(self) -> bool {
        matches!(self, Self::Radial(_))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attach {
    pub parent: String,
    pub node: NodeId,
    /// Clockwise rotation of a radial mount, in degrees. Ignored for axial nodes.
    #[serde(default)]
    pub angle_deg: f64,
    /// Explicit stage boundary, used by migrated designs whose stage had no decoupler.
    #[serde(default)]
    pub separation: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlacedPart {
    pub id: String,
    pub spec: PartSpec,
    /// `None` only for the single root part.
    #[serde(default)]
    pub attach: Option<Attach>,
    /// Symmetry group shared by copies placed together (2/3/4 radial mounts).
    #[serde(default)]
    pub symmetry: Option<String>,
}

impl PlacedPart {
    pub fn part(&self) -> Part {
        Part {
            id: self.id.clone(),
            spec: self.spec,
        }
    }
}

/// A design warning, with the part it concerns when selecting that part helps.
#[derive(Clone, Debug, PartialEq)]
pub struct VehicleWarning {
    pub message: String,
    pub part: Option<String>,
}

/// Errors and warnings share one shape so the workshop can render one validation list and make
/// every row a shortcut to the part at fault.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueSeverity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VehicleIssue {
    pub severity: IssueSeverity,
    pub message: String,
    pub part: Option<String>,
}

impl VehicleIssue {
    pub fn error(message: impl Into<String>, part: Option<String>) -> Self {
        Self {
            severity: IssueSeverity::Error,
            message: message.into(),
            part,
        }
    }

    pub fn warning(message: impl Into<String>, part: Option<String>) -> Self {
        Self {
            severity: IssueSeverity::Warning,
            message: message.into(),
            part,
        }
    }
}

/// A design as a tree of parts. The root sits at the bottom of the stack.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Vehicle {
    pub name: String,
    pub parts: Vec<PlacedPart>,
}

#[derive(Serialize)]
struct SavedVehicle<'a> {
    schema_version: u32,
    name: &'a str,
    parts: &'a [PlacedPart],
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ReadVehicle {
    /// Schema 3 and later: a graph.
    Graph {
        schema_version: u32,
        name: String,
        parts: Vec<PlacedPart>,
    },
    /// Schema 1-2: a stack of stages.
    Legacy { name: String, stages: Vec<Stage> },
}

impl Serialize for Vehicle {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        SavedVehicle {
            schema_version: VEHICLE_SCHEMA,
            name: &self.name,
            parts: &self.parts,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Vehicle {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match ReadVehicle::deserialize(deserializer)? {
            ReadVehicle::Graph {
                schema_version,
                name,
                mut parts,
            } => {
                if schema_version > VEHICLE_SCHEMA {
                    return Err(D::Error::custom(format!(
                        "vehicle schema {schema_version} is newer than this game (schema {VEHICLE_SCHEMA})"
                    )));
                }
                for part in &mut parts {
                    part.spec
                        .validate()
                        .map_err(|e| D::Error::custom(format!("{}: {e}", part.id)))?;
                }
                Ok(Self { name, parts })
            }
            ReadVehicle::Legacy { name, stages } => Ok(Self::from_stages(name, stages)),
        }
    }
}

/// Height and radius of a part, in metres. The 3D view and the attachment nodes use the same
/// figures so a node sits exactly where the mesh ends.
pub fn dimensions(spec: &PartSpec) -> (f64, f64) {
    match *spec {
        PartSpec::SolidMotor {
            length_m, diameter_m, ..
        } => (length_m, diameter_m / 2.0),
        PartSpec::FuelTank { length_m, diameter_m } => (length_m, diameter_m / 2.0),
        PartSpec::LiquidEngine { scale } => (0.35 * scale, 0.15 * scale),
        PartSpec::AirBreathing { scale } => (0.5 * scale, 0.17 * scale),
        PartSpec::Sensor {
            kind: SensorKind::Pitot,
        } => (0.25, 0.03),
        PartSpec::Sensor {
            kind: SensorKind::Barometer,
        } => (0.12, 0.12),
        PartSpec::Sensor {
            kind: SensorKind::Radar(RadarClass::Basic),
        } => (0.20, 0.15),
        PartSpec::Sensor {
            kind: SensorKind::Radar(RadarClass::Active),
        } => (0.28, 0.18),
        PartSpec::Computer { .. } => (0.15, 0.15),
        PartSpec::Decoupler => (0.06, 0.20),
        PartSpec::MissionModule { mass_kg } => ((0.2 + mass_kg / 150.0).min(2.2), 0.20),
        // A surface is a thin plate: its chord runs along the body, its span outward from the
        // mount, so the radial offset places the root exactly on the parent's surface.
        PartSpec::Wing { span_m, chord_m, .. }
        | PartSpec::FinSet { span_m, chord_m, .. }
        | PartSpec::ControlSurface { span_m, chord_m, .. } => (chord_m, span_m / 2.0),
    }
}

/// Part diameter used to compare a child with the node it attaches to. Mismatches are a warning,
/// never a refusal: the prompt keeps drag out of this lot.
fn node_diameter(spec: &PartSpec) -> f64 {
    (dimensions(spec).1 * 2.0).max(0.05)
}

fn stack_order(spec: &PartSpec) -> u8 {
    match spec {
        PartSpec::LiquidEngine { .. } | PartSpec::AirBreathing { .. } => 0,
        PartSpec::SolidMotor { .. } | PartSpec::FuelTank { .. } => 1,
        PartSpec::Decoupler => 3,
        _ => 2,
    }
}

pub fn radial_slots(spec: &PartSpec) -> u8 {
    match spec {
        PartSpec::SolidMotor { .. } | PartSpec::FuelTank { .. } | PartSpec::MissionModule { .. } => 4,
        _ => 0,
    }
}

/// One row of the workshop's assembly tree.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeEntry {
    pub id: String,
    /// 0 for the root, 1 for a direct child, and so on: the indent the tree view draws.
    pub depth: usize,
    /// Derived stage index (0-based); a decoupler or a separation edge starts a new one.
    pub stage: usize,
    pub parent: Option<String>,
    pub node: Option<NodeId>,
}

/// Position of a part in the assembled vehicle, in metres from the bottom of the root.
#[derive(Clone, Debug, PartialEq)]
pub struct PartPose {
    pub id: String,
    pub stage: usize,
    pub y_m: f64,
    pub x_m: f64,
    pub z_m: f64,
    pub height_m: f64,
    pub radius_m: f64,
}

impl Vehicle {
    pub fn from_stages(name: impl Into<String>, stages: Vec<Stage>) -> Self {
        let mut parts = Vec::new();
        let mut previous: Option<(String, bool)> = None;
        for (stage_index, stage) in stages.into_iter().enumerate() {
            let mut ordered: Vec<Part> = stage.parts;
            ordered.sort_by_key(|part| stack_order(&part.spec));
            for (part_index, part) in ordered.into_iter().enumerate() {
                let attach = previous.as_ref().map(|(parent, parent_is_decoupler)| Attach {
                    parent: parent.clone(),
                    node: NodeId::Top,
                    angle_deg: 0.0,
                    separation: stage_index > 0 && part_index == 0 && !*parent_is_decoupler,
                });
                previous = Some((part.id.clone(), matches!(part.spec, PartSpec::Decoupler)));
                parts.push(PlacedPart {
                    id: part.id,
                    spec: part.spec,
                    attach,
                    symmetry: None,
                });
            }
        }
        Self {
            name: name.into(),
            parts,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    pub fn part(&self, id: &str) -> Option<&PlacedPart> {
        self.parts.iter().find(|part| part.id == id)
    }

    pub fn part_mut(&mut self, id: &str) -> Option<&mut PlacedPart> {
        self.parts.iter_mut().find(|part| part.id == id)
    }

    pub fn root(&self) -> Option<&PlacedPart> {
        self.parts.iter().find(|part| part.attach.is_none())
    }

    pub fn root_id(&self) -> Option<&str> {
        self.root().map(|part| part.id.as_str())
    }

    pub fn children(&self, parent: &str) -> Vec<&PlacedPart> {
        self.parts
            .iter()
            .filter(|part| part.attach.as_ref().is_some_and(|a| a.parent == parent))
            .collect()
    }

    /// `separator` is true when the edge itself starts a new stage: an explicit separation flag or
    /// a decoupler between the two parts.
    fn edge_separates(&self, part: &PlacedPart) -> bool {
        let Some(attach) = &part.attach else {
            return false;
        };
        if attach.separation {
            return true;
        }
        self.part(&attach.parent)
            .is_some_and(|parent| matches!(parent.spec, PartSpec::Decoupler))
    }

    /// Stage index for every part, derived from the tree. Bottom stage is 0.
    pub fn stage_depths(&self) -> BTreeMap<String, usize> {
        let mut depths = BTreeMap::new();
        let Some(root) = self.root() else {
            return depths;
        };
        let mut stack = vec![(root.id.clone(), 0usize)];
        while let Some((id, depth)) = stack.pop() {
            if depths.insert(id.clone(), depth).is_some() {
                continue; // duplicate ids or a cycle: validation reports it
            }
            for child in self.children(&id) {
                let next = depth + usize::from(self.edge_separates(child));
                stack.push((child.id.clone(), next));
            }
        }
        depths
    }

    pub fn stage_count(&self) -> usize {
        self.stage_depths().values().max().map_or(0, |max| max + 1)
    }

    pub fn stage_of(&self, id: &str) -> Option<usize> {
        self.stage_depths().get(id).copied()
    }

    /// Rows of the workshop's assembly tree: depth-first from the single root, children in a
    /// stable node/id order, with the derived stage and the node each part hangs from.
    pub fn tree_entries(&self) -> Vec<TreeEntry> {
        let mut entries = Vec::new();
        let Some(root) = self.root() else {
            return entries;
        };
        let depths = self.stage_depths();
        let mut stack = vec![(root.id.clone(), 0usize)];
        let mut seen = BTreeSet::new();
        while let Some((id, depth)) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            let Some(part) = self.part(&id) else {
                continue;
            };
            entries.push(TreeEntry {
                id: id.clone(),
                depth,
                stage: depths.get(&id).copied().unwrap_or(0),
                parent: part.attach.as_ref().map(|attach| attach.parent.clone()),
                node: part.attach.as_ref().map(|attach| attach.node),
            });
            let mut children: Vec<&PlacedPart> = self.children(&id);
            children.sort_by(|a, b| {
                let node = |part: &PlacedPart| part.attach.as_ref().map(|attach| attach.node);
                node(a).cmp(&node(b)).then_with(|| a.id.cmp(&b.id))
            });
            // Reversed, so the stack pops them in the sorted order above.
            for child in children.into_iter().rev() {
                stack.push((child.id.clone(), depth + 1));
            }
        }
        entries
    }

    /// Parts in display order (stage, then bottom to top), with their derived stage.
    pub fn parts(&self) -> Vec<(usize, &PlacedPart)> {
        let depths = self.stage_depths();
        let poses = self.poses();
        let mut list: Vec<(usize, &PlacedPart)> = self
            .parts
            .iter()
            .map(|part| (depths.get(&part.id).copied().unwrap_or(0), part))
            .collect();
        let order: BTreeMap<&str, f64> = poses.iter().map(|p| (p.id.as_str(), p.y_m)).collect();
        list.sort_by(|(sa, a), (sb, b)| {
            sa.cmp(sb).then_with(|| {
                order
                    .get(a.id.as_str())
                    .copied()
                    .unwrap_or(0.0)
                    .total_cmp(&order.get(b.id.as_str()).copied().unwrap_or(0.0))
            })
        });
        list
    }

    /// Stage views in firing order, in the shape the programs, the flight model and the dossier
    /// expect. Parts keep their identity; the view is rebuilt from the tree every time.
    pub fn stage_views(&self) -> Vec<Stage> {
        let count = self.stage_count();
        let mut stages = vec![Stage::default(); count];
        for (stage, part) in self.parts() {
            if let Some(view) = stages.get_mut(stage) {
                view.parts.push(part.part());
            }
        }
        stages
    }

    pub fn subtree_ids(&self, id: &str) -> Vec<String> {
        let mut ids = Vec::new();
        let mut stack = vec![id.to_string()];
        let mut seen = BTreeSet::new();
        while let Some(current) = stack.pop() {
            if !seen.insert(current.clone()) {
                continue;
            }
            ids.push(current.clone());
            for child in self.children(&current) {
                stack.push(child.id.clone());
            }
        }
        ids
    }

    pub fn is_descendant(&self, ancestor: &str, id: &str) -> bool {
        let mut current = self.part(id).and_then(|p| p.attach.as_ref()).map(|a| a.parent.clone());
        let mut seen = BTreeSet::new();
        while let Some(parent) = current {
            if parent == ancestor {
                return true;
            }
            if !seen.insert(parent.clone()) {
                return false;
            }
            current = self
                .part(&parent)
                .and_then(|p| p.attach.as_ref())
                .map(|a| a.parent.clone());
        }
        false
    }

    fn node_taken_by(&self, parent: &str, node: NodeId, except: Option<&str>) -> Option<&str> {
        self.parts
            .iter()
            .filter(|part| part.id.as_str() != except.unwrap_or(""))
            .find_map(|part| {
                let attach = part.attach.as_ref()?;
                (attach.parent == parent && attach.node == node).then_some(part.id.as_str())
            })
    }

    /// Why `node` of `parent` cannot take this spec right now (empty when it can).
    pub fn node_status(&self, parent: &str, node: NodeId, spec: &PartSpec) -> Result<(), String> {
        let Some(parent_part) = self.part(parent) else {
            return Err("that part no longer exists".to_string());
        };
        if parent_part.spec.is_aero() {
            return Err(format!("{parent} is a surface and cannot carry parts"));
        }
        if spec.is_aero() && !node.is_radial() {
            return Err(format!("{} mounts on a radial node only", spec.short_label()));
        }
        if let Some(taken) = self.node_taken_by(parent, node, None) {
            return Err(format!("that node is already used by {taken}"));
        }
        match node {
            NodeId::Radial(_) if radial_slots(&parent_part.spec) == 0 => Err(format!("{} has no radial mount", parent)),
            NodeId::Radial(_) => Ok(()),
            NodeId::Top | NodeId::Bottom => Ok(()),
        }
    }

    pub fn attach(&mut self, child: &str, parent: &str, node: NodeId, angle_deg: f64) -> Result<(), String> {
        if child == parent {
            return Err("a part cannot be attached to itself".to_string());
        }
        if !self.parts.iter().any(|p| p.id == child) {
            return Err(format!("{child} is not part of the vehicle"));
        }
        if self.is_descendant(child, parent) {
            return Err(format!("{parent} is already inside {child}'s subtree (cycle)"));
        }
        if let Some(taken) = self.node_taken_by(parent, node, Some(child)) {
            return Err(format!("that node is already used by {taken}"));
        }
        let spec = self.part(child).map(|p| p.spec).ok_or("missing part")?;
        self.node_status(parent, node, &spec)?;
        let depths_before = self.stage_count();
        let part = self.part_mut(child).ok_or("missing part")?;
        part.attach = Some(Attach {
            parent: parent.to_string(),
            node,
            angle_deg,
            separation: false,
        });
        if self.stage_count() > MAX_STAGES {
            let part = self.part_mut(child).ok_or("missing part")?;
            part.attach = None;
            return Err(format!("a vehicle can have at most {MAX_STAGES} stages"));
        }
        if self.stage_count() < depths_before {
            return Err("that attachment would fold the vehicle onto itself".to_string());
        }
        Ok(())
    }

    pub fn detach(&mut self, id: &str) -> Result<(), String> {
        let part = self.part_mut(id).ok_or("missing part")?;
        if part.attach.take().is_none() {
            return Err("the root part cannot be detached".to_string());
        }
        Ok(())
    }

    /// Remove a part and everything attached below it. Returns the removed ids.
    pub fn remove_subtree(&mut self, id: &str) -> Vec<String> {
        let ids = self.subtree_ids(id);
        self.parts.retain(|part| !ids.contains(&part.id));
        ids
    }

    /// Replace one part while keeping its attachment and its children.
    pub fn replace_part(&mut self, id: &str, spec: PartSpec) {
        if let Some(part) = self.part_mut(id) {
            part.spec = spec;
        }
    }

    /// Radial slots for a symmetry count, starting at `first` and evenly spaced. The count comes
    /// from the parent body, not from the part being placed: surfaces have no mounts of their own
    /// but still form four-way sets around a body.
    pub fn symmetry_slots(&self, parent: &str, _spec: &PartSpec, first: u8, count: u8) -> Vec<(NodeId, f64)> {
        let slots = self.part(parent).map_or(0, |part| radial_slots(&part.spec)).clamp(1, 4);
        (0..count.min(slots))
            .map(|step| {
                let index = (first + step) % slots.max(1);
                (NodeId::Radial(index), f64::from(index) * 90.0)
            })
            .collect()
    }

    /// Errors and warnings in one list, each naming the part to select to fix it. This is the only
    /// compatibility source the workshop reads; it never builds a rule of its own.
    pub fn issues(&self) -> Vec<VehicleIssue> {
        let mut issues = self.error_issues();
        issues.extend(self.warning_issues());
        issues
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let issues: Vec<String> = self.error_issues().into_iter().map(|issue| issue.message).collect();
        if issues.is_empty() { Ok(()) } else { Err(issues) }
    }

    fn error_issues(&self) -> Vec<VehicleIssue> {
        let mut issues: Vec<VehicleIssue> = Vec::new();
        if self.parts.is_empty() {
            issues.push(VehicleIssue::error("the vehicle has no parts", None));
            return issues;
        }
        let roots: Vec<&PlacedPart> = self.parts.iter().filter(|p| p.attach.is_none()).collect();
        match roots.len() {
            0 => issues.push(VehicleIssue::error(
                "the vehicle has no root part (the assembly is not attached)",
                None,
            )),
            1 => {}
            count => issues.push(VehicleIssue::error(
                format!("the vehicle has {count} root parts; exactly one is allowed"),
                None,
            )),
        }
        let mut ids = BTreeSet::new();
        for part in &self.parts {
            if !ids.insert(part.id.as_str()) {
                issues.push(VehicleIssue::error(
                    format!("duplicate part id '{}'", part.id),
                    Some(part.id.clone()),
                ));
            }
        }
        let mut occupied: BTreeMap<(String, NodeId), String> = BTreeMap::new();
        for part in &self.parts {
            if let Some(attach) = &part.attach {
                if self.part(&attach.parent).is_none() {
                    issues.push(VehicleIssue::error(
                        format!("{} is attached to unknown part {}", part.id, attach.parent),
                        Some(part.id.clone()),
                    ));
                }
                let key = (attach.parent.clone(), attach.node);
                if let Some(other) = occupied.insert(key, part.id.clone()) {
                    issues.push(VehicleIssue::error(
                        format!(
                            "{} and {other} both use {} node {}",
                            part.id,
                            attach.parent,
                            attach.node.label()
                        ),
                        Some(part.id.clone()),
                    ));
                }
            }
            if let Err(error) = part.spec.validate() {
                issues.push(VehicleIssue::error(
                    format!("{}: {error}", part.id),
                    Some(part.id.clone()),
                ));
            }
        }
        // Cycles: walk up from every part.
        for part in &self.parts {
            let mut seen = BTreeSet::new();
            let mut current = part.attach.as_ref().map(|a| a.parent.clone());
            while let Some(parent) = current {
                if parent == part.id {
                    issues.push(VehicleIssue::error(
                        format!("{} is attached in a cycle", part.id),
                        Some(part.id.clone()),
                    ));
                    break;
                }
                if !seen.insert(parent.clone()) {
                    break;
                }
                current = self
                    .part(&parent)
                    .and_then(|p| p.attach.as_ref())
                    .map(|a| a.parent.clone());
            }
        }
        let depths = self.stage_depths();
        let stage_count = self.stage_count();
        if stage_count > MAX_STAGES {
            issues.push(VehicleIssue::error(
                format!("the vehicle has {stage_count} stages; at most {MAX_STAGES} are allowed"),
                None,
            ));
        }
        let tanks: BTreeSet<usize> = self
            .parts
            .iter()
            .filter(|p| matches!(p.spec, PartSpec::FuelTank { .. }))
            .filter_map(|p| depths.get(&p.id).copied())
            .collect();
        for part in &self.parts {
            if matches!(part.spec, PartSpec::LiquidEngine { .. } | PartSpec::AirBreathing { .. }) {
                let stage = depths.get(&part.id).copied().unwrap_or(0);
                if !tanks.contains(&stage) {
                    issues.push(VehicleIssue::error(
                        format!("{} (stage {}): needs a fuel tank in the same stage", part.id, stage + 1),
                        Some(part.id.clone()),
                    ));
                }
            }
        }
        let computers: Vec<&PlacedPart> = self
            .parts
            .iter()
            .filter(|p| matches!(p.spec, PartSpec::Computer { .. }))
            .collect();
        match computers.as_slice() {
            [] => issues.push(VehicleIssue::error("the vehicle needs a flight computer", None)),
            [computer] => {
                let stage = depths.get(&computer.id).copied().unwrap_or(0);
                if stage + 1 != stage_count {
                    issues.push(VehicleIssue::error(
                        format!(
                            "{} would be dropped with stage {}: put the computer in the last stage",
                            computer.id,
                            stage + 1
                        ),
                        Some(computer.id.clone()),
                    ));
                }
            }
            _ => issues.push(VehicleIssue::error("only one flight computer is allowed", None)),
        }
        for part in &self.parts {
            if !part.spec.is_aero() {
                continue;
            }
            if part.attach.as_ref().is_some_and(|attach| !attach.node.is_radial()) {
                issues.push(VehicleIssue::error(
                    format!("{}: aerodynamic surfaces mount on a radial node", part.id),
                    Some(part.id.clone()),
                ));
            }
            if self
                .parts
                .iter()
                .any(|child| child.attach.as_ref().is_some_and(|a| a.parent == part.id))
            {
                issues.push(VehicleIssue::error(
                    format!("{} is a surface and cannot carry parts", part.id),
                    Some(part.id.clone()),
                ));
            }
            if let PartSpec::ControlSurface { kind, .. } = part.spec
                && let Some(needed) = crate::aero::required_computer(kind)
                && let Some(class) = self.computer()
                && class.rank() < needed.rank()
            {
                issues.push(VehicleIssue::error(
                    format!(
                        "{}: {} control surfaces need a {needed:?} flight computer",
                        part.id,
                        kind.label()
                    ),
                    Some(part.id.clone()),
                ));
            }
        }
        issues
    }

    /// Non-blocking design notes: a diameter step between a part and its mount, plus the
    /// aerodynamic notes. Kept separate from `validate` so they never refuse an assembly.
    pub fn warning_details(&self) -> Vec<VehicleWarning> {
        self.warning_issues()
            .into_iter()
            .map(|issue| VehicleWarning {
                message: issue.message,
                part: issue.part,
            })
            .collect()
    }

    fn warning_issues(&self) -> Vec<VehicleIssue> {
        let mut warnings = Vec::new();
        for part in &self.parts {
            let Some(attach) = &part.attach else {
                continue;
            };
            let Some(parent) = self.part(&attach.parent) else {
                continue;
            };
            // Fins and wings are plates on the body, not a diameter step; their own fit warning is
            // in `aero::warnings`.
            if parent.spec.is_aero() || part.spec.is_aero() {
                continue;
            }
            let (parent_d, child_d) = (node_diameter(&parent.spec), node_diameter(&part.spec));
            if (parent_d - child_d).abs() > 0.051 {
                warnings.push(VehicleIssue::warning(
                    format!(
                        "{} ({child_d:.2} m) sits on {} ({parent_d:.2} m): diameter step",
                        part.id, attach.parent
                    ),
                    Some(part.id.clone()),
                ));
            }
        }
        warnings.extend(
            crate::aero::warning_details(self)
                .into_iter()
                .map(|warning| VehicleIssue::warning(warning.message, warning.part)),
        );
        warnings
    }

    /// The same notes as plain text, for callers that only print them.
    pub fn warnings(&self) -> Vec<String> {
        self.warning_details()
            .into_iter()
            .map(|warning| warning.message)
            .collect()
    }

    /// Abstract aerodynamic figures for the whole vehicle; one pass, computed once per edit.
    pub fn aero(&self) -> crate::aero::AeroFigures {
        crate::aero::figures(self)
    }

    pub fn computer(&self) -> Option<ComputerClass> {
        self.parts.iter().find_map(|part| match part.spec {
            PartSpec::Computer { class } => Some(class),
            _ => None,
        })
    }

    pub fn best_radar(&self, min_stage: usize) -> Option<RadarSpec> {
        let depths = self.stage_depths();
        self.parts
            .iter()
            .filter(|part| depths.get(&part.id).copied().unwrap_or(0) >= min_stage)
            .filter_map(|part| match part.spec {
                PartSpec::Sensor {
                    kind: SensorKind::Radar(class),
                } => Some(class.spec()),
                _ => None,
            })
            .max_by(|a, b| a.base_lock_probability.total_cmp(&b.base_lock_probability))
    }

    pub fn available_blocks(&self) -> BTreeSet<&'static str> {
        let mut blocks: BTreeSet<&'static str> = ["wait", "ignite_stage", "set_throttle"].into_iter().collect();
        if self.stage_count() >= 2 {
            blocks.insert("separate_stage");
        }
        for part in &self.parts {
            if let PartSpec::Sensor { kind } = part.spec {
                match kind {
                    SensorKind::Pitot => blocks.insert("hold_speed"),
                    SensorKind::Barometer => blocks.insert("hold_pressure"),
                    SensorKind::Radar(_) => blocks.insert("lock_target"),
                };
            }
        }
        blocks
    }

    pub fn missing_research(&self, owned: &BTreeSet<String>) -> Vec<String> {
        let needed: BTreeSet<&str> = self
            .parts
            .iter()
            .flat_map(|part| std::iter::once(part.spec.required_capability()).chain(part.spec.extra_capabilities()))
            .collect();
        needed
            .into_iter()
            .filter(|capability| !owned.contains(*capability))
            .map(str::to_string)
            .collect()
    }

    /// Largest body diameter. Aerodynamic surfaces span outward and are not part of the body
    /// cross-section, so they are excluded from the reference area.
    pub fn max_diameter_m(&self) -> f64 {
        self.parts
            .iter()
            .filter(|part| !part.spec.is_aero())
            .map(|part| dimensions(&part.spec).1 * 2.0)
            .fold(0.0, f64::max)
            .max(0.1)
    }

    fn stage_wet_mass(stage: &Stage) -> f64 {
        stage
            .parts
            .iter()
            .map(|part| {
                let stats = part.spec.stats();
                stats.dry_mass_kg + stats.propellant_kg
            })
            .sum()
    }

    pub fn total_wet_mass_kg(&self) -> f64 {
        self.stage_views().iter().map(Self::stage_wet_mass).sum()
    }

    /// Same formulas and constants as the flat model: a stage's ignition mass includes every stage
    /// above it, and the reports come out in firing order.
    pub fn stage_reports(&self) -> Vec<StageReport> {
        let stages = self.stage_views();
        let mut reports = Vec::new();
        for (index, stage) in stages.iter().enumerate() {
            let above: f64 = stages[index + 1..].iter().map(Self::stage_wet_mass).sum();
            let wet = Self::stage_wet_mass(stage);
            let ignition_mass = wet + above;
            let mut thrust = 0.0;
            let mut propellant = 0.0;
            let mut mass_flow_weight = 0.0;
            let mut burn_time: Option<f64> = None;
            for part in &stage.parts {
                let stats = part.spec.stats();
                thrust += stats.thrust_n;
                propellant += stats.propellant_kg;
                if stats.thrust_n > 0.0 && stats.isp_s > 0.0 {
                    mass_flow_weight += stats.thrust_n / (stats.isp_s * G0);
                }
                if let Some(time) = stats.burn_time_s {
                    burn_time = Some(burn_time.map_or(time, |current: f64| current.max(time)));
                }
            }
            let isp = if mass_flow_weight > 0.0 {
                thrust / (mass_flow_weight * G0)
            } else {
                0.0
            };
            let burnout_mass = (ignition_mass - propellant).max(1e-9);
            let delta_v = if isp > 0.0 {
                isp * G0 * (ignition_mass / burnout_mass).ln()
            } else {
                0.0
            };
            reports.push(StageReport {
                index,
                stage_wet_mass_kg: wet,
                ignition_mass_kg: ignition_mass,
                thrust_n: thrust,
                burn_time_s: burn_time,
                twr_at_ignition: thrust / (ignition_mass * G0),
                delta_v_ms: delta_v,
            });
        }
        reports
    }

    /// 3D positions of every part, derived from the attachment tree.
    pub fn poses(&self) -> Vec<PartPose> {
        let depths = self.stage_depths();
        let mut poses = Vec::new();
        let Some(root) = self.root() else {
            return poses;
        };
        let (height, radius) = dimensions(&root.spec);
        let mut stack = vec![(root.id.clone(), (height / 2.0, 0.0, 0.0, height, radius))];
        let mut seen = BTreeSet::new();
        while let Some((id, (y, x, z, parent_height, parent_radius))) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            let Some(part) = self.part(&id) else {
                continue;
            };
            let (height, radius) = dimensions(&part.spec);
            poses.push(PartPose {
                id: id.clone(),
                stage: depths.get(&id).copied().unwrap_or(0),
                y_m: y,
                x_m: x,
                z_m: z,
                height_m: height,
                radius_m: radius,
            });
            for child in self.children(&id) {
                let (child_height, child_radius) = dimensions(&child.spec);
                let pose = match child.attach.as_ref().map(|a| a.node) {
                    Some(NodeId::Top) => (
                        y + parent_height / 2.0 + child_height / 2.0,
                        x,
                        z,
                        child_height,
                        child_radius,
                    ),
                    Some(NodeId::Bottom) => (
                        y - parent_height / 2.0 - child_height / 2.0,
                        x,
                        z,
                        child_height,
                        child_radius,
                    ),
                    Some(NodeId::Radial(index)) => {
                        let angle = f64::from(index) * 90.0 + child.attach.as_ref().map_or(0.0, |a| a.angle_deg);
                        let angle = angle.to_radians();
                        let offset = parent_radius + child_radius;
                        (
                            y,
                            x + angle.cos() * offset,
                            z + angle.sin() * offset,
                            child_height,
                            child_radius,
                        )
                    }
                    None => (y, x, z, child_height, child_radius),
                };
                stack.push((child.id.clone(), pose));
            }
        }
        poses.sort_by(|a, b| a.y_m.total_cmp(&b.y_m));
        poses
    }
}

impl Vehicle {
    /// The highest part of the current assembly (top of the stack), used as the default mount
    /// when the palette adds a part without a selected node.
    pub fn topmost_id(&self) -> Option<String> {
        self.poses()
            .into_iter()
            .max_by(|a, b| (a.y_m + a.height_m / 2.0).total_cmp(&(b.y_m + b.height_m / 2.0)))
            .map(|pose| pose.id)
    }

    /// Add a part above the current stack. The first part of an empty design becomes the root.
    pub fn add_to_top(&mut self, id: impl Into<String>, spec: PartSpec) -> Result<(), String> {
        let id = id.into();
        let parent = self.topmost_id();
        self.parts.push(PlacedPart {
            id: id.clone(),
            spec,
            attach: None,
            symmetry: None,
        });
        match parent {
            Some(top) if top != id => self.attach(&id, &top, NodeId::Top, 0.0),
            _ => Ok(()),
        }
    }

    /// Add a part on a specific node of an existing part.
    pub fn add_attached(
        &mut self,
        id: impl Into<String>,
        spec: PartSpec,
        parent: &str,
        node: NodeId,
    ) -> Result<(), String> {
        let id = id.into();
        self.parts.push(PlacedPart {
            id: id.clone(),
            spec,
            attach: None,
            symmetry: None,
        });
        if let Err(error) = self.attach(&id, parent, node, 0.0) {
            // Do not leave an orphan root behind when the mount is refused.
            self.parts.retain(|part| part.id != id);
            return Err(error);
        }
        Ok(())
    }
}

/// Test helpers shared by the other rule modules.
#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn part(id: &str, spec: PartSpec) -> Part {
        Part {
            id: id.to_string(),
            spec,
        }
    }

    pub fn sounding_rocket() -> Vehicle {
        Vehicle::from_stages(
            "Test rocket",
            vec![
                Stage {
                    parts: vec![
                        part(
                            "booster",
                            PartSpec::SolidMotor {
                                length_m: 2.0,
                                diameter_m: 0.4,
                                burn_time_s: 20.0,
                            },
                        ),
                        part("dec1", PartSpec::Decoupler),
                    ],
                },
                Stage {
                    parts: vec![
                        part(
                            "sustainer",
                            PartSpec::SolidMotor {
                                length_m: 1.0,
                                diameter_m: 0.3,
                                burn_time_s: 25.0,
                            },
                        ),
                        part(
                            "pc",
                            PartSpec::Computer {
                                class: ComputerClass::Basic,
                            },
                        ),
                        part(
                            "pitot",
                            PartSpec::Sensor {
                                kind: SensorKind::Pitot,
                            },
                        ),
                        part("module", PartSpec::MissionModule { mass_kg: 40.0 }),
                    ],
                },
            ],
        )
    }

    fn two_stage_without_decoupler() -> Vehicle {
        Vehicle::from_stages(
            "Migration rocket",
            vec![
                Stage {
                    parts: vec![part(
                        "booster",
                        PartSpec::SolidMotor {
                            length_m: 2.0,
                            diameter_m: 0.4,
                            burn_time_s: 20.0,
                        },
                    )],
                },
                Stage {
                    parts: vec![
                        part(
                            "sustainer",
                            PartSpec::SolidMotor {
                                length_m: 1.0,
                                diameter_m: 0.3,
                                burn_time_s: 25.0,
                            },
                        ),
                        part(
                            "pc",
                            PartSpec::Computer {
                                class: ComputerClass::Basic,
                            },
                        ),
                    ],
                },
            ],
        )
    }

    #[test]
    fn a_migrated_stack_keeps_its_stages_and_figures() {
        let vehicle = sounding_rocket();
        assert_eq!(vehicle.stage_count(), 2);
        assert_eq!(
            vehicle
                .parts()
                .into_iter()
                .map(|(_, part)| part.id.as_str())
                .collect::<Vec<_>>(),
            vec!["booster", "dec1", "sustainer", "pc", "pitot", "module"]
        );
        assert!(vehicle.validate().is_ok(), "{:?}", vehicle.validate());
        let reports = vehicle.stage_reports();
        assert_eq!(reports.len(), 2);
        assert!(reports[0].ignition_mass_kg > reports[0].stage_wet_mass_kg);
        assert_eq!(reports[0].burn_time_s, Some(20.0));
        assert!(reports[1].twr_at_ignition > reports[1].thrust_n / (reports[0].ignition_mass_kg * G0));
        let legacy = two_stage_without_decoupler();
        assert_eq!(legacy.stage_count(), 2, "a separation edge keeps the cut");
        assert!(legacy.validate().is_ok(), "{:?}", legacy.validate());
    }

    #[test]
    fn radial_attachment_and_symmetry_slots() {
        let mut vehicle = sounding_rocket();
        let parent = "booster";
        let slots = vehicle.symmetry_slots(
            parent,
            &PartSpec::SolidMotor {
                length_m: 1.0,
                diameter_m: 0.2,
                burn_time_s: 10.0,
            },
            0,
            4,
        );
        assert_eq!(slots.len(), 4);
        for (step, (node, _)) in slots.iter().enumerate() {
            let id = format!("booster-{step}");
            vehicle.parts.push(PlacedPart {
                id: id.clone(),
                spec: PartSpec::SolidMotor {
                    length_m: 1.0,
                    diameter_m: 0.2,
                    burn_time_s: 10.0,
                },
                attach: None,
                symmetry: Some("boosters".into()),
            });
            vehicle.attach(&id, parent, *node, 0.0).unwrap();
        }
        assert!(vehicle.validate().is_ok(), "{:?}", vehicle.validate());
        let poses = vehicle.poses();
        let radii: Vec<f64> = poses
            .iter()
            .filter(|p| p.id.starts_with("booster-"))
            .map(|p| (p.x_m * p.x_m + p.z_m * p.z_m).sqrt())
            .collect();
        assert_eq!(radii.len(), 4);
        assert!(radii.iter().all(|r| *r > 0.0), "{radii:?}");
    }

    #[test]
    fn cycles_occupied_nodes_and_self_attachment_are_refused() {
        let mut vehicle = sounding_rocket();
        assert!(vehicle.attach("booster", "booster", NodeId::Top, 0.0).is_err());
        assert!(
            vehicle.attach("booster", "sustainer", NodeId::Top, 0.0).is_err(),
            "cycle"
        );
        let refused = vehicle.attach("module", "pitot", NodeId::Top, 0.0).unwrap_err();
        assert!(refused.contains("already"), "{refused}");
        let taken = vehicle.attach("pc", "sustainer", NodeId::Top, 0.0).unwrap_err();
        assert!(taken.contains("already used"), "{taken}");
        assert!(vehicle.validate().is_ok());
    }

    #[test]
    fn a_computer_left_in_a_lower_stage_is_refused() {
        let mut vehicle = sounding_rocket();
        vehicle.attach("pc", "booster", NodeId::Bottom, 0.0).unwrap();
        let issues = vehicle.validate().unwrap_err().join(" | ");
        assert!(issues.contains("last stage"), "{issues}");
    }

    #[test]
    fn an_engine_needs_a_tank_in_its_own_stage() {
        let mut vehicle = two_stage_without_decoupler();
        vehicle.parts.push(PlacedPart {
            id: "engine".into(),
            spec: PartSpec::LiquidEngine { scale: 1.0 },
            attach: Some(Attach {
                parent: "booster".into(),
                node: NodeId::Radial(0),
                angle_deg: 0.0,
                separation: false,
            }),
            symmetry: None,
        });
        let issues = vehicle.validate().unwrap_err().join(" | ");
        assert!(issues.contains("fuel tank"), "{issues}");
    }

    #[test]
    fn the_tree_view_walks_depth_first_with_nodes_and_stages() {
        let vehicle = sounding_rocket();
        let rows = vehicle.tree_entries();
        assert_eq!(rows.len(), vehicle.parts.len(), "every part appears once");
        let root = rows.first().unwrap();
        assert_eq!(root.id, "booster");
        assert_eq!(root.depth, 0);
        assert_eq!(root.stage, 0);
        assert_eq!(root.node, None);

        let decoupler = rows.iter().find(|row| row.id == "dec1").unwrap();
        assert_eq!(decoupler.depth, 1);
        assert_eq!(decoupler.stage, 0, "a decoupler belongs to the stage below it");
        assert_eq!(decoupler.node, Some(NodeId::Top));

        let sustainer = rows.iter().find(|row| row.id == "sustainer").unwrap();
        assert_eq!(sustainer.depth, 2);
        assert_eq!(sustainer.stage, 1, "the decoupler starts the next stage");
        assert_eq!(sustainer.parent.as_deref(), Some("dec1"));

        let module = rows.iter().find(|row| row.id == "module").unwrap();
        assert!(module.depth > sustainer.depth, "a payload sits further up the chain");
        assert!(module.stage >= sustainer.stage);
    }

    #[test]
    fn radial_parts_appear_under_their_parent_in_node_order() {
        let mut vehicle = sounding_rocket();
        for (index, id) in [(2u8, "fin-3"), (0u8, "fin-1"), (1u8, "fin-2")] {
            vehicle.parts.push(PlacedPart {
                id: id.into(),
                spec: PartSpec::Decoupler,
                attach: None,
                symmetry: None,
            });
            vehicle.attach(id, "booster", NodeId::Radial(index), 0.0).unwrap();
        }
        let rows = vehicle.tree_entries();
        let radial: Vec<&str> = rows
            .iter()
            .filter(|row| row.id.starts_with("fin-"))
            .map(|row| row.id.as_str())
            .collect();
        assert_eq!(radial, vec!["fin-1", "fin-2", "fin-3"]);
        assert!(
            rows.iter()
                .filter(|row| row.id.starts_with("fin-"))
                .all(|row| row.depth == 1)
        );
    }

    #[test]
    fn issues_are_structured_and_name_the_part_to_fix() {
        let mut vehicle = sounding_rocket();
        vehicle.attach("pc", "booster", NodeId::Bottom, 0.0).unwrap();
        let issues = vehicle.issues();
        let error = issues
            .iter()
            .find(|issue| issue.message.contains("last stage"))
            .expect("the computer in the lower stage is an error");
        assert_eq!(error.severity, IssueSeverity::Error);
        assert_eq!(error.part.as_deref(), Some("pc"));
        assert!(
            issues
                .iter()
                .any(|issue| issue.severity == IssueSeverity::Warning && issue.part.is_some()),
            "warnings carry their part too"
        );
        let errors: Vec<String> = issues
            .iter()
            .filter(|issue| issue.severity == IssueSeverity::Error)
            .map(|issue| issue.message.clone())
            .collect();
        assert_eq!(vehicle.validate().unwrap_err(), errors, "the error view is unchanged");
    }

    #[test]
    fn a_diameter_step_is_a_warning_not_an_error() {
        let vehicle = sounding_rocket();
        assert!(vehicle.validate().is_ok());
        assert!(
            vehicle.warnings().iter().any(|w| w.contains("diameter step")),
            "{:?}",
            vehicle.warnings()
        );
    }

    #[test]
    fn removing_a_subtree_removes_everything_above_it() {
        let mut vehicle = sounding_rocket();
        let removed = vehicle.remove_subtree("sustainer");
        assert_eq!(removed.len(), 4, "{removed:?}");
        assert_eq!(vehicle.parts.len(), 2);
        assert!(vehicle.validate().is_err(), "the computer is gone");
    }

    #[test]
    fn saved_designs_round_trip_and_legacy_files_migrate() {
        let vehicle = sounding_rocket();
        let json = serde_json::to_string(&vehicle).unwrap();
        assert!(json.contains("\"schema_version\":4"), "{json}");
        let read: Vehicle = serde_json::from_str(&json).unwrap();
        assert_eq!(read, vehicle);

        let legacy = r#"{
            "name": "Old stack",
            "stages": [
                {"parts": [{"id": "m1", "spec": {"part": "solid_motor", "length_m": 2.0, "diameter_m": 0.4, "burn_time_s": 20.0}}, {"id": "d1", "spec": {"part": "decoupler"}}]},
                {"parts": [{"id": "m2", "spec": {"part": "solid_motor", "length_m": 1.0, "diameter_m": 0.3, "burn_time_s": 25.0}}, {"id": "c1", "spec": {"part": "computer", "class": "basic"}}]}
            ]
        }"#;
        let migrated: Vehicle = serde_json::from_str(legacy).unwrap();
        assert_eq!(migrated.name, "Old stack");
        assert_eq!(migrated.stage_count(), 2);
        assert!(migrated.validate().is_ok(), "{:?}", migrated.validate());

        let future = r#"{"schema_version": 99, "name": "Future", "parts": []}"#;
        let refused = serde_json::from_str::<Vehicle>(future).unwrap_err();
        assert!(refused.to_string().contains("newer"), "{refused}");
    }

    fn wing() -> PartSpec {
        PartSpec::Wing {
            span_m: 1.2,
            chord_m: 0.4,
            sweep_deg: 15.0,
            planform: crate::vehicle::WingPlanform::Swept,
            profile: crate::vehicle::AirfoilProfile::Classic,
            light: false,
        }
    }

    #[test]
    fn aerodynamic_surfaces_mount_on_radial_nodes_and_carry_nothing() {
        let mut vehicle = sounding_rocket();
        assert!(
            vehicle.add_attached("wing", wing(), "booster", NodeId::Top).is_err(),
            "a wing cannot sit on the axis"
        );
        vehicle
            .add_attached("wing", wing(), "booster", NodeId::Radial(0))
            .unwrap();
        assert!(vehicle.validate().is_ok(), "{:?}", vehicle.validate());
        assert!(
            vehicle
                .node_status("wing", NodeId::Radial(1), &PartSpec::Decoupler)
                .unwrap_err()
                .contains("cannot carry"),
            "a surface is a leaf"
        );
        assert!(vehicle.attach("module", "wing", NodeId::Radial(1), 0.0).is_err());
    }

    #[test]
    fn a_surface_set_uses_the_parent_body_slots() {
        let vehicle = sounding_rocket();
        let slots = vehicle.symmetry_slots("booster", &wing(), 0, 4);
        assert_eq!(slots.len(), 4, "a body offers four slots to a wing");
        let decoupler = vehicle.symmetry_slots("dec1", &wing(), 0, 4);
        assert_eq!(decoupler.len(), 1, "a decoupler has no radial mount");
    }

    #[test]
    fn schema_three_still_loads_and_is_rewritten_as_four() {
        let old = r#"{
            "schema_version": 3,
            "name": "Schema three",
            "parts": [
                {"id": "m", "spec": {"part": "solid_motor", "length_m": 2.0, "diameter_m": 0.4, "burn_time_s": 20.0}},
                {"id": "pc", "spec": {"part": "computer", "class": "basic"}, "attach": {"parent": "m", "node": "top"}}
            ]
        }"#;
        let vehicle: Vehicle = serde_json::from_str(old).unwrap();
        assert_eq!(vehicle.name, "Schema three");
        assert!(vehicle.validate().is_ok(), "{:?}", vehicle.validate());
        assert_eq!(vehicle.aero().surface_count, 0);
        let written = serde_json::to_string(&vehicle).unwrap();
        assert!(written.contains("\"schema_version\":4"), "{written}");

        let future = r#"{"schema_version": 5, "name": "Future", "parts": []}"#;
        let refused = serde_json::from_str::<Vehicle>(future).unwrap_err();
        assert!(refused.to_string().contains("newer"), "{refused}");
    }

    #[test]
    fn schema_four_surfaces_round_trip_with_their_design_choices() {
        let mut vehicle = sounding_rocket();
        vehicle
            .add_attached("wing", wing(), "booster", NodeId::Radial(0))
            .unwrap();
        vehicle
            .add_attached(
                "fin",
                PartSpec::FinSet {
                    span_m: 0.3,
                    chord_m: 0.35,
                    role: crate::vehicle::StabilizerRole::Rear,
                    layout: crate::vehicle::FinLayout::Asymmetric,
                    light: true,
                },
                "booster",
                NodeId::Radial(1),
            )
            .unwrap();
        let json = serde_json::to_string(&vehicle).unwrap();
        assert!(json.contains("\"schema_version\":4"), "{json}");
        assert!(!json.contains("variable_geometry"));
        let read: Vehicle = serde_json::from_str(&json).unwrap();
        assert_eq!(read, vehicle);
        assert_eq!(read.aero().surface_count, 2);
        assert!(read.aero().mass_kg > 0.0);
    }

    #[test]
    fn a_moving_control_surface_needs_a_compatible_computer() {
        let mut vehicle = sounding_rocket();
        vehicle
            .add_attached(
                "ctrl",
                PartSpec::ControlSurface {
                    span_m: 0.4,
                    chord_m: 0.4,
                    kind: crate::vehicle::ControlKind::Combined,
                    light: false,
                },
                "booster",
                NodeId::Radial(0),
            )
            .unwrap();
        let issues = vehicle.validate().unwrap_err().join(" | ");
        assert!(issues.contains("Advanced"), "{issues}");
        vehicle.replace_part(
            "pc",
            PartSpec::Computer {
                class: ComputerClass::Advanced,
            },
        );
        assert!(vehicle.validate().is_ok(), "{:?}", vehicle.validate());
    }
}
