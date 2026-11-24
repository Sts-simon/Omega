//! Abstract aerodynamics for the workshop: wings, fin sets and control surfaces become one set of
//! gameplay figures per vehicle (areas, stability, authority, lift, drag, complexity, reliability,
//! aerodynamic mass) plus a fictional centre of gravity / centre of aerodynamics pair.
//!
//! The model is deliberately simple and deterministic: each design choice maps to one fixed effect
//! table below, effects are summed over the placed parts and normalised against one reference area.
//! There is no CFD, no real lift/drag table, no static-margin formula and no guidance or control
//! law. See `docs/AERODYNAMICS_PLAN.md` and `docs/DECISIONS.md` for the accepted values.

use crate::assembly::Vehicle;
use crate::vehicle::{AirfoilProfile, ControlKind, FinLayout, PartSpec, StabilizerRole, WingPlanform};
use std::f64::consts::PI;

/// Area (m²) at which a surface family reaches its full gameplay contribution. Chosen so a small
/// four-fin set is clearly useful and a huge wing saturates instead of breaking the scale.
pub const SURFACE_REFERENCE_M2: f64 = 0.30;
/// Stable band for the aerodynamic centre behind the centre of gravity, as a fraction of length.
pub const STABLE_BAND_LOW: f64 = 0.02;
pub const STABLE_BAND_HIGH: f64 = 0.10;
/// Areal density of the light-structure build, as a fraction of the normal surface mass.
pub const LIGHT_MASS_FACTOR: f64 = 0.70;
/// Drag index at which the drag indicator reads 100.
const DRAG_INDEX_REFERENCE_M2: f64 = 0.60;
/// Reliability below which a trial can suffer one abstract surface fault (see
/// [`AeroFigures::surface_fault_chance`]). A sound airframe never draws one.
pub const SURFACE_FAULT_RELIABILITY: f64 = 60.0;
/// Fault chance a completely unreliable airframe (the 25 floor) would reach.
pub const SURFACE_FAULT_MAX_CHANCE: f64 = 0.35;

/// Areal density in kg/m². Thin wing skins, heavier fin sets, heaviest moving surfaces.
pub const WING_AREAL_DENSITY: f64 = 3.2;
pub const FIN_AREAL_DENSITY: f64 = 4.8;
pub const CONTROL_AREAL_DENSITY: f64 = 5.6;

/// One design choice's contribution. Every field is a multiplier or a 0..1 score; keeping them in
/// one struct makes the trade-offs readable instead of scattering constants through the code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effect {
    pub area: f64,
    pub lift: f64,
    pub drag: f64,
    pub stability: f64,
    pub authority: f64,
    pub complexity: f64,
    pub reliability: f64,
    pub mass: f64,
}

impl Effect {
    const fn neutral() -> Self {
        Self {
            area: 1.0,
            lift: 1.0,
            drag: 1.0,
            stability: 1.0,
            authority: 1.0,
            complexity: 0.0,
            reliability: 0.0,
            mass: 1.0,
        }
    }
}

pub fn planform_effect(planform: WingPlanform) -> Effect {
    match planform {
        WingPlanform::Straight => Effect::neutral(),
        WingPlanform::Swept => Effect {
            area: 0.95,
            drag: 0.75,
            stability: 1.05,
            authority: 0.95,
            complexity: 0.15,
            ..Effect::neutral()
        },
        WingPlanform::Delta => Effect {
            area: 0.90,
            lift: 1.15,
            drag: 1.10,
            stability: 1.10,
            authority: 0.90,
            complexity: 0.25,
            reliability: -0.05,
            mass: 1.05,
        },
        WingPlanform::Trapezoidal => Effect {
            area: 0.95,
            lift: 1.05,
            drag: 0.90,
            stability: 1.05,
            complexity: 0.10,
            reliability: 0.02,
            mass: 1.02,
            ..Effect::neutral()
        },
        WingPlanform::VariableGeometry => Effect {
            lift: 1.10,
            drag: 0.85,
            stability: 1.05,
            authority: 1.15,
            complexity: 0.55,
            reliability: -0.20,
            mass: 1.25,
            ..Effect::neutral()
        },
    }
}

