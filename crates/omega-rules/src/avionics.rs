//! Onboard electronics as one family. Every device is abstract: a mass, a paper cost, a power draw
//! or supply, a design complexity, a reliability shift and the gameplay effect it contributes.
//! Nothing here describes a real component or a real signal.
//!
//! Seven categories share the same shape: flight computers, control modules, sensors, navigation,
//! communication, power and diagnostics. The legacy computer and sensor parts are part of the
//! family through [`spec_of`], so a vehicle's electronics are always summarised in one place.
//!
//! The rules are deliberately one-way: figures aggregate the tree, and [`issues`] reports
//! incompatibilities as ordinary [`VehicleIssue`]s so the workshop's validation panel renders them
//! like every other error and warning.

use crate::assembly::{Vehicle, VehicleIssue};
use crate::vehicle::{ComputerClass, PartSpec, RadarClass, SensorKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Highest control-authority bonus a stack of modules can contribute.
pub const MAX_AUTHORITY_BONUS: f64 = 0.20;
/// Highest target-lock bonus a stack of modules can contribute.
pub const MAX_LOCK_BONUS: f64 = 0.15;
/// Highest share of the airframe fault chance the self-test suite can remove.
pub const MAX_FAULT_REDUCTION: f64 = 0.65;
/// Below this share of the supplied power the design is warned about its headroom.
pub const POWER_HEADROOM_WARN: f64 = 0.25;

/// The seven electronics categories the workshop and the rules agree on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvionicsCategory {
    Computers,
    Control,
    Sensors,
    Navigation,
    Communication,
    Power,
    Diagnostics,
}

impl AvionicsCategory {
    pub const ALL: [Self; 7] = [
        Self::Computers,
        Self::Control,
        Self::Sensors,
        Self::Navigation,
        Self::Communication,
        Self::Power,
        Self::Diagnostics,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Computers => "Computers",
            Self::Control => "Control modules",
            Self::Sensors => "Sensors",
            Self::Navigation => "Navigation",
            Self::Communication => "Communication",
            Self::Power => "Power",
            Self::Diagnostics => "Diagnostics",
        }
    }
}

/// Mission electronics that are not a computer or a sensor. Every variant is one part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvionicsModule {
    /// Augments the flight computer's command path: more authority, less reliability.
    ControlAugment,
    /// Abstract navigation source: better target solutions, so a higher lock chance.
    InertialNav,
    /// Relay module: needs navigation, improves the target solution further.
    Datalink,
    /// Supplies the mission electronics; the heaviest module and the only negative draw.
    PowerCell,
    /// Self-test suite: halves the seeded airframe fault chance when fitted.
    Diagnostics,
}

impl AvionicsModule {
    pub const ALL: [Self; 5] = [
        Self::ControlAugment,
        Self::InertialNav,
        Self::Datalink,
        Self::PowerCell,
        Self::Diagnostics,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ControlAugment => "Control augment",
            Self::InertialNav => "Inertial platform",
            Self::Datalink => "Datalink",
            Self::PowerCell => "Power cell",
            Self::Diagnostics => "Self-test suite",
        }
    }

    /// Short form used by the workshop palette, where the category heading carries the context.
    pub fn short_label(self) -> &'static str {
        match self {
            Self::ControlAugment => "CTRL AUGMENT",
            Self::InertialNav => "NAV PLATFORM",
            Self::Datalink => "DATALINK",
            Self::PowerCell => "POWER CELL",
            Self::Diagnostics => "SELF-TEST",
        }
    }

    pub fn category(self) -> AvionicsCategory {
        match self {
            Self::ControlAugment => AvionicsCategory::Control,
            Self::InertialNav | Self::Datalink => AvionicsCategory::Navigation,
            Self::PowerCell => AvionicsCategory::Power,
            Self::Diagnostics => AvionicsCategory::Diagnostics,
        }
    }

    /// Research capability needed before the workshop accepts the module.
    pub fn capability(self) -> &'static str {
        match self {
            Self::ControlAugment => "avionics:control",
            Self::InertialNav => "avionics:navigation",
            Self::Datalink => "avionics:datalink",
            Self::PowerCell => "avionics:power",
            Self::Diagnostics => "avionics:diagnostics",
        }
    }

    pub fn spec(self) -> AvionicsSpec {
        match self {
            Self::ControlAugment => AvionicsSpec {
                category: AvionicsCategory::Control,
                mass_kg: 9.0,
                cost_cr: 1_400,
                power_w: 25.0,
                complexity: 0.35,
                reliability: -0.05,
                authority_bonus: 0.15,
                lock_bonus: 0.0,
                fault_reduction: 0.0,
            },
            Self::InertialNav => AvionicsSpec {
                category: AvionicsCategory::Navigation,
                mass_kg: 7.0,
                cost_cr: 1_800,
                power_w: 18.0,
                complexity: 0.20,
                reliability: 0.05,
                authority_bonus: 0.0,
                lock_bonus: 0.06,
                fault_reduction: 0.0,
            },
            Self::Datalink => AvionicsSpec {
                category: AvionicsCategory::Communication,
                mass_kg: 4.0,
                cost_cr: 1_200,
                power_w: 12.0,
                complexity: 0.25,
                reliability: 0.0,
                authority_bonus: 0.0,
                lock_bonus: 0.04,
                fault_reduction: 0.0,
            },
            Self::PowerCell => AvionicsSpec {
                category: AvionicsCategory::Power,
                mass_kg: 14.0,
                cost_cr: 900,
                power_w: -60.0,
                complexity: 0.30,
                reliability: -0.02,
                authority_bonus: 0.0,
                lock_bonus: 0.0,
                fault_reduction: 0.0,
            },
            Self::Diagnostics => AvionicsSpec {
                category: AvionicsCategory::Diagnostics,
                mass_kg: 5.0,
                cost_cr: 1_100,
                power_w: 10.0,
                complexity: 0.20,
                reliability: 0.10,
                authority_bonus: 0.0,
                lock_bonus: 0.0,
                fault_reduction: 0.5,
            },
        }
    }

    /// The relay needs a navigation source to point at.
    pub fn needs_navigation(self) -> bool {
        matches!(self, Self::Datalink)
    }

    /// The control augment plugs into a standard command path, not a basic one.
    pub fn needs_standard_computer(self) -> bool {
        matches!(self, Self::ControlAugment)
    }
}

