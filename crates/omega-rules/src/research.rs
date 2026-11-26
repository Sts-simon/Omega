//! Research tree: technologies with costs, durations and prerequisites that grant capabilities
//! other facilities check (parts, analyses, limits). One project at a time, so the order of
//! research is a strategic choice. All values are abstract gameplay numbers.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Department {
    VehicleFamilies,
    Materials,
    Aerodynamics,
    Sensors,
    Computing,
    DesignTools,
}

impl Department {
    pub const ALL: [Self; 6] = [
        Self::VehicleFamilies,
        Self::Materials,
        Self::Aerodynamics,
        Self::Sensors,
        Self::Computing,
        Self::DesignTools,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::VehicleFamilies => "MISSILE FAMILIES",
            Self::Materials => "PROPULSION & STRUCTURES",
            Self::Aerodynamics => "AERODYNAMICS",
            Self::Sensors => "SENSORS & ELECTRONICS",
            Self::Computing => "ONBOARD COMPUTING",
            Self::DesignTools => "DESIGN & TEST TOOLS",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "grant", content = "value", rename_all = "snake_case")]
pub enum Grant {
    /// A named capability checked by other facilities, e.g. `part:cylinder` or `analysis:truss`.
    Capability(String),
    /// Raises the number of simultaneously active contracts.
    ExtraActiveProjects(u8),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TechNode {
    pub id: String,
    pub department: Department,
    pub label: String,
    pub description: String,
    pub cost_cr: u64,
    pub duration_days: u16,
    pub prereqs: Vec<String>,
    pub grants: Vec<Grant>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveResearch {
    pub id: String,
    pub days_left: u16,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchState {
    pub completed: Vec<String>,
    pub active: Option<ActiveResearch>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResearchError {
    UnknownTech(String),
    AlreadyCompleted,
    ResearchBusy(String),
    MissingPrerequisite(String),
    InsufficientCredits { needed: u64, available: u64 },
}

impl std::fmt::Display for ResearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownTech(id) => write!(f, "unknown technology '{id}'"),
            Self::AlreadyCompleted => write!(f, "already researched"),
            Self::ResearchBusy(id) => write!(f, "'{id}' is already being researched"),
            Self::MissingPrerequisite(id) => write!(f, "requires '{id}' first"),
            Self::InsufficientCredits { needed, available } => {
                write!(f, "needs {needed} cr, you have {available} cr")
            }
        }
    }
}

/// Check ids are unique, prerequisites exist, nothing requires itself and there are no cycles.
pub fn validate_tree(tree: &[TechNode]) -> Result<(), String> {
    let mut index: HashMap<&str, &TechNode> = HashMap::new();
    for node in tree {
        if index.insert(node.id.as_str(), node).is_some() {
            return Err(format!("duplicate technology id '{}'", node.id));
        }
    }
    for node in tree {
        for prereq in &node.prereqs {
            if !index.contains_key(prereq.as_str()) {
                return Err(format!("'{}' requires unknown technology '{prereq}'", node.id));
            }
        }
    }
    // Depth-first cycle detection.
    fn visit<'a>(
        id: &'a str,
        index: &HashMap<&'a str, &'a TechNode>,
        state: &mut HashMap<&'a str, u8>,
    ) -> Result<(), String> {
        match state.get(id) {
            Some(2) => return Ok(()),
            Some(1) => return Err(format!("prerequisite cycle through '{id}'")),
            _ => {}
        }
        state.insert(id, 1);
        for prereq in &index[id].prereqs {
            visit(prereq.as_str(), index, state)?;
        }
        state.insert(id, 2);
        Ok(())
    }
    let mut state = HashMap::new();
    for node in tree {
        visit(node.id.as_str(), &index, &mut state)?;
    }
    Ok(())
}

impl ResearchState {
    pub fn is_completed(&self, id: &str) -> bool {
        self.completed.iter().any(|c| c == id)
    }