pub fn profile_effect(profile: AirfoilProfile) -> Effect {
    match profile {
        AirfoilProfile::Symmetric => Effect {
            lift: 0.90,
            drag: 0.95,
            stability: 1.05,
            complexity: 0.0,
            reliability: 0.05,
            ..Effect::neutral()
        },
        AirfoilProfile::Classic => Effect {
            complexity: 0.05,
            reliability: 0.02,
            ..Effect::neutral()
        },
        AirfoilProfile::LowDrag => Effect {
            lift: 0.85,
            drag: 0.70,
            stability: 0.95,
            authority: 0.95,
            complexity: 0.20,
            ..Effect::neutral()
        },
        AirfoilProfile::HighLift => Effect {
            lift: 1.25,
            drag: 1.25,
            stability: 0.90,
            authority: 0.90,
            complexity: 0.20,
            reliability: -0.05,
            ..Effect::neutral()
        },
        AirfoilProfile::Experimental => Effect {
            lift: 1.35,
            drag: 0.80,
            stability: 0.85,
            authority: 1.20,
            complexity: 0.60,
            reliability: -0.30,
            ..Effect::neutral()
        },
    }
}

pub fn role_effect(role: StabilizerRole) -> Effect {
    match role {
        StabilizerRole::Rear => Effect {
            lift: 0.20,
            authority: 0.60,
            stability: 1.00,
            complexity: 0.0,
            reliability: 0.05,
            ..Effect::neutral()
        },
        StabilizerRole::Front => Effect {
            lift: 0.30,
            drag: 1.05,
            stability: 0.60,
            authority: 0.80,
            complexity: 0.15,
            ..Effect::neutral()
        },
        StabilizerRole::Blended => Effect {
            lift: 0.15,
            drag: 0.70,
            stability: 0.50,
            authority: 0.40,
            complexity: 0.05,
            reliability: 0.05,
            mass: 0.80,
            ..Effect::neutral()
        },
        StabilizerRole::Combined => Effect {
            lift: 0.25,
            drag: 1.20,
            stability: 1.15,
            authority: 0.90,
            complexity: 0.30,
            reliability: -0.05,
            mass: 1.10,
            ..Effect::neutral()
        },
    }
}

pub fn layout_effect(layout: FinLayout) -> Effect {
    match layout {
        FinLayout::Symmetric => Effect::neutral(),
        FinLayout::Asymmetric => Effect {
            drag: 1.10,
            stability: 0.80,
            authority: 1.30,
            complexity: 0.35,
            reliability: -0.25,
            ..Effect::neutral()
        },
    }
}

pub fn control_effect(kind: ControlKind) -> Effect {
    match kind {
        ControlKind::Classic => Effect {
            lift: 0.15,
            authority: 1.00,
            complexity: 0.05,
            ..Effect::neutral()
        },
        ControlKind::Integrated => Effect {
            lift: 0.15,
            drag: 0.90,
            authority: 1.20,
            complexity: 0.25,
            reliability: -0.05,
            ..Effect::neutral()
        },
        ControlKind::Combined => Effect {
            lift: 0.20,
            drag: 1.15,
            stability: 1.10,
            authority: 1.35,
            complexity: 0.40,
            reliability: -0.12,
            mass: 1.10,
            ..Effect::neutral()
        },
        ControlKind::Fixed => Effect {
            lift: 0.10,
            drag: 1.10,
            stability: 1.25,
            authority: 0.0,
            complexity: 0.0,
            reliability: 0.10,
            ..Effect::neutral()
        },
    }
}

/// The flight computer a moving control surface needs. `Fixed` and `Classic` work with the basic
/// computer every vehicle already requires; integrated and combined surfaces need more.
pub fn required_computer(kind: ControlKind) -> Option<crate::vehicle::ComputerClass> {
    use crate::vehicle::ComputerClass;
    match kind {
        ControlKind::Integrated => Some(ComputerClass::Standard),
        ControlKind::Combined => Some(ComputerClass::Advanced),
        ControlKind::Classic | ControlKind::Fixed => None,
    }
}

/// Span × chord in m² before any shape factor. `None` for parts that are not aerodynamic.
pub fn surface_area(spec: &PartSpec) -> Option<f64> {
    match *spec {
        PartSpec::Wing { span_m, chord_m, .. }
        | PartSpec::FinSet { span_m, chord_m, .. }
        | PartSpec::ControlSurface { span_m, chord_m, .. } => Some(span_m * chord_m),
        _ => None,
    }
}