/// One device's abstract properties. Legacy computers and sensors fill the cost, complexity and
/// reliability fields; their mass still comes from [`PartSpec::stats`] so there is one mass reader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvionicsSpec {
    pub category: AvionicsCategory,
    pub mass_kg: f64,
    pub cost_cr: u64,
    /// Positive draws from the mission bus, negative supplies it.
    pub power_w: f64,
    pub complexity: f64,
    pub reliability: f64,
    pub authority_bonus: f64,
    pub lock_bonus: f64,
    pub fault_reduction: f64,
}

/// Height and radius of a mission module, in metres, so the 3D view and the attachment nodes agree.
pub fn dimensions(module: AvionicsModule) -> (f64, f64) {
    match module {
        AvionicsModule::ControlAugment => (0.22, 0.14),
        AvionicsModule::InertialNav => (0.18, 0.12),
        AvionicsModule::Datalink => (0.10, 0.10),
        AvionicsModule::PowerCell => (0.30, 0.15),
        AvionicsModule::Diagnostics => (0.12, 0.12),
    }
}

/// The electronics view of any part, or `None` for a part that carries no electronics.
pub fn spec_of(spec: &PartSpec) -> Option<AvionicsSpec> {
    let electronics = |category, cost_cr, complexity, reliability| AvionicsSpec {
        category,
        mass_kg: 0.0,
        cost_cr,
        power_w: 0.0,
        complexity,
        reliability,
        authority_bonus: 0.0,
        lock_bonus: 0.0,
        fault_reduction: 0.0,
    };
    match *spec {
        PartSpec::Computer { class } => Some(electronics(
            AvionicsCategory::Computers,
            match class {
                ComputerClass::Basic => 700,
                ComputerClass::Standard => 1_600,
                ComputerClass::Advanced => 3_200,
            },
            match class {
                ComputerClass::Basic => 0.10,
                ComputerClass::Standard => 0.20,
                ComputerClass::Advanced => 0.35,
            },
            match class {
                ComputerClass::Basic => 0.05,
                ComputerClass::Standard => 0.02,
                ComputerClass::Advanced => -0.05,
            },
        )),
        PartSpec::Sensor { kind } => Some(electronics(
            AvionicsCategory::Sensors,
            match kind {
                SensorKind::Pitot => 150,
                SensorKind::Barometer => 200,
                SensorKind::Radar(RadarClass::Basic) => 1_800,
                SensorKind::Radar(RadarClass::Active) => 4_200,
            },
            match kind {
                SensorKind::Pitot | SensorKind::Barometer => 0.02,
                SensorKind::Radar(RadarClass::Basic) => 0.15,
                SensorKind::Radar(RadarClass::Active) => 0.25,
            },
            match kind {
                SensorKind::Pitot | SensorKind::Barometer => 0.02,
                SensorKind::Radar(RadarClass::Basic) => 0.0,
                SensorKind::Radar(RadarClass::Active) => -0.02,
            },
        )),
        PartSpec::Avionics { module } => Some(module.spec()),
        _ => None,
    }
}