    pub fn can_start(&self, tree: &[TechNode], id: &str, credits: u64) -> Result<(), ResearchError> {
        let node = tree
            .iter()
            .find(|n| n.id == id)
            .ok_or_else(|| ResearchError::UnknownTech(id.to_string()))?;
        if self.is_completed(id) {
            return Err(ResearchError::AlreadyCompleted);
        }
        if let Some(active) = &self.active {
            return Err(ResearchError::ResearchBusy(active.id.clone()));
        }
        for prereq in &node.prereqs {
            if !self.is_completed(prereq) {
                return Err(ResearchError::MissingPrerequisite(prereq.clone()));
            }
        }
        if credits < node.cost_cr {
            return Err(ResearchError::InsufficientCredits {
                needed: node.cost_cr,
                available: credits,
            });
        }
        Ok(())
    }

    /// Begin researching `id`; returns the credits to deduct.
    pub fn start(&mut self, tree: &[TechNode], id: &str, credits: u64) -> Result<u64, ResearchError> {
        self.can_start(tree, id, credits)?;
        let node = tree.iter().find(|n| n.id == id).expect("checked by can_start");
        self.active = Some(ActiveResearch {
            id: node.id.clone(),
            days_left: node.duration_days.max(1),
        });
        Ok(node.cost_cr)
    }

    /// Advance the clock; returns the ids completed during these days.
    pub fn advance_days(&mut self, days: u16) -> Vec<String> {
        let mut finished = Vec::new();
        if let Some(active) = &mut self.active {
            active.days_left = active.days_left.saturating_sub(days);
            if active.days_left == 0 {
                finished.push(active.id.clone());
            }
        }
        if let Some(id) = finished.first() {
            self.completed.push(id.clone());
            self.active = None;
        }
        finished
    }

    pub fn capabilities(&self, tree: &[TechNode]) -> BTreeSet<String> {
        tree.iter()
            .filter(|n| self.is_completed(&n.id))
            .flat_map(|n| n.grants.iter())
            .filter_map(|g| match g {
                Grant::Capability(name) => Some(name.clone()),
                Grant::ExtraActiveProjects(_) => None,
            })
            .collect()
    }

    /// Starting capabilities plus everything researched so far.
    pub fn owned_capabilities(&self, tree: &[TechNode]) -> BTreeSet<String> {
        let mut owned: BTreeSet<String> = starting_capabilities().iter().map(|c| (*c).to_string()).collect();
        owned.extend(self.capabilities(tree));
        owned
    }

    pub fn has_capability(&self, tree: &[TechNode], name: &str) -> bool {
        self.owned_capabilities(tree).contains(name)
    }

    pub fn max_active_projects(&self, tree: &[TechNode], base: usize) -> usize {
        let extra: usize = tree
            .iter()
            .filter(|n| self.is_completed(&n.id))
            .flat_map(|n| n.grants.iter())
            .map(|g| match g {
                Grant::ExtraActiveProjects(n) => usize::from(*n),
                Grant::Capability(_) => 0,
            })
            .sum();
        base + extra
    }
}

fn node(
    id: &str,
    dept: Department,
    label: &str,
    description: &str,
    cost: u64,
    days: u16,
    prereqs: &[&str],
    grants: Vec<Grant>,
) -> TechNode {
    TechNode {
        id: id.to_string(),
        department: dept,
        label: label.to_string(),
        description: description.to_string(),
        cost_cr: cost,
        duration_days: days,
        prereqs: prereqs.iter().map(|p| (*p).to_string()).collect(),
        grants,
    }
}

fn cap(name: &str) -> Grant {
    Grant::Capability(name.to_string())
}

/// What every design office can do from day one.
pub fn starting_capabilities() -> &'static [&'static str] {
    &[
        "part:solid_motor",
        "part:fuel_tank",
        "part:decoupler",
        "part:mission_module",
        "part:explosive",
        "part:adapter",
        "part:nose",
        "computer:basic",
        "sensor:pitot",
        "family:ballistic",
    ]
}