/// Dry mass of one aerodynamic surface in kg.
pub fn surface_mass_kg(spec: &PartSpec) -> Option<f64> {
    let area = surface_area(spec)?;
    let (density, effect) = match *spec {
        PartSpec::Wing { planform, profile, .. } => (
            WING_AREAL_DENSITY,
            combine(planform_effect(planform), profile_effect(profile)),
        ),
        PartSpec::FinSet { role, layout, .. } => (FIN_AREAL_DENSITY, combine(role_effect(role), layout_effect(layout))),
        PartSpec::ControlSurface { kind, .. } => (CONTROL_AREAL_DENSITY, control_effect(kind)),
        _ => return None,
    };
    let light = matches!(
        *spec,
        PartSpec::Wing { light: true, .. }
            | PartSpec::FinSet { light: true, .. }
            | PartSpec::ControlSurface { light: true, .. }
    );
    let factor = if light { LIGHT_MASS_FACTOR } else { 1.0 };
    Some((area * density * effect.mass * factor).max(0.05))
}

fn combine(a: Effect, b: Effect) -> Effect {
    Effect {
        area: a.area * b.area,
        lift: a.lift * b.lift,
        drag: a.drag * b.drag,
        stability: a.stability * b.stability,
        authority: a.authority * b.authority,
        complexity: a.complexity + b.complexity,
        reliability: a.reliability + b.reliability,
        mass: a.mass * b.mass,
    }
}

/// Effect of one placed surface, shape included.
fn surface_effect(spec: &PartSpec) -> Option<Effect> {
    match *spec {
        PartSpec::Wing { planform, profile, .. } => Some(combine(planform_effect(planform), profile_effect(profile))),
        PartSpec::FinSet { role, layout, .. } => Some(combine(role_effect(role), layout_effect(layout))),
        PartSpec::ControlSurface { kind, .. } => Some(control_effect(kind)),
        _ => None,
    }
}

/// Sweep multiplier on surface drag: a swept surface pays less at speed, a straight one pays more.
fn sweep_drag_factor(sweep_deg: f64) -> f64 {
    (1.0 + 0.35 * (sweep_deg / 60.0)).clamp(0.8, 1.35)
}

/// The vehicle-level aerodynamic sheet. All scores are 0..100; masses are kg and positions are
/// metres measured from the bottom of the design.
#[derive(Clone, Debug, PartialEq)]
pub struct AeroFigures {
    pub reference_area_m2: f64,
    pub wing_area_m2: f64,
    pub fin_area_m2: f64,
    pub control_area_m2: f64,
    pub mean_sweep_deg: f64,
    pub stability: f64,
    pub control_authority: f64,
    pub lift: f64,
    pub drag_index: f64,
    pub complexity: f64,
    pub reliability: f64,
    pub mass_kg: f64,
    pub surface_drag_area_m2: f64,
    pub cg_y_m: f64,
    pub ca_y_m: f64,
    pub margin_m: f64,
    pub balance: f64,
    pub length_m: f64,
    pub surface_count: usize,
    pub asymmetric: bool,
}

impl Default for AeroFigures {
    fn default() -> Self {
        Self {
            reference_area_m2: 0.0,
            wing_area_m2: 0.0,
            fin_area_m2: 0.0,
            control_area_m2: 0.0,
            mean_sweep_deg: 0.0,
            stability: 0.0,
            control_authority: 0.0,
            lift: 0.0,
            drag_index: 0.0,
            complexity: 0.0,
            reliability: 100.0,
            mass_kg: 0.0,
            surface_drag_area_m2: 0.0,
            cg_y_m: 0.0,
            ca_y_m: 0.0,
            margin_m: 0.0,
            balance: 0.0,
            length_m: 0.0,
            surface_count: 0,
            asymmetric: false,
        }
    }
}

impl AeroFigures {
    /// 0..1 lift indicator, used by the flight model.
    pub fn lift_ratio(&self) -> f64 {
        self.lift / 100.0
    }

    pub fn stability_ratio(&self) -> f64 {
        self.stability / 100.0
    }

    pub fn authority_ratio(&self) -> f64 {
        self.control_authority / 100.0
    }

    /// Seeded chance that this airframe suffers one surface fault during a trial. Only a fragile
    /// design below [`SURFACE_FAULT_RELIABILITY`] is exposed, and the chance grows linearly as the
    /// reliability drops; this is the gameplay sink for `reliability` and `complexity`.
    pub fn surface_fault_chance(&self) -> f64 {
        if self.surface_count == 0 {
            return 0.0;
        }
        let exposure = (SURFACE_FAULT_RELIABILITY - self.reliability) / SURFACE_FAULT_RELIABILITY;
        SURFACE_FAULT_MAX_CHANCE * exposure.clamp(0.0, 1.0)
    }

    /// The gameplay band in which the aerodynamic centre sits behind the centre of gravity.
    pub fn stable_band_m(&self) -> (f64, f64) {
        (STABLE_BAND_LOW * self.length_m, STABLE_BAND_HIGH * self.length_m)
    }