/// The category a part belongs to in the electronics view.
pub fn category_of(spec: &PartSpec) -> Option<AvionicsCategory> {
    spec_of(spec).map(|spec| spec.category)
}

/// Aggregate electronics figures for one vehicle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AvionicsFigures {
    /// Every electronic device, legacy computers and sensors included.
    pub devices: usize,
    /// Mission modules only, which are the ones that draw power and grant effects.
    pub mission_modules: usize,
    pub mass_kg: f64,
    pub cost_cr: u64,
    pub power_draw_w: f64,
    pub power_supply_w: f64,
    pub power_margin_w: f64,
    pub complexity: f64,
    pub reliability: f64,
    pub authority_bonus: f64,
    pub lock_bonus: f64,
    pub fault_reduction: f64,
    pub has_control: bool,
    pub has_navigation: bool,
    pub has_datalink: bool,
    pub has_power: bool,
    pub has_diagnostics: bool,
}

impl AvionicsFigures {
    /// Multiplier applied to the aerodynamic control authority.
    pub fn authority_multiplier(self) -> f64 {
        1.0 + self.authority_bonus
    }

    /// Multiplier applied to the seeded airframe fault chance.
    pub fn fault_multiplier(self) -> f64 {
        1.0 - self.fault_reduction
    }

    /// Lock probability for a radar whose own probability is `base`.
    pub fn lock_probability(self, base: f64) -> f64 {
        (base + self.lock_bonus).clamp(0.0, 1.0)
    }

    pub fn power_shortfall(self) -> bool {
        self.power_draw_w > self.power_supply_w + f64::EPSILON
    }
}

/// The part of the electronics figures a flight report quotes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AvionicsSummary {
    pub mission_modules: usize,
    pub cost_cr: u64,
    pub power_draw_w: f64,
    pub power_supply_w: f64,
    pub power_margin_w: f64,
    pub lock_bonus: f64,
    pub authority_bonus: f64,
    pub fault_reduction: f64,
}

impl AvionicsFigures {
    pub fn summary(self) -> AvionicsSummary {
        AvionicsSummary {
            mission_modules: self.mission_modules,
            cost_cr: self.cost_cr,
            power_draw_w: self.power_draw_w,
            power_supply_w: self.power_supply_w,
            power_margin_w: self.power_margin_w,
            lock_bonus: self.lock_bonus,
            authority_bonus: self.authority_bonus,
            fault_reduction: self.fault_reduction,
        }
    }
}

/// One pass over the tree. The mission bus is drawn by the mission modules only: the flight
/// computer and the legacy sensors are powered by the vehicle bus, so a design without mission
/// electronics never needs a power cell.
pub fn figures(vehicle: &Vehicle) -> AvionicsFigures {
    let mut figures = AvionicsFigures::default();
    for part in &vehicle.parts {
        let Some(spec) = spec_of(&part.spec) else {
            continue;
        };
        figures.devices += 1;
        figures.mass_kg += part.spec.stats().dry_mass_kg;
        figures.cost_cr += spec.cost_cr;
        figures.complexity += spec.complexity;
        figures.reliability += spec.reliability;
        let PartSpec::Avionics { module } = part.spec else {
            continue;
        };
        figures.mission_modules += 1;
        figures.has_control |= module == AvionicsModule::ControlAugment;
        figures.has_navigation |= module == AvionicsModule::InertialNav;
        figures.has_datalink |= module == AvionicsModule::Datalink;
        figures.has_power |= module == AvionicsModule::PowerCell;
        figures.has_diagnostics |= module == AvionicsModule::Diagnostics;
        if spec.power_w >= 0.0 {
            figures.power_draw_w += spec.power_w;
        } else {
            figures.power_supply_w -= spec.power_w;
        }
        figures.authority_bonus += spec.authority_bonus;
        figures.lock_bonus += spec.lock_bonus;
        figures.fault_reduction += spec.fault_reduction;
    }
    figures.authority_bonus = figures.authority_bonus.clamp(0.0, MAX_AUTHORITY_BONUS);
    figures.lock_bonus = figures.lock_bonus.clamp(0.0, MAX_LOCK_BONUS);
    figures.fault_reduction = figures.fault_reduction.clamp(0.0, MAX_FAULT_REDUCTION);
    figures.power_margin_w = figures.power_supply_w - figures.power_draw_w;
    figures
}