/// Starting tree (fictional missile programme, abstract numbers). Capability names match
/// `vehicle::PartSpec::required_capability`.
pub fn default_tree() -> Vec<TechNode> {
    use Department::*;
    vec![
        node(
            "family_surface_to_air",
            VehicleFamilies,
            "Surface-to-air family",
            "Contracts for surface-launched interceptors.",
            3_000,
            3,
            &[],
            vec![cap("family:surface_to_air")],
        ),
        node(
            "family_air_to_air",
            VehicleFamilies,
            "Air-to-air family",
            "Contracts for air-launched missiles.",
            3_500,
            4,
            &["family_surface_to_air"],
            vec![cap("family:air_to_air")],
        ),
        node(
            "family_cruise",
            VehicleFamilies,
            "Cruise family",
            "Long-endurance low-flying vehicles.",
            4_000,
            4,
            &["turbofans"],
            vec![cap("family:cruise")],
        ),
        node(
            "family_anti_ship",
            VehicleFamilies,
            "Anti-ship family",
            "Contracts for sea-skimming vehicles.",
            4_500,
            4,
            &["family_cruise", "barometer"],
            vec![cap("family:anti_ship")],
        ),
        node(
            "liquid_engines",
            Materials,
            "Liquid engines",
            "Throttleable engines fed from tanks.",
            2_500,
            3,
            &[],
            vec![cap("part:liquid_engine")],
        ),
        node(
            "turbofans",
            Materials,
            "Turbofan engines",
            "Economical engines that need airspeed to run.",
            4_000,
            4,
            &["liquid_engines"],
            vec![cap("part:air_breathing")],
        ),
        node(
            "improved_explosives",
            Materials,
            "Improved explosive charges",
            "Unlocks the improved charge: more game yield for the same payload mass.",
            3_000,
            2,
            &[],
            vec![cap("part:explosive_improved")],
        ),
        node(
            "advanced_explosives",
            Materials,
            "Advanced explosive charges",
            "Unlocks the advanced charge, the most mass-efficient payload package.",
            5_000,
            3,
            &["improved_explosives"],
            vec![cap("part:explosive_advanced")],
        ),
        node(
            "aerodynamics",
            Aerodynamics,
            "Aerodynamics",
            "Unlocks wings for cruise airframes.",
            2_200,
            3,
            &["liquid_engines"],
            vec![cap("part:wing")],
        ),
        node(
            "fin_stabilisation",
            Aerodynamics,
            "Fin stabilisation",
            "Unlocks fin sets for passive stability.",
            1_800,
            2,
            &["aerodynamics"],
            vec![cap("part:fin")],
        ),
        node(
            "control_surfaces",
            Aerodynamics,
            "Control surfaces",
            "Unlocks moving surfaces; integrated and combined sets need a better computer.",
            3_000,
            4,
            &["aerodynamics", "computer_standard"],
            vec![cap("part:control_surface")],
        ),
        node(
            "advanced_aero_profiles",
            Aerodynamics,
            "Advanced profiles",
            "Experimental profiles, variable geometry and asymmetric fin sets.",
            4_200,
            4,
            &["fin_stabilisation", "control_surfaces"],
            vec![cap("aero:advanced_profiles")],
        ),
        node(
            "light_aero_structures",
            Aerodynamics,
            "Light aero structures",
            "Lighter surfaces at the cost of complexity and reliability.",
            2_600,
            3,
            &["advanced_aero_profiles"],
            vec![cap("aero:light_structures")],
        ),
        node(
            "barometer",
            Sensors,
            "Barometric altimeter",
            "Unlocks the Hold pressure block.",
            1_200,
            2,
            &[],
            vec![cap("sensor:barometer")],
        ),
        node(
            "radar_basic",
            Sensors,
            "Basic radar",
            "Unlocks the Lock target block (limited speed).",
            3_000,
            4,
            &[],
            vec![cap("sensor:radar_basic")],
        ),
        node(
            "radar_active",
            Sensors,
            "Active radar",
            "Better lock chance and a higher speed limit.",
            6_000,
            5,
            &["radar_basic"],
            vec![cap("sensor:radar_active")],
        ),
        node(
            "computer_standard",
            Computing,
            "Standard flight computer",
            "Two threads and more memory.",
            2_500,
            3,
            &[],
            vec![cap("computer:standard")],
        ),
        node(
            "computer_advanced",
            Computing,
            "Advanced flight computer",
            "Four threads and a large memory.",
            5_500,
            5,
            &["computer_standard"],
            vec![cap("computer:advanced")],
        ),
        node(
            "flight_control_electronics",
            Computing,
            "Flight control electronics",
            "Mission modules for control, power and self-test.",
            6_500,
            9,
            &["computer_standard"],
            vec![
                cap("avionics:control"),
                cap("avionics:power"),
                cap("avionics:diagnostics"),
            ],
        ),
        node(
            "navigation_datalink",
            Sensors,
            "Navigation and datalink",
            "Navigation sources and the relay that shares their target solution.",
            7_500,
            10,
            &["radar_basic"],
            vec![cap("avionics:navigation"), cap("avionics:datalink")],
        ),
        node(
            "parallel_projects",
            DesignTools,
            "Parallel project management",
            "One extra active contract.",
            6_000,
            6,
            &[],
            vec![Grant::ExtraActiveProjects(1)],
        ),
        node(
            "monte_carlo_trials",
            DesignTools,
            "Batch trials",
            "Run many seeded tests at once and compare results.",
            3_500,
            4,
            &[],
            vec![cap("analysis:monte_carlo")],
        ),
    ]
}