    pub fn is_stable(&self) -> bool {
        let (low, high) = self.stable_band_m();
        self.surface_count > 0 && (low..=high).contains(&self.margin_m)
    }

    pub fn summary(&self) -> AeroSummary {
        AeroSummary {
            stability: self.stability,
            control_authority: self.control_authority,
            lift: self.lift,
            drag_index: self.drag_index,
            surface_drag_area_m2: self.surface_drag_area_m2,
            wing_area_m2: self.wing_area_m2,
            fin_area_m2: self.fin_area_m2,
            control_area_m2: self.control_area_m2,
            mass_kg: self.mass_kg,
            margin_m: self.margin_m,
            surface_count: self.surface_count,
        }
    }
}

/// The part of the figures a flight report quotes.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct AeroSummary {
    pub stability: f64,
    pub control_authority: f64,
    pub lift: f64,
    pub drag_index: f64,
    pub surface_drag_area_m2: f64,
    pub wing_area_m2: f64,
    pub fin_area_m2: f64,
    pub control_area_m2: f64,
    pub mass_kg: f64,
    pub margin_m: f64,
    pub surface_count: usize,
}

fn clamp01(value: f64) -> f64 {
    0.0f64.max(value.min(1.0))
}

/// One pass over the tree: areas, masses, drag and the CG/CA pair.
pub fn figures(vehicle: &Vehicle) -> AeroFigures {
    let poses = vehicle.poses();
    let length = poses
        .iter()
        .map(|pose| pose.y_m + pose.height_m / 2.0)
        .fold(0.0, f64::max)
        - poses
            .iter()
            .map(|pose| pose.y_m - pose.height_m / 2.0)
            .fold(f64::INFINITY, f64::min);
    let length = if length.is_finite() && length > 0.0 {
        length
    } else {
        1.0
    };
    let body_diameter = vehicle.max_diameter_m().max(0.1);
    let reference_area = PI * body_diameter * body_diameter / 4.0;

    let mut mass_total = 0.0;
    let mut moment = 0.0;
    let mut wing_area = 0.0;
    let mut fin_area = 0.0;
    let mut fin_effective = 0.0;
    let mut control_area = 0.0;
    let mut moving_area = 0.0;
    let mut lift_area = 0.0;
    let mut drag_area = 0.0;
    let mut sweep_weighted = 0.0;
    let mut sweep_area = 0.0;
    let mut complexity_sum = 0.0;
    let mut reliability_sum = 0.0;
    let mut aero_mass = 0.0;
    let mut ca_weight = 0.0;
    let mut ca_moment = 0.0;
    let mut surface_count = 0;
    let mut asymmetric = false;

    for pose in &poses {
        let Some(part) = vehicle.part(&pose.id) else {
            continue;
        };
        let stats = part.spec.stats();
        let mass = stats.dry_mass_kg + stats.propellant_kg;
        mass_total += mass;
        moment += mass * pose.y_m;
        let Some(effect) = surface_effect(&part.spec) else {
            continue;
        };
        let area = surface_area(&part.spec).unwrap_or(0.0) * effect.area;
        surface_count += 1;
        aero_mass += stats.dry_mass_kg;
        complexity_sum += effect.complexity;
        reliability_sum += effect.reliability;
        match part.spec {
            PartSpec::Wing {
                sweep_deg,
                planform,
                profile,
                ..
            } => {
                wing_area += area;
                lift_area += area * planform_effect(planform).lift * profile_effect(profile).lift;
                sweep_weighted += sweep_deg * area;
                sweep_area += area;
                drag_area += area * effect.drag * sweep_drag_factor(sweep_deg);
                let weight = area;
                ca_weight += weight;
                ca_moment += weight * pose.y_m;
            }
            PartSpec::FinSet { layout, .. } => {
                fin_area += area;
                fin_effective += area * effect.stability;
                drag_area += area * effect.drag;
                let weight = area * 0.55;
                ca_weight += weight;
                ca_moment += weight * pose.y_m;
                if matches!(layout, FinLayout::Asymmetric) {
                    asymmetric = true;
                }
            }
            PartSpec::ControlSurface { .. } => {
                control_area += area;
                moving_area += area * effect.authority;
                drag_area += area * effect.drag;
                let weight = area * 0.25;
                ca_weight += weight;
                ca_moment += weight * pose.y_m;
            }
            _ => {}
        }
        if matches!(
            part.spec,
            PartSpec::Wing { light: true, .. }
                | PartSpec::FinSet { light: true, .. }
                | PartSpec::ControlSurface { light: true, .. }
        ) {
            complexity_sum += 0.10;
            reliability_sum -= 0.05;
        }
    }

    let cg_y = if mass_total > 0.0 { moment / mass_total } else { 0.0 };
    let ca_y = if ca_weight > 0.0 { ca_moment / ca_weight } else { cg_y };
    let margin = cg_y - ca_y;

    if surface_count == 0 {
        return AeroFigures {
            reference_area_m2: reference_area,
            cg_y_m: cg_y,
            ca_y_m: cg_y,
            length_m: length,
            ..AeroFigures::default()
        };
    }

    let fin_ratio = clamp01(fin_effective / SURFACE_REFERENCE_M2);
    let margin_score = clamp01((margin / length + STABLE_BAND_LOW) / (STABLE_BAND_HIGH - STABLE_BAND_LOW));
    let stability = 100.0 * (0.55 * fin_ratio + 0.45 * margin_score);
    let authority = 100.0
        * (0.85 * clamp01(moving_area / SURFACE_REFERENCE_M2)
            + 0.15 * clamp01(wing_area / (2.0 * SURFACE_REFERENCE_M2)));
    // One default wing reads about 50: a pair of wings is what a cruise design needs.
    let lift = 100.0 * clamp01(lift_area / (3.0 * SURFACE_REFERENCE_M2));
    let drag_index = 100.0 * clamp01(drag_area / DRAG_INDEX_REFERENCE_M2);
    let count = surface_count as f64;
    let complexity = 100.0 * clamp01(complexity_sum / count + 0.05 * (count - 1.0));
    let reliability = 100.0
        * (1.0 - 0.45 * (complexity_sum / count) + 0.5 * (reliability_sum / count) - 0.05 * (count - 1.0))
            .clamp(0.25, 1.0);
    let balance = if margin < STABLE_BAND_LOW * length {
        100.0 * clamp01((margin / length + STABLE_BAND_LOW) / (2.0 * STABLE_BAND_LOW))
    } else if margin > STABLE_BAND_HIGH * length {
        100.0 * clamp01(1.0 - (margin / length - STABLE_BAND_HIGH) / 0.15)
    } else {
        100.0
    };

    AeroFigures {
        reference_area_m2: reference_area,
        wing_area_m2: wing_area,
        fin_area_m2: fin_area,
        control_area_m2: control_area,
        mean_sweep_deg: if sweep_area > 0.0 {
            sweep_weighted / sweep_area
        } else {
            0.0
        },
        stability,
        control_authority: authority,
        lift,
        drag_index,
        complexity,
        reliability,
        mass_kg: aero_mass,
        surface_drag_area_m2: drag_area,
        cg_y_m: cg_y,
        ca_y_m: ca_y,
        margin_m: margin,
        balance,
        length_m: length,
        surface_count,
        asymmetric,
    }
}