/// Compatibility of the electronics stack: dependencies, duplicates and the power budget.
pub fn issues(vehicle: &Vehicle) -> Vec<VehicleIssue> {
    let modules: Vec<&crate::assembly::PlacedPart> = vehicle
        .parts
        .iter()
        .filter(|part| matches!(part.spec, PartSpec::Avionics { .. }))
        .collect();
    let mut issues = Vec::new();
    let module_of = |part: &crate::assembly::PlacedPart| match part.spec {
        PartSpec::Avionics { module } => Some(module),
        _ => None,
    };

    // One of each mission module, except the power cell, which may be stacked for capacity.
    let mut seen: BTreeMap<AvionicsModule, String> = BTreeMap::new();
    for part in &modules {
        let Some(module) = module_of(part) else {
            continue;
        };
        if module == AvionicsModule::PowerCell {
            continue;
        }
        if let Some(first) = seen.insert(module, part.id.clone()) {
            issues.push(VehicleIssue::error(
                format!(
                    "{} and {first} are both {} modules: only one is allowed",
                    part.id,
                    module.label()
                ),
                Some(part.id.clone()),
            ));
        }
    }

    let navigation = figures(vehicle).has_navigation;
    let computer = vehicle.computer();
    for part in &modules {
        let Some(module) = module_of(part) else {
            continue;
        };
        if module.needs_standard_computer()
            && computer.is_some_and(|class| class.rank() < ComputerClass::Standard.rank())
        {
            issues.push(VehicleIssue::error(
                format!("{}: {} needs a Standard flight computer", part.id, module.label()),
                Some(part.id.clone()),
            ));
        }
        if module.needs_navigation() && !navigation {
            issues.push(VehicleIssue::error(
                format!("{}: {} needs a navigation module to point at", part.id, module.label()),
                Some(part.id.clone()),
            ));
        }
    }

    let figures = figures(vehicle);
    if figures.power_shortfall() {
        let first = modules
            .iter()
            .find(|part| module_of(part).is_some_and(|module| module.spec().power_w > 0.0))
            .map(|part| part.id.clone());
        let message = if figures.power_supply_w <= 0.0 {
            format!(
                "the mission electronics draw {:.0} W but no power source is fitted",
                figures.power_draw_w
            )
        } else {
            format!(
                "the mission electronics draw {:.0} W but the supply provides {:.0} W",
                figures.power_draw_w, figures.power_supply_w
            )
        };
        issues.push(VehicleIssue::error(message, first));
    }

    let last_stage = vehicle.stage_count().saturating_sub(1);
    for part in &modules {
        if vehicle.stage_of(&part.id) != Some(last_stage) {
            issues.push(VehicleIssue::warning(
                format!(
                    "{} would be dropped before the last stage; keep mission electronics with the computer",
                    part.id
                ),
                Some(part.id.clone()),
            ));
        }
        if part.attach.as_ref().is_some_and(|attach| attach.node.is_radial()) {
            issues.push(VehicleIssue::warning(
                format!("{}: mission electronics mount on the axis", part.id),
                Some(part.id.clone()),
            ));
        }
    }
    if !figures.power_shortfall()
        && figures.power_draw_w > 0.0
        && figures.power_margin_w < figures.power_supply_w * POWER_HEADROOM_WARN
    {
        issues.push(VehicleIssue::warning(
            format!(
                "the power margin is only {:.0} W; add a cell or drop a module",
                figures.power_margin_w
            ),
            None,
        ));
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vehicle::{Part, Stage};

    fn vehicle(specs: Vec<(&str, PartSpec)>) -> Vehicle {
        let parts = specs
            .into_iter()
            .map(|(id, spec)| Part {
                id: id.to_string(),
                spec,
            })
            .collect();
        Vehicle::from_stages("Avionics test", vec![Stage { parts }])
    }

    fn module(module: AvionicsModule) -> PartSpec {
        PartSpec::Avionics { module }
    }

    fn standard_computer() -> PartSpec {
        PartSpec::Computer {
            class: ComputerClass::Standard,
        }
    }

    fn errors(vehicle: &Vehicle) -> Vec<String> {
        issues(vehicle)
            .into_iter()
            .filter(|issue| issue.severity == crate::assembly::IssueSeverity::Error)
            .map(|issue| issue.message)
            .collect()
    }

    #[test]
    fn every_category_is_reachable() {
        let specs = [
            standard_computer(),
            module(AvionicsModule::ControlAugment),
            PartSpec::Sensor {
                kind: SensorKind::Pitot,
            },
            module(AvionicsModule::InertialNav),
            module(AvionicsModule::Datalink),
            module(AvionicsModule::PowerCell),
            module(AvionicsModule::Diagnostics),
        ];
        for category in AvionicsCategory::ALL {
            assert!(
                specs.iter().any(|spec| category_of(spec) == Some(category)),
                "{category:?}"
            );
        }
    }

    #[test]
    fn figures_aggregate_mass_cost_power_and_bonuses() {
        let vehicle = vehicle(vec![
            ("computer", standard_computer()),
            ("nav", module(AvionicsModule::InertialNav)),
            ("link", module(AvionicsModule::Datalink)),
            ("power", module(AvionicsModule::PowerCell)),
            ("test", module(AvionicsModule::Diagnostics)),
        ]);
        let figures = figures(&vehicle);
        assert_eq!(figures.mission_modules, 4);
        assert_eq!(figures.devices, 5);
        assert_eq!(figures.power_draw_w, 18.0 + 12.0 + 10.0);
        assert_eq!(figures.power_supply_w, 60.0);
        assert_eq!(figures.power_margin_w, 20.0);
        assert!(figures.has_navigation && figures.has_datalink && figures.has_diagnostics);
        assert!((figures.lock_bonus - 0.10).abs() < 1e-9);
        assert!((figures.fault_multiplier() - 0.5).abs() < 1e-9);
        assert_eq!(figures.cost_cr, 1_600 + 1_800 + 1_200 + 900 + 1_100);
        assert!(figures.reliability > 0.0);
    }

    #[test]
    fn effects_are_capped_and_power_cells_stack() {
        let vehicle = vehicle(vec![
            ("power-1", module(AvionicsModule::PowerCell)),
            ("power-2", module(AvionicsModule::PowerCell)),
            ("control-1", module(AvionicsModule::ControlAugment)),
            ("control-2", module(AvionicsModule::ControlAugment)),
        ]);
        let figures = figures(&vehicle);
        assert_eq!(figures.power_supply_w, 120.0);
        assert!((figures.authority_bonus - MAX_AUTHORITY_BONUS).abs() < 1e-9);
        assert!(figures.power_margin_w > 0.0);
    }

    #[test]
    fn a_mission_module_without_a_cell_is_an_error() {
        let vehicle = vehicle(vec![
            ("computer", standard_computer()),
            ("nav", module(AvionicsModule::InertialNav)),
        ]);
        assert!(
            errors(&vehicle).iter().any(|error| error.contains("no power source")),
            "{:?}",
            errors(&vehicle)
        );
    }

    #[test]
    fn a_power_deficit_names_the_draw() {
        let vehicle = vehicle(vec![
            ("computer", standard_computer()),
            ("power", module(AvionicsModule::PowerCell)),
            ("control", module(AvionicsModule::ControlAugment)),
            ("test", module(AvionicsModule::Diagnostics)),
            ("nav", module(AvionicsModule::InertialNav)),
            ("link", module(AvionicsModule::Datalink)),
        ]);
        let errors = errors(&vehicle);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("draw 65 W") && error.contains("provides 60 W")),
            "{errors:?}"
        );
    }

    #[test]
    fn a_datalink_needs_navigation() {
        let vehicle = vehicle(vec![
            ("computer", standard_computer()),
            ("power", module(AvionicsModule::PowerCell)),
            ("link", module(AvionicsModule::Datalink)),
        ]);
        assert!(
            errors(&vehicle)
                .iter()
                .any(|error| error.contains("needs a navigation module")),
            "{:?}",
            errors(&vehicle)
        );
    }

    #[test]
    fn a_control_augment_needs_a_standard_computer() {
        let vehicle = vehicle(vec![
            (
                "computer",
                PartSpec::Computer {
                    class: ComputerClass::Basic,
                },
            ),
            ("power", module(AvionicsModule::PowerCell)),
            ("control", module(AvionicsModule::ControlAugment)),
        ]);
        assert!(
            errors(&vehicle)
                .iter()
                .any(|error| error.contains("Standard flight computer")),
            "{:?}",
            errors(&vehicle)
        );
    }

    #[test]
    fn only_one_of_a_mission_module_is_allowed() {
        let vehicle = vehicle(vec![
            ("computer", standard_computer()),
            ("power", module(AvionicsModule::PowerCell)),
            ("nav-1", module(AvionicsModule::InertialNav)),
            ("nav-2", module(AvionicsModule::InertialNav)),
        ]);
        assert!(
            errors(&vehicle)
                .iter()
                .any(|error| error.contains("only one is allowed")),
            "{:?}",
            errors(&vehicle)
        );
    }
}