/// The technology that grants a capability, so a refusal can name what to research instead of an
/// internal id such as `family:surface_to_air`.
pub fn technology_granting(capability: &str) -> Option<TechNode> {
    default_tree().into_iter().find(|node| {
        node.grants
            .iter()
            .any(|grant| matches!(grant, Grant::Capability(name) if name == capability))
    })
}

/// Readable name for a capability: the technology label, or the identifier if nothing grants it.
pub fn capability_label(capability: &str) -> String {
    technology_granting(capability).map_or_else(|| capability.to_string(), |tech| tech.label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tree_fits_a_reasonable_serial_campaign_budget() {
        let tree = default_tree();
        let cost: u64 = tree.iter().map(|node| node.cost_cr).sum();
        let days: u16 = tree.iter().map(|node| node.duration_days).sum();
        assert_eq!(cost, 85_000, "the total research cost is the campaign's credit target");
        assert!(
            days <= 95,
            "serial research must stay near the 90-day campaign target, not {days}"
        );
        let aero: u16 = tree
            .iter()
            .filter(|node| node.department == Department::Aerodynamics)
            .map(|node| node.duration_days)
            .sum();
        assert!(aero <= 16, "the aerodynamics department costs {aero} days");
    }

    #[test]
    fn default_tree_is_valid_and_covers_every_department() {
        let tree = default_tree();
        validate_tree(&tree).unwrap();
        for dept in Department::ALL {
            assert!(tree.iter().any(|n| n.department == dept), "{dept:?}");
        }
    }

    #[test]
    fn every_part_capability_a_vehicle_can_need_is_either_free_or_researchable() {
        use crate::vehicle::{ComputerClass, PartSpec, RadarClass, SensorKind};
        let tree = default_tree();
        let state = ResearchState {
            completed: tree.iter().map(|n| n.id.clone()).collect(),
            active: None,
        };
        let owned = state.owned_capabilities(&tree);
        let specs = [
            PartSpec::SolidMotor {
                length_m: 1.0,
                diameter_m: 0.3,
                burn_time_s: 10.0,
            },
            PartSpec::FuelTank {
                length_m: 1.0,
                diameter_m: 0.3,
            },
            PartSpec::LiquidEngine { scale: 1.0 },
            PartSpec::AirBreathing { scale: 1.0 },
            PartSpec::Sensor {
                kind: SensorKind::Pitot,
            },
            PartSpec::Sensor {
                kind: SensorKind::Barometer,
            },
            PartSpec::Sensor {
                kind: SensorKind::Radar(RadarClass::Basic),
            },
            PartSpec::Sensor {
                kind: SensorKind::Radar(RadarClass::Active),
            },
            PartSpec::Computer {
                class: ComputerClass::Basic,
            },
            PartSpec::Computer {
                class: ComputerClass::Standard,
            },
            PartSpec::Computer {
                class: ComputerClass::Advanced,
            },
            PartSpec::Decoupler,
            PartSpec::MissionModule { mass_kg: 10.0 },
            PartSpec::Explosive {
                mass_kg: 40.0,
                grade: crate::vehicle::ExplosiveGrade::Standard,
            },
            PartSpec::Explosive {
                mass_kg: 40.0,
                grade: crate::vehicle::ExplosiveGrade::Improved,
            },
            PartSpec::Explosive {
                mass_kg: 40.0,
                grade: crate::vehicle::ExplosiveGrade::Advanced,
            },
            PartSpec::Adapter {
                length_m: 0.4,
                diameter_m: 0.4,
                exit_diameter_m: 0.3,
                shape: crate::vehicle::AdapterShape::Conical,
            },
            PartSpec::Wing {
                span_m: 1.0,
                chord_m: 0.3,
                sweep_deg: 15.0,
                planform: crate::vehicle::WingPlanform::Swept,
                profile: crate::vehicle::AirfoilProfile::Classic,
                light: false,
            },
            PartSpec::FinSet {
                span_m: 0.3,
                chord_m: 0.3,
                role: crate::vehicle::StabilizerRole::Rear,
                layout: crate::vehicle::FinLayout::Symmetric,
                light: false,
            },
            PartSpec::ControlSurface {
                span_m: 0.3,
                chord_m: 0.3,
                kind: crate::vehicle::ControlKind::Classic,
                light: false,
            },
        ];
        for spec in specs {
            assert!(
                owned.contains(spec.required_capability()),
                "{}",
                spec.required_capability()
            );
        }
        // And the free starting set is limited: radars and engines must be researched.
        let fresh = ResearchState::default().owned_capabilities(&tree);
        assert!(!fresh.contains("sensor:radar_basic") && !fresh.contains("part:liquid_engine"));
    }

    #[test]
    fn invalid_trees_are_rejected() {
        let mut tree = default_tree();
        tree.push(tree[0].clone());
        assert!(validate_tree(&tree).unwrap_err().contains("duplicate"));

        let mut tree = default_tree();
        tree[0].prereqs.push("nope".into());
        assert!(validate_tree(&tree).unwrap_err().contains("unknown"));

        let mut tree = default_tree();
        // liquid_engines -> turbofans -> liquid_engines
        let turbofans = tree.iter().position(|n| n.id == "turbofans").unwrap();
        let liquid = tree.iter().position(|n| n.id == "liquid_engines").unwrap();
        let turbofans_id = tree[turbofans].id.clone();
        tree[liquid].prereqs.push(turbofans_id);
        assert!(validate_tree(&tree).unwrap_err().contains("cycle"));
    }

    #[test]
    fn start_requires_prerequisites_credits_and_a_free_lab() {
        let tree = default_tree();
        let mut state = ResearchState::default();
        assert_eq!(
            state.can_start(&tree, "turbofans", 99_999),
            Err(ResearchError::MissingPrerequisite("liquid_engines".into()))
        );
        assert_eq!(
            state.can_start(&tree, "liquid_engines", 100),
            Err(ResearchError::InsufficientCredits {
                needed: 2_500,
                available: 100
            })
        );
        assert_eq!(
            state.can_start(&tree, "ghost", 10_000),
            Err(ResearchError::UnknownTech("ghost".into()))
        );
        assert_eq!(state.start(&tree, "liquid_engines", 5_000), Ok(2_500));
        assert_eq!(
            state.can_start(&tree, "barometer", 5_000),
            Err(ResearchError::ResearchBusy("liquid_engines".into()))
        );
    }

    #[test]
    fn research_completes_after_its_duration_and_grants_capabilities() {
        let tree = default_tree();
        let mut state = ResearchState::default();
        assert!(!state.has_capability(&tree, "part:liquid_engine"));
        assert!(state.has_capability(&tree, "part:solid_motor"), "starting capability");
        let days = tree
            .iter()
            .find(|node| node.id == "liquid_engines")
            .unwrap()
            .duration_days;
        state.start(&tree, "liquid_engines", 5_000).unwrap();
        assert!(state.advance_days(days - 1).is_empty());
        assert_eq!(state.active.as_ref().unwrap().days_left, 1);
        assert_eq!(state.advance_days(5), vec!["liquid_engines".to_string()]);
        assert!(state.active.is_none());
        assert!(state.has_capability(&tree, "part:liquid_engine"));
        assert_eq!(
            state.can_start(&tree, "liquid_engines", 5_000),
            Err(ResearchError::AlreadyCompleted)
        );
        assert_eq!(state.can_start(&tree, "turbofans", 5_000), Ok(()));
    }

    #[test]
    fn a_design_is_blocked_until_its_research_is_done() {
        use crate::assembly::tests::sounding_rocket;
        use crate::vehicle::{PartSpec, RadarClass, SensorKind};
        let tree = default_tree();
        let mut vehicle = sounding_rocket();
        vehicle
            .add_to_top(
                "radar",
                PartSpec::Sensor {
                    kind: SensorKind::Radar(RadarClass::Basic),
                },
            )
            .unwrap();
        let mut state = ResearchState::default();
        assert_eq!(
            vehicle.missing_research(&state.owned_capabilities(&tree)),
            vec!["sensor:radar_basic".to_string()]
        );
        state.start(&tree, "radar_basic", 10_000).unwrap();
        state.advance_days(5);
        assert!(vehicle.missing_research(&state.owned_capabilities(&tree)).is_empty());
    }

    #[test]
    fn extra_projects_grant_raises_the_limit() {
        let tree = default_tree();
        let mut state = ResearchState::default();
        assert_eq!(state.max_active_projects(&tree, 2), 2);
        state.start(&tree, "parallel_projects", 10_000).unwrap();
        state.advance_days(8);
        assert_eq!(state.max_active_projects(&tree, 2), 3);
    }

    #[test]
    fn capabilities_can_be_traced_back_to_their_technology() {
        let tech = technology_granting("family:surface_to_air").expect("granted by a node");
        assert_eq!(tech.label, "Surface-to-air family");
        assert_eq!(capability_label("family:cruise"), "Cruise family");
        assert_eq!(capability_label("family:unknown"), "family:unknown");
    }

    #[test]
    fn state_round_trips_through_json() {
        let tree = default_tree();
        let mut state = ResearchState::default();
        state.start(&tree, "barometer", 5_000).unwrap();
        state.advance_days(1);
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(serde_json::from_str::<ResearchState>(&json).unwrap(), state);
    }

    #[test]
    fn the_research_tree_stays_affordable_and_spread_across_departments() {
        let tree = default_tree();
        validate_tree(&tree).unwrap();
        assert!(tree.len() >= 20);
        let total: u64 = tree.iter().map(|node| node.cost_cr).sum();
        assert!((60_000..=120_000).contains(&total), "{total}");
        for department in Department::ALL {
            let nodes: Vec<_> = tree.iter().filter(|node| node.department == department).collect();
            assert!(!nodes.is_empty(), "{department:?}");
        }
        for node in &tree {
            assert!(node.cost_cr <= 8_000, "{} costs too much", node.id);
            assert!((1..=12).contains(&node.duration_days), "{} duration", node.id);
            assert!(!node.grants.is_empty(), "{} grants nothing", node.id);
        }
        let starting = starting_capabilities();
        assert!(starting.contains(&"part:solid_motor"));
        for family in ["family:ballistic", "family:surface_to_air", "family:cruise"] {
            assert!(
                starting.contains(&family) || technology_granting(family).is_some(),
                "{family} has neither a starting grant nor a research source"
            );
        }
    }
}