/// Non-blocking design notes for the aerodynamics. Refusals live in `Vehicle::validate`.
pub fn warnings(vehicle: &Vehicle) -> Vec<String> {
    let figures = figures(vehicle);
    let mut warnings = Vec::new();
    let body_diameter = vehicle.max_diameter_m().max(0.1);
    for part in &vehicle.parts {
        let (PartSpec::Wing { span_m, .. } | PartSpec::FinSet { span_m, .. } | PartSpec::ControlSurface { span_m, .. }) =
            part.spec
        else {
            continue;
        };
        if span_m > 2.0 * body_diameter {
            warnings.push(format!(
                "{} spans {span_m:.2} m on a {body_diameter:.2} m body: ground clearance and launcher fit",
                part.id
            ));
        }
    }
    if figures.mass_kg > 0.0 {
        let total = vehicle.total_wet_mass_kg().max(1e-9);
        let share = figures.mass_kg / total;
        if share > 0.20 {
            warnings.push(format!(
                "aerodynamic surfaces add {:.0} kg, {:.0}% of launch mass",
                figures.mass_kg,
                share * 100.0
            ));
        }
    }
    if figures.control_area_m2 > 0.0 && figures.wing_area_m2 == 0.0 && figures.fin_area_m2 == 0.0 {
        warnings.push("control surfaces have no fixed wing or fin to act with: add a lifting surface".to_string());
    }
    if figures.asymmetric {
        warnings.push("asymmetric fin set: experimental, stability reduced".to_string());
    }
    if figures.surface_count > 0 && figures.margin_m < 0.0 {
        warnings.push(format!(
            "centre of aerodynamics is {:.2} m ahead of the centre of gravity: unstable",
            -figures.margin_m
        ));
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::tests::sounding_rocket;
    use crate::vehicle::{ComputerClass, FinLayout, PartSpec};
    use crate::{NodeId, PlacedPart};

    fn wing(span_m: f64, chord_m: f64) -> PartSpec {
        PartSpec::Wing {
            span_m,
            chord_m,
            sweep_deg: 20.0,
            planform: WingPlanform::Swept,
            profile: AirfoilProfile::Classic,
            light: false,
        }
    }

    fn fin(span_m: f64, chord_m: f64) -> PartSpec {
        PartSpec::FinSet {
            span_m,
            chord_m,
            role: StabilizerRole::Rear,
            layout: FinLayout::Symmetric,
            light: false,
        }
    }

    fn control(span_m: f64, chord_m: f64, kind: ControlKind) -> PartSpec {
        PartSpec::ControlSurface {
            span_m,
            chord_m,
            kind,
            light: false,
        }
    }

    fn mount(vehicle: &mut Vehicle, id: &str, spec: PartSpec, parent: &str, node: NodeId) {
        vehicle.parts.push(PlacedPart {
            id: id.to_string(),
            spec,
            attach: None,
            symmetry: None,
        });
        vehicle.attach(id, parent, node, 0.0).unwrap();
    }

    fn four_fins(vehicle: &mut Vehicle) {
        for index in 0..4u8 {
            mount(
                vehicle,
                &format!("fin-{index}"),
                fin(0.3, 0.35),
                "booster",
                NodeId::Radial(index),
            );
        }
    }

    #[test]
    fn a_vehicle_without_surfaces_has_empty_figures_and_zero_drag_area() {
        let figures = figures(&sounding_rocket());
        assert_eq!(figures.surface_count, 0);
        assert_eq!(figures.surface_drag_area_m2, 0.0);
        assert_eq!(figures.wing_area_m2, 0.0);
        assert_eq!(figures.mass_kg, 0.0);
        assert_eq!(figures.stability, 0.0);
        assert!(figures.ca_y_m == figures.cg_y_m);
        assert!(figures.margin_m == 0.0);
    }

    #[test]
    fn fins_add_stability_and_reduce_the_centre_of_aerodynamics() {
        let base = sounding_rocket();
        let mut with_fins = base.clone();
        four_fins(&mut with_fins);
        let plain = figures(&base);
        let stabilised = figures(&with_fins);
        assert!(stabilised.fin_area_m2 > 0.0);
        assert!(stabilised.stability > plain.stability + 30.0);
        assert!(
            stabilised.ca_y_m < stabilised.cg_y_m,
            "fins pull the aerodynamic centre behind the centre of gravity"
        );
        assert!(stabilised.surface_drag_area_m2 > 0.0);
        assert!(stabilised.mass_kg > 0.0);
    }

    #[test]
    fn wings_add_lift_and_control_surfaces_add_authority() {
        let mut vehicle = sounding_rocket();
        mount(&mut vehicle, "wing-1", wing(1.2, 0.4), "sustainer", NodeId::Radial(0));
        let winged = figures(&vehicle);
        assert!(winged.lift > 30.0, "lift {}", winged.lift);
        assert!(winged.wing_area_m2 > 0.4);
        let authority = winged.control_authority;

        mount(
            &mut vehicle,
            "ctrl-1",
            control(0.4, 0.4, ControlKind::Combined),
            "sustainer",
            NodeId::Radial(1),
        );
        let with_control = figures(&vehicle);
        assert!(with_control.control_authority > authority + 20.0);
        assert!(with_control.control_area_m2 > 0.0);
    }

    #[test]
    fn a_fixed_surface_adds_stability_but_no_authority() {
        let mut vehicle = sounding_rocket();
        mount(
            &mut vehicle,
            "ctrl-1",
            control(0.4, 0.4, ControlKind::Fixed),
            "booster",
            NodeId::Radial(0),
        );
        let figures = figures(&vehicle);
        assert!(figures.stability > 0.0);
        assert_eq!(figures.control_authority, 0.0);
        assert!(figures.surface_drag_area_m2 > 0.0);
    }

    #[test]
    fn the_light_build_is_lighter_but_less_reliable() {
        let mut normal = sounding_rocket();
        four_fins(&mut normal);
        let mut light = normal.clone();
        for part in &mut light.parts {
            if let PartSpec::FinSet { light, .. } = &mut part.spec {
                *light = true;
            }
        }
        let heavy = figures(&normal);
        let built = figures(&light);
        assert!(built.mass_kg < heavy.mass_kg);
        assert!(built.reliability < heavy.reliability);
        assert!(built.complexity > heavy.complexity);
    }

    #[test]
    fn the_design_choices_change_the_scores_monotonically() {
        let mut high_lift = sounding_rocket();
        mount(
            &mut high_lift,
            "wing-1",
            PartSpec::Wing {
                span_m: 1.2,
                chord_m: 0.4,
                sweep_deg: 0.0,
                planform: WingPlanform::Straight,
                profile: AirfoilProfile::HighLift,
                light: false,
            },
            "sustainer",
            NodeId::Radial(0),
        );
        let mut low_drag = high_lift.clone();
        if let PartSpec::Wing { profile, .. } = &mut low_drag.parts.last_mut().unwrap().spec {
            *profile = AirfoilProfile::LowDrag;
        }
        let lift = figures(&high_lift);
        let drag = figures(&low_drag);
        assert!(lift.lift > drag.lift);
        assert!(drag.drag_index < lift.drag_index);
    }

    #[test]
    fn an_unstable_configuration_is_reported_as_a_warning() {
        let mut vehicle = sounding_rocket();
        // A big wing near the nose pulls the aerodynamic centre ahead of the centre of gravity.
        mount(&mut vehicle, "wing-1", wing(2.0, 1.2), "sustainer", NodeId::Radial(0));
        let figures = figures(&vehicle);
        assert!(figures.margin_m < 0.0, "margin {}", figures.margin_m);
        assert!(
            warnings(&vehicle)
                .iter()
                .any(|w| w.contains("ahead of the centre of gravity")),
            "{:?}",
            warnings(&vehicle)
        );
    }

    #[test]
    fn an_oversized_surface_is_a_warning_not_a_refusal() {
        let mut vehicle = sounding_rocket();
        mount(&mut vehicle, "wing-1", wing(3.0, 1.0), "booster", NodeId::Radial(0));
        assert!(
            warnings(&vehicle).iter().any(|w| w.contains("ground clearance")),
            "{:?}",
            warnings(&vehicle)
        );
        assert!(vehicle.validate().is_ok(), "{:?}", vehicle.validate());
    }

    #[test]
    fn moving_control_surfaces_need_a_compatible_computer() {
        assert_eq!(required_computer(ControlKind::Classic), None);
        assert_eq!(
            required_computer(ControlKind::Integrated),
            Some(ComputerClass::Standard)
        );
        assert_eq!(required_computer(ControlKind::Combined), Some(ComputerClass::Advanced));
        assert_eq!(required_computer(ControlKind::Fixed), None);
    }

    /// Every score is finite, inside 0..100, and the CG/CA pair stays inside the vehicle.
    fn assert_bounded(vehicle: &Vehicle) -> AeroFigures {
        let figures = figures(vehicle);
        for (name, value) in [
            ("reference_area", figures.reference_area_m2),
            ("wing_area", figures.wing_area_m2),
            ("fin_area", figures.fin_area_m2),
            ("control_area", figures.control_area_m2),
            ("mean_sweep", figures.mean_sweep_deg),
            ("mass", figures.mass_kg),
            ("drag_area", figures.surface_drag_area_m2),
            ("cg_y", figures.cg_y_m),
            ("ca_y", figures.ca_y_m),
            ("margin", figures.margin_m),
            ("length", figures.length_m),
        ] {
            assert!(value.is_finite(), "{name} is not finite: {value}");
        }
        for (name, score) in [
            ("stability", figures.stability),
            ("authority", figures.control_authority),
            ("lift", figures.lift),
            ("drag", figures.drag_index),
            ("complexity", figures.complexity),
            ("reliability", figures.reliability),
            ("balance", figures.balance),
        ] {
            assert!(
                (0.0..=100.0).contains(&score),
                "{name} left its scale: {score} for {:?}",
                vehicle.parts.last().map(|part| part.spec)
            );
        }
        assert!(figures.mass_kg >= 0.0 && figures.wing_area_m2 >= 0.0);
        assert!(figures.length_m > 0.0);
        let (low, high) = figures.stable_band_m();
        assert!(low < high, "the stable band is empty: {low}..{high}");
        let poses = vehicle.poses();
        let bottom = poses
            .iter()
            .map(|pose| pose.y_m - pose.height_m / 2.0)
            .fold(f64::INFINITY, f64::min);
        let top = poses
            .iter()
            .map(|pose| pose.y_m + pose.height_m / 2.0)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            (bottom..=top).contains(&figures.cg_y_m),
            "cg {} outside {bottom}..{top}",
            figures.cg_y_m
        );
        assert!(
            (bottom..=top).contains(&figures.ca_y_m),
            "ca {} outside {bottom}..{top}",
            figures.ca_y_m
        );
        figures
    }

    #[test]
    fn a_fragile_airframe_is_exposed_and_a_sound_one_is_not() {
        let mut sound = sounding_rocket();
        four_fins(&mut sound);
        let mut fragile = sounding_rocket();
        for index in 0..4u8 {
            mount(
                &mut fragile,
                &format!("fin-{index}"),
                PartSpec::FinSet {
                    span_m: 0.3,
                    chord_m: 0.35,
                    role: StabilizerRole::Rear,
                    layout: FinLayout::Asymmetric,
                    light: true,
                },
                "booster",
                NodeId::Radial(index),
            );
        }
        let sound = figures(&sound);
        let fragile = figures(&fragile);
        assert_eq!(sound.surface_fault_chance(), 0.0);
        assert!(fragile.reliability < SURFACE_FAULT_RELIABILITY);
        assert!(
            fragile.surface_fault_chance() > 0.0 && fragile.surface_fault_chance() < SURFACE_FAULT_MAX_CHANCE / 2.0,
            "{}",
            fragile.surface_fault_chance()
        );
        assert_eq!(AeroFigures::default().surface_fault_chance(), 0.0);
    }

    #[test]
    fn every_design_choice_and_extreme_dimension_stays_bounded() {
        let planforms = [
            WingPlanform::Straight,
            WingPlanform::Swept,
            WingPlanform::Delta,
            WingPlanform::Trapezoidal,
            WingPlanform::VariableGeometry,
        ];
        let profiles = [
            AirfoilProfile::Symmetric,
            AirfoilProfile::Classic,
            AirfoilProfile::LowDrag,
            AirfoilProfile::HighLift,
            AirfoilProfile::Experimental,
        ];
        for planform in planforms {
            for profile in profiles {
                for sweep in [0.0, 30.0, 60.0] {
                    for (span, chord) in [(0.2, 0.1), (1.0, 0.4), (4.0, 2.0)] {
                        for light in [false, true] {
                            let mut vehicle = sounding_rocket();
                            mount(
                                &mut vehicle,
                                "surface",
                                PartSpec::Wing {
                                    span_m: span,
                                    chord_m: chord,
                                    sweep_deg: sweep,
                                    planform,
                                    profile,
                                    light,
                                },
                                "booster",
                                NodeId::Radial(0),
                            );
                            let sheet = assert_bounded(&vehicle);
                            assert!(sheet.mass_kg > 0.0);
                            assert_eq!(sheet, figures(&vehicle), "the sheet is deterministic");
                        }
                    }
                }
            }
        }

        for role in [
            StabilizerRole::Rear,
            StabilizerRole::Front,
            StabilizerRole::Blended,
            StabilizerRole::Combined,
        ] {
            for layout in [FinLayout::Symmetric, FinLayout::Asymmetric] {
                for kind in [
                    ControlKind::Classic,
                    ControlKind::Integrated,
                    ControlKind::Combined,
                    ControlKind::Fixed,
                ] {
                    for light in [false, true] {
                        let mut vehicle = sounding_rocket();
                        mount(
                            &mut vehicle,
                            "fin",
                            PartSpec::FinSet {
                                span_m: 4.0,
                                chord_m: 2.0,
                                role,
                                layout,
                                light,
                            },
                            "booster",
                            NodeId::Radial(0),
                        );
                        mount(
                            &mut vehicle,
                            "control",
                            PartSpec::ControlSurface {
                                span_m: 4.0,
                                chord_m: 2.0,
                                kind,
                                light,
                            },
                            "booster",
                            NodeId::Radial(1),
                        );
                        assert_bounded(&vehicle);
                    }
                }
            }
        }
    }

    #[test]
    fn the_areal_mass_scales_with_area_and_shape() {
        let small = PartSpec::Wing {
            span_m: 0.5,
            chord_m: 0.2,
            sweep_deg: 0.0,
            planform: WingPlanform::Straight,
            profile: AirfoilProfile::Classic,
            light: false,
        };
        let large = PartSpec::Wing {
            span_m: 1.0,
            chord_m: 0.2,
            sweep_deg: 0.0,
            planform: WingPlanform::Straight,
            profile: AirfoilProfile::Classic,
            light: false,
        };
        assert!(surface_mass_kg(&large).unwrap() > 2.0 * surface_mass_kg(&small).unwrap() - 0.1);
        assert_eq!(surface_area(&large), Some(0.2));
        assert_eq!(surface_mass_kg(&PartSpec::Decoupler), None);
    }
}
