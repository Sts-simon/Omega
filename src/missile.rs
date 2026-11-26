//! Missile workshop: a staged vehicle built from resizable parts (`omega-rules::vehicle`),
//! edited with buttons and shown in 3D. Parts are gated by research capabilities.
//! Everything here is fictional and abstract; see `docs/VISION_MISSILES.md`.

use crate::campus::{CampusState, CampusView};
use crate::save::{self, SaveFile};
use crate::scene::OrbitCameraRig;
use crate::subassembly::{self, Subassembly, SubassemblyKind};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use omega_rules::assembly::{Attach, SymmetryKind, decode_symmetry, encode_symmetry, radial_slots};
use omega_rules::{
    AdapterShape, AirfoilProfile, AvionicsModule, ComputerClass, ControlKind, ExplosiveGrade, FinLayout, IssueSeverity,
    NoseShape, Part, PartCategory, PartSpec, RadarClass, SensorKind, StabilizerRole, Stage, Vehicle, WingPlanform,
};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

/// Notches closer than this to the previous one on the same part and value form one gesture.
const WHEEL_GESTURE: Duration = Duration::from_millis(500);
/// Bounds accepted by the numeric transform fields. They are deliberately generous for a
/// workshop measured in metres and degrees, but still keep a typo from throwing a part out of
/// the editable universe.
pub(crate) const MAX_TRANSFORM_OFFSET_M: f64 = 1_000.0;
pub(crate) const MAX_TRANSFORM_ROTATION_DEG: f64 = 3_600.0;

pub(crate) const ANCHOR: Vec3 = Vec3::new(0.0, 0.2, 18.0);
pub(crate) const VISUAL_SCALE: f32 = 1.6;
/// A cruise vehicle lies on the assembly pad, so its presentation anchor rises above the pad
/// instead of sinking the thin horizontal hull into it.
const HORIZONTAL_LIFT: f32 = 0.9;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PartTemplate {
    SolidMotor,
    FuelTank,
    LiquidEngine,
    AirBreathing,
    Pitot,
    Barometer,
    RadarBasic,
    RadarActive,
    ComputerBasic,
    ComputerStandard,
    ComputerAdvanced,
    ControlAugment,
    InertialNav,
    Datalink,
    PowerCell,
    Diagnostics,
    Decoupler,
    MissionModule,
    ExplosiveStandard,
    ExplosiveImproved,
    ExplosiveAdvanced,
    Adapter,
    Nose,
    Wing,
    FinSet,
    ControlSurface,
}

impl PartTemplate {
    /// Every palette entry, in the order the workshop shows them inside each category.
    pub const ALL: [Self; 26] = [
        Self::SolidMotor,
        Self::FuelTank,
        Self::LiquidEngine,
        Self::AirBreathing,
        Self::Decoupler,
        Self::MissionModule,
        Self::ExplosiveStandard,
        Self::ExplosiveImproved,
        Self::ExplosiveAdvanced,
        Self::Adapter,
        Self::Nose,
        Self::Wing,
        Self::FinSet,
        Self::ControlSurface,
        Self::Pitot,
        Self::Barometer,
        Self::RadarBasic,
        Self::RadarActive,
        Self::ComputerBasic,
        Self::ComputerStandard,
        Self::ComputerAdvanced,
        Self::ControlAugment,
        Self::InertialNav,
        Self::Datalink,
        Self::PowerCell,
        Self::Diagnostics,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::SolidMotor => "SOLID MOTOR",
            Self::FuelTank => "FUEL TANK",
            Self::LiquidEngine => "LIQUID ENGINE",
            Self::AirBreathing => "TURBOFAN",
            Self::Pitot => "PITOT TUBE",
            Self::Barometer => "BAROMETER",
            Self::RadarBasic => "RADAR BASIC",
            Self::RadarActive => "RADAR ACTIVE",
            Self::ComputerBasic => "COMPUTER 1",
            Self::ComputerStandard => "COMPUTER 2",
            Self::ComputerAdvanced => "COMPUTER 3",
            Self::ControlAugment => AvionicsModule::ControlAugment.short_label(),
            Self::InertialNav => AvionicsModule::InertialNav.short_label(),
            Self::Datalink => AvionicsModule::Datalink.short_label(),
            Self::PowerCell => AvionicsModule::PowerCell.short_label(),
            Self::Diagnostics => AvionicsModule::Diagnostics.short_label(),
            Self::Decoupler => "DECOUPLER",
            Self::MissionModule => "MISSION MODULE",
            Self::ExplosiveStandard => "EXPLOSIVE CHARGE",
            Self::ExplosiveImproved => "IMPROVED CHARGE",
            Self::ExplosiveAdvanced => "ADVANCED CHARGE",
            Self::Adapter => "ADAPTER",
            Self::Nose => "NOSE CONE",
            Self::Wing => "WING",
            Self::FinSet => "FIN SET",
            Self::ControlSurface => "CONTROL",
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::SolidMotor => "motor",
            Self::FuelTank => "tank",
            Self::LiquidEngine => "engine",
            Self::AirBreathing => "turbofan",
            Self::Pitot => "pitot",
            Self::Barometer => "baro",
            Self::RadarBasic | Self::RadarActive => "radar",
            Self::ComputerBasic | Self::ComputerStandard | Self::ComputerAdvanced => "computer",
            Self::ControlAugment => "control_module",
            Self::InertialNav => "nav",
            Self::Datalink => "datalink",
            Self::PowerCell => "power",
            Self::Diagnostics => "selftest",
            Self::Decoupler => "decoupler",
            Self::MissionModule => "module",
            Self::ExplosiveStandard => "charge",
            Self::ExplosiveImproved => "charge_improved",
            Self::ExplosiveAdvanced => "charge_advanced",
            Self::Adapter => "adapter",
            Self::Nose => "nose",
            Self::Wing => "wing",
            Self::FinSet => "fin",
            Self::ControlSurface => "control",
        }
    }

    pub fn spec(self) -> PartSpec {
        match self {
            Self::SolidMotor => PartSpec::SolidMotor {
                length_m: 1.5,
                diameter_m: 0.4,
                burn_time_s: 15.0,
            },
            Self::FuelTank => PartSpec::FuelTank {
                length_m: 1.5,
                diameter_m: 0.4,
            },
            Self::LiquidEngine => PartSpec::LiquidEngine { scale: 1.0 },
            Self::AirBreathing => PartSpec::AirBreathing { scale: 1.0 },
            Self::Pitot => PartSpec::Sensor {
                kind: SensorKind::Pitot,
            },
            Self::Barometer => PartSpec::Sensor {
                kind: SensorKind::Barometer,
            },
            Self::RadarBasic => PartSpec::Sensor {
                kind: SensorKind::Radar(RadarClass::Basic),
            },
            Self::RadarActive => PartSpec::Sensor {
                kind: SensorKind::Radar(RadarClass::Active),
            },
            Self::ComputerBasic => PartSpec::Computer {
                class: ComputerClass::Basic,
            },
            Self::ComputerStandard => PartSpec::Computer {
                class: ComputerClass::Standard,
            },
            Self::ComputerAdvanced => PartSpec::Computer {
                class: ComputerClass::Advanced,
            },
            Self::ControlAugment => PartSpec::Avionics {
                module: AvionicsModule::ControlAugment,
            },
            Self::InertialNav => PartSpec::Avionics {
                module: AvionicsModule::InertialNav,
            },
            Self::Datalink => PartSpec::Avionics {
                module: AvionicsModule::Datalink,
            },
            Self::PowerCell => PartSpec::Avionics {
                module: AvionicsModule::PowerCell,
            },
            Self::Diagnostics => PartSpec::Avionics {
                module: AvionicsModule::Diagnostics,
            },
            Self::Decoupler => PartSpec::Decoupler,
            Self::MissionModule => PartSpec::MissionModule { mass_kg: 40.0 },
            Self::ExplosiveStandard => PartSpec::Explosive {
                mass_kg: 40.0,
                grade: ExplosiveGrade::Standard,
            },
            Self::ExplosiveImproved => PartSpec::Explosive {
                mass_kg: 40.0,
                grade: ExplosiveGrade::Improved,
            },
            Self::ExplosiveAdvanced => PartSpec::Explosive {
                mass_kg: 40.0,
                grade: ExplosiveGrade::Advanced,
            },
            Self::Adapter => PartSpec::Adapter {
                length_m: 0.4,
                diameter_m: 0.4,
                exit_diameter_m: 0.3,
                shape: AdapterShape::Conical,
            },
            Self::Nose => PartSpec::NoseCone {
                length_m: 0.6,
                diameter_m: 0.4,
                shape: NoseShape::Ogive,
            },
            Self::Wing => PartSpec::Wing {
                span_m: 1.2,
                chord_m: 0.4,
                sweep_deg: 20.0,
                planform: WingPlanform::Swept,
                profile: AirfoilProfile::Classic,
                light: false,
            },
            Self::FinSet => PartSpec::FinSet {
                span_m: 0.35,
                chord_m: 0.4,
                role: StabilizerRole::Rear,
                layout: FinLayout::Symmetric,
                light: false,
            },
            Self::ControlSurface => PartSpec::ControlSurface {
                span_m: 0.35,
                chord_m: 0.35,
                kind: ControlKind::Classic,
                light: false,
            },
        }
    }

    pub fn is_computer(self) -> bool {
        matches!(
            self,
            Self::ComputerBasic | Self::ComputerStandard | Self::ComputerAdvanced
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Param {
    Length,
    Diameter,
    ExitDiameter,
    BurnTime,
    Scale,
    Mass,
    Span,
    Chord,
    Sweep,
    /// Actual mounting diameter of a fixed-cross-section part, backed by `PlacedPart::size_scale`.
    Size,
}

impl Param {
    pub fn label(self) -> &'static str {
        match self {
            Self::Length => "Length (m)",
            Self::Diameter => "Diameter (m)",
            Self::ExitDiameter => "Exit diameter (m)",
            Self::BurnTime => "Burn time (s)",
            Self::Scale => "Engine scale",
            Self::Mass => "Mass (kg)",
            Self::Span => "Span (m)",
            Self::Chord => "Chord (m)",
            Self::Sweep => "Sweep (deg)",
            Self::Size => "Mount diameter (m)",
        }
    }

    fn step(self) -> f64 {
        match self {
            Self::Length => 0.1,
            Self::Diameter => 0.05,
            Self::ExitDiameter => 0.05,
            Self::BurnTime => 1.0,
            Self::Scale => 0.1,
            Self::Mass => 5.0,
            Self::Span => 0.05,
            Self::Chord => 0.05,
            Self::Sweep => 5.0,
            Self::Size => 0.05,
        }
    }
}

/// Adjustable parameters of a part, with their current values.
pub(crate) fn params_of(part: &omega_rules::PlacedPart) -> Vec<(Param, f64)> {
    let mut params = match part.spec {
        PartSpec::SolidMotor {
            length_m,
            diameter_m,
            burn_time_s,
        } => vec![
            (Param::Length, length_m),
            (Param::Diameter, diameter_m),
            (Param::BurnTime, burn_time_s),
        ],
        PartSpec::FuelTank { length_m, diameter_m } => {
            vec![(Param::Length, length_m), (Param::Diameter, diameter_m)]
        }
        PartSpec::LiquidEngine { scale } | PartSpec::AirBreathing { scale } => {
            vec![(Param::Scale, scale)]
        }
        PartSpec::MissionModule { mass_kg } => vec![(Param::Mass, mass_kg)],
        PartSpec::Explosive { mass_kg, .. } => vec![(Param::Mass, mass_kg)],
        PartSpec::Adapter {
            length_m,
            diameter_m,
            exit_diameter_m,
            ..
        } => vec![
            (Param::Length, length_m),
            (Param::Diameter, diameter_m),
            (Param::ExitDiameter, exit_diameter_m),
        ],
        PartSpec::NoseCone {
            length_m, diameter_m, ..
        } => vec![(Param::Length, length_m), (Param::Diameter, diameter_m)],
        PartSpec::Wing {
            span_m,
            chord_m,
            sweep_deg,
            ..
        } => vec![
            (Param::Span, span_m),
            (Param::Chord, chord_m),
            (Param::Sweep, sweep_deg),
        ],
        PartSpec::FinSet { span_m, chord_m, .. } | PartSpec::ControlSurface { span_m, chord_m, .. } => {
            vec![(Param::Span, span_m), (Param::Chord, chord_m)]
        }
        _ => Vec::new(),
    };
    if omega_rules::assembly::supports_size_scale(&part.spec) {
        params.push((Param::Size, part.diameter_m()));
    }
    params
}

/// Contextual help shown under the selected part: role, what the settings change, and the
/// constraint that is easiest to miss. Written for a first-time designer, not a manual.
pub(crate) fn part_help(spec: &PartSpec) -> Vec<String> {
    let (role, effect, caution) = match *spec {
        PartSpec::SolidMotor { .. } => (
            "Propulse l'etage pendant sa combustion ; c'est le moteur du bas de pile.",
            "Une combustion plus courte donne plus de poussee mais une coque plus lourde.",
            "Un moteur solide n'a pas besoin de reservoir et ne peut pas etre rallume.",
        ),
        PartSpec::FuelTank { .. } => (
            "Stocke le propergol des moteurs liquides et des turboreacteurs de son etage.",
            "La longueur et le diametre augmentent la masse et le volume disponible.",
            "Un moteur liquide sans reservoir dans le meme etage est invalide.",
        ),
        PartSpec::LiquidEngine { .. } => (
            "Moteur principal alimente par les reservoirs de son etage.",
            "L'echelle change la poussee, la masse et la consommation ensemble.",
            "Il exige un reservoir et reste dans l'etage ou il est monte.",
        ),
        PartSpec::AirBreathing { .. } => (
            "Turboreacteur de croisiere, econome mais tributaire de la vitesse de l'air.",
            "L'echelle regle la poussee ; les ailes et la vitesse reduisent le seuil de fonctionnement.",
            "Sans vitesse suffisante il ne pousse pas : ajoutez des ailes et un profil de croisiere.",
        ),
        PartSpec::Sensor { .. } => (
            "Capteur de vol : pitot pour la vitesse, barometre pour l'altitude, radar pour le verrouillage.",
            "Le diametre de montage aligne le capteur sur le corps sans adaptateur.",
            "Un radar n'est utile que si l'ordinateur et le programme savent l'exploiter.",
        ),
        PartSpec::Computer { .. } => (
            "Cerveau du vehicule : memoire, taches et autorite de commande.",
            "Chaque classe augmente la memoire et les taches, mais aussi la masse et le cout.",
            "Un seul ordinateur par vehicule, et il doit rester dans le dernier etage.",
        ),
        PartSpec::Decoupler => (
            "Separe deux etages : la piece du dessus devient un etage autonome.",
            "Sa masse est faible ; il ne change pas les performances du moteur.",
            "Le premier bloc d'un etage superieur est generalement une separation.",
        ),
        PartSpec::MissionModule { .. } => (
            "Charge utile abstraite : elle compte dans la masse au decollage et dans le contrat.",
            "La masse regle directement le poids emporte et la difficulte de la mission.",
            "Un module sans cellule d'alimentation suffisante sera signale par l'avionique.",
        ),
        PartSpec::Explosive { .. } => (
            "Charge militaire : sa masse et son rendement sont lus par les contrats.",
            "Une charge amelioree convertit la meme masse en plus de rendement abstrait.",
            "Le boitier et la charge comptent dans la masse au decollage ; verifiez la limite du contrat.",
        ),
        PartSpec::Adapter { .. } => (
            "Transition de diametre entre deux corps, sans saut visuel ni avertissement.",
            "Le diametre d'entree et de sortie doivent correspondre aux pieces voisines.",
            "Il ne cree pas d'etage : la hierarchie reste celle du parent.",
        ),
        PartSpec::NoseCone { .. } => (
            "Coiffe placee par le joueur : elle ferme le sommet de la pile.",
            "Longueur, diametre de base et profil changent la silhouette et la masse de coque.",
            "Terminale : retirez-la avant d'ajouter une autre piece au-dessus.",
        ),
        PartSpec::Wing { .. } => (
            "Aile portante : elle cree de la portance et deplace le centre aerodynamique.",
            "Envergure, corde, fleche et profil arbitrent portance, trainee et stabilite.",
            "Elle se monte sur un point radial libre et demande un profil de vol adapte.",
        ),
        PartSpec::FinSet { .. } => (
            "Ailettes stabilisatrices : elles ramenent le centre aerodynamique derriere le CG.",
            "Le role, la configuration et la construction legere changent stabilite et masse.",
            "Elles se montent uniquement sur les points radiaux d'un corps.",
        ),
        PartSpec::ControlSurface { .. } => (
            "Surface mobile : elle augmente l'autorite de commande en vol.",
            "Une surface fixe stabilise ; une surface mobile demande un ordinateur compatible.",
            "Sans ordinateur ou sans programme de guidage, son autorite ne sert pas.",
        ),
        PartSpec::Avionics { module } => (
            "Electronique de mission : commande, navigation, liaison, alimentation ou diagnostic.",
            match module {
                AvionicsModule::ControlAugment => "Ajoute de l'autorite au prix d'un peu de fiabilite.",
                AvionicsModule::InertialNav => "Ameliore la solution de ciblage et le verrouillage.",
                AvionicsModule::Datalink => "Ameliore encore le verrouillage, mais exige la navigation.",
                AvionicsModule::PowerCell => "Fournit le courant ; plusieurs cellules sont autorisees.",
                AvionicsModule::Diagnostics => "Reduit l'exposition aux pannes de la cellule.",
            },
            "Verifiez les dependances et la marge electrique dans le bilan des etages.",
        ),
    };
    vec![role.to_string(), effect.to_string(), caution.to_string()]
}

/// One-line palette description, used by the hover hint.
pub(crate) fn template_description(template: PartTemplate) -> &'static str {
    match template {
        PartTemplate::SolidMotor => "Poussee initiale, combustion courte.",
        PartTemplate::FuelTank => "Propergol pour les moteurs liquides et a air.",
        PartTemplate::LiquidEngine => "Moteur principal alimente par reservoir.",
        PartTemplate::AirBreathing => "Croisiere econome, necessite de la vitesse.",
        PartTemplate::Pitot => "Vitesse relative pour le pilote automatique.",
        PartTemplate::Barometer => "Altitude pression pour les consignes.",
        PartTemplate::RadarBasic => "Verrouillage de cible simple.",
        PartTemplate::RadarActive => "Verrouillage plus rapide et plus fiable.",
        PartTemplate::ComputerBasic => "Memoire et taches de base.",
        PartTemplate::ComputerStandard => "Plus de taches et d'autorite.",
        PartTemplate::ComputerAdvanced => "La classe d'ordinateur la plus complete.",
        PartTemplate::ControlAugment => "Autorite de commande accrue.",
        PartTemplate::InertialNav => "Navigation et solution de ciblage.",
        PartTemplate::Datalink => "Liaison de donnees pour le verrouillage.",
        PartTemplate::PowerCell => "Alimente l'electronique de mission.",
        PartTemplate::Diagnostics => "Reduit l'exposition aux pannes.",
        PartTemplate::Decoupler => "Separe deux etages.",
        PartTemplate::MissionModule => "Charge utile abstraite.",
        PartTemplate::ExplosiveStandard => "Charge militaire standard.",
        PartTemplate::ExplosiveImproved => "Meilleur rendement pour la meme masse.",
        PartTemplate::ExplosiveAdvanced => "Charge militaire la plus efficace.",
        PartTemplate::Adapter => "Transition de diametre entre deux corps.",
        PartTemplate::Nose => "Coiffe terminale reglable.",
        PartTemplate::Wing => "Aile portante pour la croisiere.",
        PartTemplate::FinSet => "Ailettes de stabilisation.",
        PartTemplate::ControlSurface => "Surface mobile d'autorite.",
    }
}

/// Discrete design choices of an aerodynamic part, cycled from the inspector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AeroChoice {
    Planform,
    Profile,
    AdapterShape,
    NoseShape,
    Angle,
    Role,
    Layout,
    ControlKind,
    Light,
}

impl AeroChoice {
    pub fn label(self) -> &'static str {
        match self {
            Self::Planform => "PLANFORM",
            Self::Profile => "PROFILE",
            Self::AdapterShape | Self::NoseShape => "SHAPE",
            Self::Angle => "ANGLE",
            Self::Role => "STABILISER",
            Self::Layout => "LAYOUT",
            Self::ControlKind => "SURFACE",
            Self::Light => "BUILD",
        }
    }
}

pub(crate) fn choices_of(spec: &PartSpec) -> Vec<AeroChoice> {
    match *spec {
        PartSpec::Wing { .. } => vec![
            AeroChoice::Planform,
            AeroChoice::Profile,
            AeroChoice::Angle,
            AeroChoice::Light,
        ],
        PartSpec::FinSet { .. } => vec![AeroChoice::Role, AeroChoice::Layout, AeroChoice::Light],
        PartSpec::ControlSurface { .. } => vec![AeroChoice::ControlKind, AeroChoice::Light],
        PartSpec::Adapter { .. } => vec![AeroChoice::AdapterShape],
        PartSpec::NoseCone { .. } => vec![AeroChoice::NoseShape],
        _ => Vec::new(),
    }
}

pub(crate) fn choice_value(spec: &PartSpec, choice: AeroChoice) -> String {
    match (*spec, choice) {
        (PartSpec::Wing { planform, .. }, AeroChoice::Planform) => planform.label().to_string(),
        (PartSpec::Wing { profile, .. }, AeroChoice::Profile) => profile.label().to_string(),
        (PartSpec::Adapter { shape, .. }, AeroChoice::AdapterShape) => shape.label().to_string(),
        (PartSpec::NoseCone { shape, .. }, AeroChoice::NoseShape) => shape.label().to_string(),
        (_, AeroChoice::Angle) => "USE PART ANGLE".to_string(),
        (PartSpec::FinSet { role, .. }, AeroChoice::Role) => role.label().to_string(),
        (PartSpec::FinSet { layout, .. }, AeroChoice::Layout) => layout.label().to_string(),
        (PartSpec::ControlSurface { kind, .. }, AeroChoice::ControlKind) => kind.label().to_string(),
        (
            PartSpec::Wing { light, .. } | PartSpec::FinSet { light, .. } | PartSpec::ControlSurface { light, .. },
            AeroChoice::Light,
        ) => if light { "LIGHT" } else { "STANDARD" }.to_string(),
        _ => "-".to_string(),
    }
}

/// Advance one discrete design choice; `None` when the part does not offer it.
pub(crate) fn choice_value_for_part(part: &omega_rules::PlacedPart, choice: AeroChoice) -> String {
    if choice == AeroChoice::Angle {
        return format!(
            "{:+.0}°",
            part.attach.as_ref().map_or(0.0, |attach| attach.rotation_x_deg)
        );
    }
    choice_value(&part.spec, choice)
}

pub(crate) fn cycled(spec: PartSpec, choice: AeroChoice) -> Option<PartSpec> {
    let toggled = |light: bool| !light;
    match (spec, choice) {
        (
            PartSpec::Wing {
                span_m,
                chord_m,
                sweep_deg,
                planform,
                profile,
                light,
            },
            AeroChoice::Planform,
        ) => Some(PartSpec::Wing {
            span_m,
            chord_m,
            sweep_deg,
            planform: planform.next(),
            profile,
            light,
        }),
        (
            PartSpec::Wing {
                span_m,
                chord_m,
                sweep_deg,
                planform,
                profile,
                light,
            },
            AeroChoice::Profile,
        ) => Some(PartSpec::Wing {
            span_m,
            chord_m,
            sweep_deg,
            planform,
            profile: profile.next(),
            light,
        }),
        (
            PartSpec::Wing {
                span_m,
                chord_m,
                sweep_deg,
                planform,
                profile,
                light,
            },
            AeroChoice::Light,
        ) => Some(PartSpec::Wing {
            span_m,
            chord_m,
            sweep_deg,
            planform,
            profile,
            light: toggled(light),
        }),
        (
            PartSpec::FinSet {
                span_m,
                chord_m,
                role,
                layout,
                light,
            },
            AeroChoice::Role,
        ) => Some(PartSpec::FinSet {
            span_m,
            chord_m,
            role: role.next(),
            layout,
            light,
        }),
        (
            PartSpec::FinSet {
                span_m,
                chord_m,
                role,
                layout,
                light,
            },
            AeroChoice::Layout,
        ) => Some(PartSpec::FinSet {
            span_m,
            chord_m,
            role,
            layout: layout.next(),
            light,
        }),
        (
            PartSpec::FinSet {
                span_m,
                chord_m,
                role,
                layout,
                light,
            },
            AeroChoice::Light,
        ) => Some(PartSpec::FinSet {
            span_m,
            chord_m,
            role,
            layout,
            light: toggled(light),
        }),
        (
            PartSpec::ControlSurface {
                span_m,
                chord_m,
                kind,
                light,
            },
            AeroChoice::ControlKind,
        ) => Some(PartSpec::ControlSurface {
            span_m,
            chord_m,
            kind: kind.next(),
            light,
        }),
        (
            PartSpec::ControlSurface {
                span_m,
                chord_m,
                kind,
                light,
            },
            AeroChoice::Light,
        ) => Some(PartSpec::ControlSurface {
            span_m,
            chord_m,
            kind,
            light: toggled(light),
        }),
        (
            PartSpec::Adapter {
                length_m,
                diameter_m,
                exit_diameter_m,
                shape,
            },
            AeroChoice::AdapterShape,
        ) => Some(PartSpec::Adapter {
            length_m,
            diameter_m,
            exit_diameter_m,
            shape: shape.next(),
        }),
        (
            PartSpec::NoseCone {
                length_m,
                diameter_m,
                shape,
            },
            AeroChoice::NoseShape,
        ) => Some(PartSpec::NoseCone {
            length_m,
            diameter_m,
            shape: shape.next(),
        }),
        _ => None,
    }
}

/// `direction` is +1 or -1; the result is clamped to the part's valid range.
pub(crate) fn adjusted(spec: PartSpec, param: Param, direction: f64) -> Option<PartSpec> {
    let round = |v: f64| (v * 1000.0).round() / 1000.0;
    let delta = param.step() * direction;
    let changed = match (spec, param) {
        (
            PartSpec::SolidMotor {
                length_m,
                diameter_m,
                burn_time_s,
            },
            Param::Length,
        ) => PartSpec::SolidMotor {
            length_m: round((length_m + delta).clamp(0.3, 8.0)),
            diameter_m,
            burn_time_s,
        },
        (
            PartSpec::SolidMotor {
                length_m,
                diameter_m,
                burn_time_s,
            },
            Param::Diameter,
        ) => PartSpec::SolidMotor {
            length_m,
            diameter_m: round((diameter_m + delta).clamp(0.1, 1.2)),
            burn_time_s,
        },
        (
            PartSpec::SolidMotor {
                length_m,
                diameter_m,
                burn_time_s,
            },
            Param::BurnTime,
        ) => PartSpec::SolidMotor {
            length_m,
            diameter_m,
            burn_time_s: round((burn_time_s + delta).clamp(1.0, 60.0)),
        },
        (PartSpec::FuelTank { length_m, diameter_m }, Param::Length) => PartSpec::FuelTank {
            length_m: round((length_m + delta).clamp(0.3, 8.0)),
            diameter_m,
        },
        (PartSpec::FuelTank { length_m, diameter_m }, Param::Diameter) => PartSpec::FuelTank {
            length_m,
            diameter_m: round((diameter_m + delta).clamp(0.1, 1.2)),
        },
        (PartSpec::LiquidEngine { scale }, Param::Scale) => PartSpec::LiquidEngine {
            scale: round((scale + delta).clamp(0.3, 3.0)),
        },
        (PartSpec::AirBreathing { scale }, Param::Scale) => PartSpec::AirBreathing {
            scale: round((scale + delta).clamp(0.3, 3.0)),
        },
        (PartSpec::MissionModule { mass_kg }, Param::Mass) => PartSpec::MissionModule {
            mass_kg: round((mass_kg + delta).clamp(1.0, 300.0)),
        },
        (PartSpec::Explosive { mass_kg, grade }, Param::Mass) => PartSpec::Explosive {
            mass_kg: round((mass_kg + delta).clamp(5.0, 300.0)),
            grade,
        },
        (
            PartSpec::Adapter {
                length_m,
                diameter_m,
                exit_diameter_m,
                shape,
            },
            Param::Length,
        ) => PartSpec::Adapter {
            length_m: round((length_m + delta).clamp(0.1, 2.0)),
            diameter_m,
            exit_diameter_m,
            shape,
        },
        (
            PartSpec::Adapter {
                length_m,
                diameter_m,
                exit_diameter_m,
                shape,
            },
            Param::Diameter,
        ) => PartSpec::Adapter {
            length_m,
            diameter_m: round((diameter_m + delta).clamp(0.05, 1.2)),
            exit_diameter_m,
            shape,
        },
        (
            PartSpec::Adapter {
                length_m,
                diameter_m,
                exit_diameter_m,
                shape,
            },
            Param::ExitDiameter,
        ) => PartSpec::Adapter {
            length_m,
            diameter_m,
            exit_diameter_m: round((exit_diameter_m + delta).clamp(0.05, 1.2)),
            shape,
        },
        (
            PartSpec::NoseCone {
                length_m,
                diameter_m,
                shape,
            },
            Param::Length,
        ) => PartSpec::NoseCone {
            length_m: round((length_m + delta).clamp(0.1, 3.0)),
            diameter_m,
            shape,
        },
        (
            PartSpec::NoseCone {
                length_m,
                diameter_m,
                shape,
            },
            Param::Diameter,
        ) => PartSpec::NoseCone {
            length_m,
            diameter_m: round((diameter_m + delta).clamp(0.05, 1.2)),
            shape,
        },
        (
            PartSpec::Wing {
                span_m,
                chord_m,
                sweep_deg,
                planform,
                profile,
                light,
            },
            Param::Span,
        ) => PartSpec::Wing {
            span_m: round((span_m + delta).clamp(0.2, 4.0)),
            chord_m,
            sweep_deg,
            planform,
            profile,
            light,
        },
        (
            PartSpec::Wing {
                span_m,
                chord_m,
                sweep_deg,
                planform,
                profile,
                light,
            },
            Param::Chord,
        ) => PartSpec::Wing {
            span_m,
            chord_m: round((chord_m + delta).clamp(0.1, 2.0)),
            sweep_deg,
            planform,
            profile,
            light,
        },
        (
            PartSpec::Wing {
                span_m,
                chord_m,
                sweep_deg,
                planform,
                profile,
                light,
            },
            Param::Sweep,
        ) => PartSpec::Wing {
            span_m,
            chord_m,
            sweep_deg: round((sweep_deg + delta).clamp(0.0, 60.0)),
            planform,
            profile,
            light,
        },
        (
            PartSpec::FinSet {
                span_m,
                chord_m,
                role,
                layout,
                light,
            },
            Param::Span,
        ) => PartSpec::FinSet {
            span_m: round((span_m + delta).clamp(0.2, 4.0)),
            chord_m,
            role,
            layout,
            light,
        },
        (
            PartSpec::FinSet {
                span_m,
                chord_m,
                role,
                layout,
                light,
            },
            Param::Chord,
        ) => PartSpec::FinSet {
            span_m,
            chord_m: round((chord_m + delta).clamp(0.1, 2.0)),
            role,
            layout,
            light,
        },
        (
            PartSpec::ControlSurface {
                span_m,
                chord_m,
                kind,
                light,
            },
            Param::Span,
        ) => PartSpec::ControlSurface {
            span_m: round((span_m + delta).clamp(0.2, 4.0)),
            chord_m,
            kind,
            light,
        },
        (
            PartSpec::ControlSurface {
                span_m,
                chord_m,
                kind,
                light,
            },
            Param::Chord,
        ) => PartSpec::ControlSurface {
            span_m,
            chord_m: round((chord_m + delta).clamp(0.1, 2.0)),
            kind,
            light,
        },
        _ => return None,
    };
    Some(changed)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MissileAction {
    /// Kept for tests and saved key bindings; the palette itself drags (see `workshop`).
    #[allow(dead_code)]
    AddPart(PartTemplate),
    Select {
        id: String,
    },
    ToggleSelect {
        id: String,
    },
    SelectGroup {
        id: String,
    },
    ClearSelection,
    ToggleCollapsed {
        id: String,
    },
    ToggleHidden {
        id: String,
    },
    ToggleLock {
        id: String,
    },
    BeginRename {
        id: String,
    },
    CommitRename,
    CancelRename,
    ToggleFavorite(PartTemplate),
    ClearPaletteSearch,
    Adjust(Param, i8),
    CycleAero(AeroChoice),
    /// Keep the current aerodynamic figures as the A side of a comparison.
    CompareAero,
    ClearComparison,
    RemovePart,
    Preset(u8),
    Clear,
    CycleSymmetry,
    CycleSymmetryKind,
    BreakSymmetry,
    LinkSymmetry,
    DuplicateSelection,
    SaveSubassembly(SubassemblyKind),
    ApplySubassembly(usize),
    DeleteSubassembly(usize),
    ExportSubassembly(usize),
    ImportSubassemblies,
    SetPaletteCategory(Option<PartCategory>),
    Undo,
    Redo,
}

/// Why the assembly control panel lists a row, so its header can group the problems the same way
/// a builder thinks about them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AssemblyIssueKind {
    Unconnected,
    Occupied,
    Incompatible,
    Locked,
    Subassembly,
}

impl AssemblyIssueKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unconnected => "CONNEXIONS",
            Self::Occupied => "POINTS OCCUPÉS",
            Self::Incompatible => "INCOMPATIBILITÉS",
            Self::Locked => "VERROUS",
            Self::Subassembly => "SOUS-ENSEMBLES",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AssemblyIssue {
    pub kind: AssemblyIssueKind,
    pub severity: IssueSeverity,
    pub message: String,
    pub part: Option<String>,
}

#[derive(Resource)]
pub(crate) struct MissileSession {
    pub vehicle: Vehicle,
    /// Primary selection: the part the inspector and the gizmo act on.
    pub selected: Option<String>,
    /// Every selected part. `selected` is always one of these when it is set.
    pub selection: BTreeSet<String>,
    /// Collapsed subtree roots in the assembly tree.
    pub collapsed: BTreeSet<String>,
    /// Subtree roots hidden in the 3D view. Hidden parts still count in the rules.
    pub hidden: BTreeSet<String>,
    /// Parts protected from edition, including their subtree.
    pub locked: BTreeSet<String>,
    /// Player names for subtree roots.
    pub group_names: BTreeMap<String, String>,
    /// Subtree root currently being renamed, with the draft text.
    pub rename_target: Option<String>,
    pub rename_draft: String,
    /// Palette discovery and organization, kept only for the current session.
    pub favorites: Vec<PartTemplate>,
    pub recent: Vec<PartTemplate>,
    pub palette_search: String,
    /// Radial copies placed by the next drop: 1 (off) through 8.
    pub symmetry: u8,
    /// Generation used by the next drop or reassociation.
    pub symmetry_kind: SymmetryKind,
    pub palette_category: Option<PartCategory>,
    /// Reusable subassemblies saved with this design slot.
    pub library: Vec<Subassembly>,
    /// Set when `library` differs from the file on disk.
    pub library_dirty: bool,
    next_id: u32,
    /// Serial for symmetry group ids, so reassociating parts cannot collide with an older set.
    pub next_symmetry: u32,
    undo: Vec<Vehicle>,
    redo: Vec<Vehicle>,
    /// The continuous wheel gesture in progress: its snapshot is pushed once, on the first notch.
    wheel_gesture: Option<((String, Param), Instant)>,
    /// Aerodynamic figures kept as the A side of the workshop comparison.
    pub comparison: Option<omega_rules::AeroFigures>,
}

impl Default for MissileSession {
    fn default() -> Self {
        Self {
            vehicle: Vehicle::default(),
            selected: None,
            selection: BTreeSet::new(),
            collapsed: BTreeSet::new(),
            hidden: BTreeSet::new(),
            locked: BTreeSet::new(),
            group_names: BTreeMap::new(),
            rename_target: None,
            rename_draft: String::new(),
            favorites: Vec::new(),
            recent: Vec::new(),
            palette_search: String::new(),
            symmetry: 1,
            symmetry_kind: SymmetryKind::Radial,
            palette_category: None,
            library: Vec::new(),
            library_dirty: false,
            next_id: 0,
            next_symmetry: 1,
            undo: Vec::new(),
            redo: Vec::new(),
            wheel_gesture: None,
            comparison: None,
        }
    }
}

fn part(id: &str, spec: PartSpec) -> Part {
    Part {
        id: id.to_string(),
        spec,
    }
}

impl MissileSession {
    pub fn new_default() -> Self {
        let mut session = Self::default();
        session.load_preset(0);
        session
    }

    fn fresh_id(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{prefix}-{}", self.next_id)
    }

    fn sync_next_id(&mut self) {
        self.next_id = self
            .vehicle
            .parts
            .iter()
            .filter_map(|p| p.id.rsplit('-').next().and_then(|n| n.parse::<u32>().ok()))
            .max()
            .unwrap_or(0);
        // A loaded design may already contain `sym-<n>-...` sets; never reuse one of their keys
        // for a new drop or reassociation, or two independent sets would start moving together.
        let next = self
            .vehicle
            .parts
            .iter()
            .filter_map(|part| part.symmetry.as_deref())
            .filter_map(|group| {
                let (key, _, _) = decode_symmetry(group);
                key.strip_prefix("sym-")?.parse::<u32>().ok()
            })
            .max()
            .unwrap_or(0)
            + 1;
        self.next_symmetry = self.next_symmetry.max(next);
    }

    pub fn selected_part(&self) -> Option<&omega_rules::PlacedPart> {
        self.selected.as_deref().and_then(|id| self.vehicle.part(id))
    }

    pub fn is_selected(&self, id: &str) -> bool {
        self.selection.contains(id)
    }

    /// Replace the selection with one part (or clear it).
    pub fn select_only(&mut self, id: Option<String>) {
        self.selection.clear();
        self.selected = id.filter(|id| self.vehicle.part(id).is_some());
        if let Some(id) = &self.selected {
            self.selection.insert(id.clone());
        }
    }

    /// Ctrl+click: add or remove one part without disturbing the rest of the selection.
    pub fn toggle_selection(&mut self, id: &str) -> String {
        if self.vehicle.part(id).is_none() {
            return "That part is no longer there".to_string();
        }
        if self.selection.remove(id) {
            if self.selected.as_deref() == Some(id) {
                self.selected = self.selection.iter().next().cloned();
            }
            format!("Deselected {id}")
        } else {
            self.selection.insert(id.to_string());
            self.selected = Some(id.to_string());
            format!("Selected {id} ({} parts)", self.selection.len())
        }
    }

    /// Select a complete subtree, which is how the tree's group button works.
    pub fn select_group(&mut self, id: &str) -> String {
        if self.vehicle.part(id).is_none() {
            return "That part is no longer there".to_string();
        }
        let ids = self.vehicle.subtree_ids(id);
        self.selection = ids.iter().cloned().collect();
        self.selected = Some(id.to_string());
        format!("Selected {} part(s) under {id}", ids.len())
    }

    pub fn clear_selection(&mut self) -> String {
        if self.selection.is_empty() {
            return String::new();
        }
        self.selection.clear();
        self.selected = None;
        "Selection cleared".to_string()
    }

    /// Selected parts that are not inside another selected part; moving or deleting these roots
    /// cannot apply an edit twice to the same subtree.
    pub fn selected_roots(&self) -> Vec<String> {
        let selected: Vec<&String> = self.selection.iter().collect();
        selected
            .iter()
            .filter(|id| {
                !selected.iter().any(|other| {
                    other.as_str() != id.as_str() && self.vehicle.is_descendant(other.as_str(), id.as_str())
                })
            })
            .map(|id| (*id).clone())
            .collect()
    }

    /// A lock protects the part and everything attached below it.
    pub fn is_locked(&self, id: &str) -> bool {
        self.locked
            .iter()
            .any(|locked| locked == id || self.vehicle.is_descendant(locked, id))
    }

    /// True when the part or any part below it is locked. Deleting or moving the root would
    /// otherwise silently move a locked child even though the child itself is protected.
    pub fn subtree_locked(&self, id: &str) -> bool {
        let ids = self.vehicle.subtree_ids(id);
        self.locked.iter().any(|locked| ids.contains(locked))
    }

    /// Editing a part is forbidden when it is itself under a lock or when its edit would carry a
    /// locked descendant (deleting/moving a whole subtree).
    pub fn edit_locked(&self, id: &str) -> bool {
        self.is_locked(id) || self.subtree_locked(id)
    }

    /// A hide applies to the whole subtree, including children mounted under the hidden root.
    pub fn hidden_parts(&self) -> BTreeSet<String> {
        let mut parts = BTreeSet::new();
        for id in &self.hidden {
            parts.extend(self.vehicle.subtree_ids(id));
        }
        parts
    }

    pub fn group_name(&self, id: &str) -> Option<&str> {
        self.group_names
            .get(id)
            .map(String::as_str)
            .filter(|name| !name.is_empty())
    }

    /// Rows the tree should show: a collapsed row hides every following row deeper than it.
    pub fn tree_rows(&self) -> Vec<omega_rules::TreeEntry> {
        let mut rows = Vec::new();
        let mut collapsed_depth = None;
        for entry in self.vehicle.tree_entries() {
            if let Some(depth) = collapsed_depth {
                if entry.depth > depth {
                    continue;
                }
                collapsed_depth = None;
            }
            if self.collapsed.contains(&entry.id) {
                collapsed_depth = Some(entry.depth);
            }
            rows.push(entry);
        }
        rows
    }

    pub fn palette_matches(&self, template: PartTemplate) -> bool {
        let query = self.palette_search.trim().to_lowercase();
        query.is_empty()
            || template.label().to_lowercase().contains(&query)
            || part_help(&template.spec())
                .iter()
                .any(|line| line.to_lowercase().contains(&query))
    }

    pub fn note_recent(&mut self, template: PartTemplate) {
        self.recent.retain(|entry| *entry != template);
        self.recent.insert(0, template);
        self.recent.truncate(6);
    }

    /// Tree and palette state that must redraw the board when it changes.
    pub fn ui_signature(&self) -> String {
        let library: Vec<(&str, &str, usize)> = self
            .library
            .iter()
            .map(|entry| (entry.name.as_str(), entry.kind.short_label(), entry.parts.len()))
            .collect();
        format!(
            "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{}|{:?}|{}|{:?}|{:?}",
            self.selected,
            self.selection,
            self.collapsed,
            self.hidden,
            self.locked,
            self.group_names,
            self.favorites,
            self.recent,
            self.palette_search,
            self.rename_target,
            self.rename_draft,
            self.symmetry_kind,
            library
        )
    }

    /// Drop editor state that refers to parts removed by an edit or an undo.
    fn retain_editor_state(&mut self) {
        let ids: BTreeSet<String> = self.vehicle.parts.iter().map(|part| part.id.clone()).collect();
        self.selection.retain(|id| ids.contains(id));
        self.collapsed.retain(|id| ids.contains(id));
        self.hidden.retain(|id| ids.contains(id));
        self.locked.retain(|id| ids.contains(id));
        self.group_names.retain(|id, _| ids.contains(id));
        if self.rename_target.as_ref().is_some_and(|id| !ids.contains(id)) {
            self.rename_target = None;
            self.rename_draft.clear();
        }
        if self.selected.as_ref().is_some_and(|id| !ids.contains(id)) {
            self.selected = self.selection.iter().next().cloned();
        }
        if self.selected.is_none() {
            self.selected = self.selection.iter().next().cloned();
        }
        if self.selected.is_some() && self.selection.is_empty() {
            self.select_only(self.selected.clone());
        }
    }

    pub fn load_preset(&mut self, index: u8) {
        self.vehicle = match index {
            0 | 2 => Vehicle::from_stages(
                "Two-stage rocket",
                vec![
                    Stage {
                        parts: vec![
                            part(
                                "motor-1",
                                PartSpec::SolidMotor {
                                    length_m: 2.0,
                                    diameter_m: 0.4,
                                    burn_time_s: 20.0,
                                },
                            ),
                            part("decoupler-2", PartSpec::Decoupler),
                        ],
                    },
                    Stage {
                        parts: vec![
                            part(
                                "motor-3",
                                PartSpec::SolidMotor {
                                    length_m: 1.0,
                                    diameter_m: 0.3,
                                    burn_time_s: 25.0,
                                },
                            ),
                            part(
                                "computer-4",
                                PartSpec::Computer {
                                    class: ComputerClass::Basic,
                                },
                            ),
                            part(
                                "pitot-5",
                                PartSpec::Sensor {
                                    kind: SensorKind::Pitot,
                                },
                            ),
                            part("module-6", PartSpec::MissionModule { mass_kg: 40.0 }),
                        ],
                    },
                ],
            ),
            _ => Vehicle::from_stages(
                "Cruise vehicle",
                vec![Stage {
                    parts: vec![
                        part(
                            "tank-1",
                            PartSpec::FuelTank {
                                length_m: 1.5,
                                diameter_m: 0.3,
                            },
                        ),
                        part("turbofan-2", PartSpec::AirBreathing { scale: 1.0 }),
                        part(
                            "computer-3",
                            PartSpec::Computer {
                                class: ComputerClass::Basic,
                            },
                        ),
                        part(
                            "pitot-4",
                            PartSpec::Sensor {
                                kind: SensorKind::Pitot,
                            },
                        ),
                        part("module-5", PartSpec::MissionModule { mass_kg: 30.0 }),
                    ],
                }],
            ),
        };
        // Two presets exist to teach the airframe: fins make a small rocket stable, wings make the
        // cruise vehicle lift. Both stay research-gated like the other presets.
        if index == 2 {
            self.vehicle.name = "Fin trainer".to_string();
            for slot in 0..4u8 {
                self.vehicle
                    .add_attached(
                        format!("fin-{}", slot + 5),
                        PartSpec::FinSet {
                            span_m: 0.35,
                            chord_m: 0.30,
                            role: StabilizerRole::Rear,
                            layout: FinLayout::Symmetric,
                            light: false,
                        },
                        "motor-1",
                        omega_rules::NodeId::Radial(slot),
                    )
                    .expect("the fin trainer has free radial slots");
            }
        }
        if index == 3 {
            self.vehicle.name = "Winged cruise".to_string();
            // Opposite mounts keep both wings horizontal once the cruise is laid out.
            for slot in [0u8, 2u8] {
                self.vehicle
                    .add_attached(
                        format!("wing-{}", slot + 6),
                        PartSpec::Wing {
                            span_m: 1.2,
                            chord_m: 0.4,
                            sweep_deg: 20.0,
                            planform: WingPlanform::Swept,
                            profile: AirfoilProfile::Classic,
                            light: false,
                        },
                        "tank-1",
                        omega_rules::NodeId::Radial(slot),
                    )
                    .expect("the winged cruise has free radial slots");
            }
        }
        self.collapsed.clear();
        self.hidden.clear();
        self.locked.clear();
        self.group_names.clear();
        self.rename_target = None;
        self.rename_draft.clear();
        self.select_only(self.vehicle.topmost_id());
        self.sync_next_id();
    }

    /// Apply a workshop action. Returns the status message to show.
    pub fn apply(&mut self, action: MissileAction, owned: &BTreeSet<String>) -> String {
        // A button edit (including +/-) closes any running wheel gesture.
        self.wheel_gesture = None;
        match action {
            MissileAction::AddPart(template) => self.add_part(template, owned),
            MissileAction::Select { id } => {
                if self.vehicle.part(&id).is_some() {
                    self.select_only(Some(id));
                }
                String::new()
            }
            MissileAction::ToggleSelect { id } => self.toggle_selection(&id),
            MissileAction::SelectGroup { id } => self.select_group(&id),
            MissileAction::ClearSelection => self.clear_selection(),
            MissileAction::ClearPaletteSearch => {
                self.palette_search.clear();
                "Search cleared".to_string()
            }
            MissileAction::ToggleCollapsed { id } => {
                if self.collapsed.remove(&id) {
                    format!("Expanded {id}")
                } else {
                    self.collapsed.insert(id.clone());
                    format!("Collapsed {id}")
                }
            }
            MissileAction::ToggleHidden { id } => {
                if self.hidden.remove(&id) {
                    format!("Shown {id}")
                } else {
                    self.hidden.insert(id.clone());
                    format!("Hidden {id} and its subtree; the rules still count it")
                }
            }
            MissileAction::ToggleLock { id } => {
                if self.locked.remove(&id) {
                    format!("Unlocked {id}")
                } else {
                    self.locked.insert(id.clone());
                    format!("Locked {id} and its subtree")
                }
            }
            MissileAction::BeginRename { id } => {
                if self.vehicle.part(&id).is_none() {
                    return "That part is no longer there".to_string();
                }
                self.rename_draft = self.group_name(&id).unwrap_or_default().to_string();
                self.rename_target = Some(id.clone());
                format!("Rename {id}: type a name, Enter to confirm, Escape to cancel")
            }
            MissileAction::CommitRename => {
                let Some(id) = self.rename_target.take() else {
                    return String::new();
                };
                let name = self.rename_draft.trim().to_string();
                self.rename_draft.clear();
                if name.is_empty() {
                    self.group_names.remove(&id);
                    format!("Group name cleared for {id}")
                } else {
                    self.group_names.insert(id.clone(), name.clone());
                    format!("Group {id} renamed to {name}")
                }
            }
            MissileAction::CancelRename => {
                self.rename_target = None;
                self.rename_draft.clear();
                "Rename cancelled".to_string()
            }
            MissileAction::ToggleFavorite(template) => {
                if let Some(index) = self.favorites.iter().position(|entry| *entry == template) {
                    self.favorites.remove(index);
                    format!("{} removed from favourites", template.label())
                } else {
                    self.favorites.push(template);
                    format!("{} added to favourites", template.label())
                }
            }
            MissileAction::Adjust(param, direction) => self.adjust_selected(param, direction, true),
            MissileAction::CycleAero(choice) => self.cycle_aero(choice, owned),
            MissileAction::CompareAero => {
                self.comparison = Some(self.vehicle.aero());
                "Aerodynamic snapshot A saved; edit the design to compare".to_string()
            }
            MissileAction::ClearComparison => {
                self.comparison = None;
                "Comparison cleared".to_string()
            }
            MissileAction::RemovePart => self.delete_selected(),
            MissileAction::Preset(index) => {
                let mut candidate = Self::default();
                candidate.load_preset(index);
                let missing = candidate.vehicle.missing_research(owned);
                if !missing.is_empty() {
                    return format!("Preset locked: research {} first", missing.join(", "));
                }
                // A preset starts a new history; the old design is not one undo away. Palette
                // discovery follows the player across presets.
                let favorites = std::mem::take(&mut self.favorites);
                let recent = std::mem::take(&mut self.recent);
                *self = candidate;
                self.favorites = favorites;
                self.recent = recent;
                format!("Loaded preset: {}", self.vehicle.name)
            }
            MissileAction::Clear => {
                self.push_undo();
                self.vehicle = Vehicle {
                    name: "New vehicle".into(),
                    parts: Vec::new(),
                };
                self.select_only(None);
                self.collapsed.clear();
                self.hidden.clear();
                self.locked.clear();
                self.group_names.clear();
                self.rename_target = None;
                self.rename_draft.clear();
                self.comparison = None;
                "Started an empty vehicle: drag a part from the palette".to_string()
            }
            MissileAction::CycleSymmetry => self.cycle_symmetry(),
            MissileAction::CycleSymmetryKind => self.cycle_symmetry_kind(),
            MissileAction::BreakSymmetry => self.break_symmetry(),
            MissileAction::LinkSymmetry => self.link_symmetry(),
            MissileAction::DuplicateSelection => self.duplicate_selected(),
            MissileAction::SaveSubassembly(kind) => self.save_subassembly(kind),
            MissileAction::ApplySubassembly(index) => self.apply_subassembly(index, owned),
            MissileAction::DeleteSubassembly(index) => self.delete_subassembly(index),
            MissileAction::ExportSubassembly(index) => self.export_subassembly(index),
            MissileAction::ImportSubassemblies => self.import_subassemblies(),
            MissileAction::SetPaletteCategory(category) => {
                self.palette_category = category;
                String::new()
            }
            MissileAction::Undo => self.undo(),
            MissileAction::Redo => self.redo(),
        }
    }

    /// Adjust the selected part. `snapshot` is false for the notches of a wheel gesture that is
    /// still running: the gesture's first notch already captured the state to return to.
    fn adjust_selected(&mut self, param: Param, direction: i8, snapshot: bool) -> String {
        let Some(id) = self.selected.clone() else {
            return "Select a part first".to_string();
        };
        let Some(current) = self.vehicle.part(&id).map(|part| part.spec) else {
            return String::new();
        };
        if self.is_locked(&id) {
            return format!("{id} est verrouillée : déverrouillez-la d'abord.");
        }
        if param == Param::Size {
            return self.adjust_selected_size(&id, direction, snapshot);
        }
        match adjusted(current, param, f64::from(direction)) {
            Some(spec) => {
                let members = self.vehicle.symmetry_members(&id);
                if members.len() > 1 {
                    if let Some(locked) = members.iter().find(|member| self.is_locked(member)) {
                        return format!("{locked} est dans le même groupe de symétrie et verrouillée.");
                    }
                    if snapshot {
                        self.push_undo();
                    }
                    match self.replace_in_symmetry_group(&id, spec) {
                        Ok(count) => format!("{} : {} copie(s) ajustée(s)", current.short_label(), count),
                        Err(error) => error,
                    }
                } else {
                    if snapshot {
                        self.push_undo();
                    }
                    self.vehicle.replace_part(&id, spec);
                    String::new()
                }
            }
            None => "This part has no such setting".to_string(),
        }
    }

    /// Set the mounting diameter of a fixed-cross-section part through its per-part scale. The
    /// inspector shows metres, so a 0.30 m computer can be brought to a 0.40 m body in two steps
    /// instead of forcing an adapter.
    fn adjust_selected_size(&mut self, id: &str, direction: i8, snapshot: bool) -> String {
        let Some(part) = self.vehicle.part(id) else {
            return String::new();
        };
        if self.edit_locked(id) {
            return format!("{id} is locked; unlock it first");
        }
        if !omega_rules::assembly::supports_size_scale(&part.spec) {
            return "This part has no such setting".to_string();
        }
        let base_diameter = omega_rules::assembly::dimensions(&part.spec).1 * 2.0;
        let target = (part.diameter_m() + Param::Size.step() * f64::from(direction)).clamp(0.05, 1.2);
        let scale = (target / base_diameter).clamp(
            omega_rules::assembly::MIN_SIZE_SCALE,
            omega_rules::assembly::MAX_SIZE_SCALE,
        );
        if let Some(locked) = self
            .vehicle
            .symmetry_members(id)
            .iter()
            .find(|member| self.is_locked(member))
        {
            return format!("{locked} est dans le même groupe de symétrie et verrouillée.");
        }
        if snapshot {
            self.push_undo();
        }
        match self.scale_in_symmetry_group(id, scale) {
            Ok(_) => String::new(),
            Err(error) => error,
        }
    }

    /// One wheel notch on the selected part. Notches of the same gesture share one undo entry.
    pub fn adjust_wheel(&mut self, param: Param, direction: i8) -> String {
        self.adjust_wheel_at(param, direction, Instant::now())
    }

    fn adjust_wheel_at(&mut self, param: Param, direction: i8, now: Instant) -> String {
        let Some(id) = self.selected.clone() else {
            return "Select a part first".to_string();
        };
        if self.vehicle.part(&id).is_none() {
            return String::new();
        }
        let key = (id, param);
        let continuous = self
            .wheel_gesture
            .as_ref()
            .is_some_and(|(last, at)| *last == key && now.saturating_duration_since(*at) < WHEEL_GESTURE);
        let message = self.adjust_selected(param, direction, !continuous);
        if message.is_empty() {
            self.wheel_gesture = Some((key, now));
        } else {
            self.wheel_gesture = None;
        }
        message
    }

    fn push_undo(&mut self) {
        self.undo.push(self.vehicle.clone());
        if self.undo.len() > 50 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Undo the last workshop edit. The stack holds at most 50 vehicle snapshots.
    pub fn undo(&mut self) -> String {
        self.wheel_gesture = None;
        let Some(previous) = self.undo.pop() else {
            return "Nothing to undo".to_string();
        };
        let current = std::mem::replace(&mut self.vehicle, previous);
        self.redo.push(current);
        self.retain_editor_state();
        self.select_only(self.vehicle.topmost_id());
        "Undid the last workshop edit".to_string()
    }

    pub fn redo(&mut self) -> String {
        self.wheel_gesture = None;
        let Some(next) = self.redo.pop() else {
            return "Nothing to redo".to_string();
        };
        let current = std::mem::replace(&mut self.vehicle, next);
        self.undo.push(current);
        self.retain_editor_state();
        self.select_only(self.vehicle.topmost_id());
        "Redid the workshop edit".to_string()
    }

    pub fn cycle_symmetry(&mut self) -> String {
        self.symmetry = if self.symmetry >= 8 { 1 } else { self.symmetry + 1 };
        format!("Symétrie {} x{}", self.symmetry_kind.label(), self.symmetry)
    }

    /// Switch the mode used by the next drop and by `LinkSymmetry`.
    pub fn cycle_symmetry_kind(&mut self) -> String {
        self.symmetry_kind = self.symmetry_kind.cycle();
        format!(
            "Mode de symétrie : {} ({} copie{})",
            self.symmetry_kind.label(),
            if self.symmetry_kind == SymmetryKind::Radial {
                self.symmetry
            } else {
                2
            },
            if self.symmetry_kind == SymmetryKind::Radial && self.symmetry == 1 {
                ""
            } else {
                "s"
            }
        )
    }

    /// Replace a part's spec on every copy of its symmetry set as one undoable edit. Parameter
    /// changes are the common case: resizing one fin should resize its paired fins too.
    fn replace_in_symmetry_group(&mut self, id: &str, spec: PartSpec) -> Result<usize, String> {
        let members = self.vehicle.symmetry_members(id);
        if let Some(locked) = members.iter().find(|member| self.edit_locked(member)) {
            return Err(format!("{locked} est dans le même groupe de symétrie et verrouillée."));
        }
        for member in &members {
            if self.vehicle.part(member).is_some() {
                self.vehicle.replace_part(member, spec);
            }
        }
        Ok(members.len())
    }

    fn scale_in_symmetry_group(&mut self, id: &str, scale: f64) -> Result<usize, String> {
        let members = self.vehicle.symmetry_members(id);
        if let Some(locked) = members.iter().find(|member| self.edit_locked(member)) {
            return Err(format!("{locked} est dans le même groupe de symétrie et verrouillée."));
        }
        for member in &members {
            if let Some(part) = self.vehicle.part_mut(member) {
                part.size_scale = scale;
            }
        }
        Ok(members.len())
    }

    /// Drop the link on every selected part. The geometry is untouched; only later grouped
    /// edits and the symmetry badges stop following the other copies.
    pub fn break_symmetry(&mut self) -> String {
        let ids: Vec<String> = if self.selection.is_empty() {
            self.selected.iter().cloned().collect()
        } else {
            self.selection.iter().cloned().collect()
        };
        if ids.is_empty() {
            return "Sélectionnez au moins une pièce à détacher.".to_string();
        }
        if let Some(locked) = ids.iter().find(|id| self.is_locked(id)) {
            return format!("{locked} est verrouillée : déverrouillez-la d'abord.");
        }
        let linked: Vec<String> = ids
            .iter()
            .filter(|id| self.vehicle.part(id).is_some_and(|part| part.symmetry.is_some()))
            .cloned()
            .collect();
        if linked.is_empty() {
            return "La sélection n'a pas de lien de symétrie.".to_string();
        }
        self.push_undo();
        for id in &linked {
            if let Some(part) = self.vehicle.part_mut(id) {
                part.symmetry = None;
            }
        }
        format!("Lien de symétrie rompu pour {} pièce(s)", linked.len())
    }

    /// Attach the selected parts to a new symmetry set of the active mode. Their transforms and
    /// hierarchy are preserved; only the copy indices are new.
    pub fn link_symmetry(&mut self) -> String {
        let mut ids: Vec<String> = self.selection.iter().cloned().collect();
        if ids.len() < 2 {
            return "Sélectionnez au moins deux pièces (Ctrl+clic) pour les lier.".to_string();
        }
        if let Some(locked) = ids.iter().find(|id| self.is_locked(id)) {
            return format!("{locked} est verrouillée : déverrouillez-la d'abord.");
        }
        self.push_undo();
        let key = format!("sym-{}", self.next_symmetry);
        self.next_symmetry += 1;
        // Copy indices must follow the physical order around the parent, not the alphabetical id:
        // a radial transform is derived from the index difference, so an arbitrary order would
        // rotate the copies by the wrong angle.
        let angle = |id: &str| -> (String, f64) {
            let Some(part) = self.vehicle.part(id) else {
                return (String::new(), 0.0);
            };
            let Some(attach) = &part.attach else {
                return (String::new(), 0.0);
            };
            let mount = match attach.node {
                omega_rules::NodeId::Radial(index) => self
                    .vehicle
                    .part(&attach.parent)
                    .map_or(0.0, |parent| omega_rules::assembly::radial_angle(&parent.spec, index)),
                _ => 0.0,
            };
            (attach.parent.clone(), mount + attach.angle_deg)
        };
        ids.sort_by(|a, b| {
            let (parent_a, angle_a) = angle(a);
            let (parent_b, angle_b) = angle(b);
            parent_a
                .cmp(&parent_b)
                .then_with(|| angle_a.total_cmp(&angle_b))
                .then_with(|| a.cmp(b))
        });
        for (index, id) in ids.iter().enumerate() {
            if let Some(part) = self.vehicle.part_mut(id) {
                part.symmetry = Some(encode_symmetry(&key, self.symmetry_kind, index as u8));
            }
        }
        format!("{} pièces liées en symétrie {}", ids.len(), self.symmetry_kind.label())
    }

    /// A free mount under `parent` for a new part of this spec, radial first so a surface set can
    /// be duplicated around the same body.
    fn free_mount(&self, vehicle: &Vehicle, parent: &str, spec: &PartSpec) -> Option<(omega_rules::NodeId, f64)> {
        if self.is_locked(parent) {
            return None;
        }
        let part = vehicle.part(parent)?;
        for index in 0..radial_slots(&part.spec) {
            let node = omega_rules::NodeId::Radial(index);
            if vehicle.node_status(parent, node, spec).is_ok() {
                return Some((node, 0.0));
            }
        }
        for node in [omega_rules::NodeId::Top, omega_rules::NodeId::Bottom] {
            if vehicle.node_status(parent, node, spec).is_ok() {
                return Some((node, 0.0));
            }
        }
        None
    }

    /// Duplicate every selected subtree, preserving its inner attachments, onto free mounts of the
    /// same parent. The copies become a new independent group (their symmetry link is cleared).
    pub fn duplicate_selected(&mut self) -> String {
        let roots = self.selected_roots();
        if roots.is_empty() {
            return "Sélectionnez un sous-ensemble à dupliquer.".to_string();
        }
        if let Some(locked) = roots.iter().find(|id| self.edit_locked(id)) {
            return format!("{locked} contient une pièce verrouillée : déverrouillez-la d'abord.");
        }
        let mut candidate = self.vehicle.clone();
        let mut new_roots = Vec::new();
        for root in &roots {
            let Some(source) = candidate.part(root).cloned() else {
                continue;
            };
            let Some(source_attach) = source.attach.clone() else {
                return "La racine du véhicule ne peut pas être dupliquée : sélectionnez un sous-ensemble.".to_string();
            };
            let Some((node, angle)) = self.free_mount(&candidate, &source_attach.parent, &source.spec) else {
                return format!("Aucun point libre sur {} pour dupliquer {root}.", source_attach.parent);
            };
            let ids = candidate.subtree_ids(root);
            let mut mapping: BTreeMap<String, String> = BTreeMap::new();
            for old in &ids {
                mapping.insert(old.clone(), self.fresh_id("copy"));
            }
            for old in &ids {
                let Some(original) = candidate.part(old).cloned() else {
                    continue;
                };
                let new_id = mapping[old].clone();
                let mut attach = original.attach.clone();
                if old == root {
                    attach = Some(Attach {
                        parent: source_attach.parent.clone(),
                        node,
                        angle_deg: angle,
                        ..source_attach.clone()
                    });
                } else if let Some(attach) = attach.as_mut()
                    && let Some(parent) = mapping.get(&attach.parent)
                {
                    attach.parent = parent.clone();
                }
                candidate.parts.push(omega_rules::PlacedPart {
                    id: new_id,
                    spec: original.spec,
                    attach,
                    symmetry: None,
                    size_scale: original.size_scale,
                });
            }
            new_roots.push(mapping[root].clone());
        }
        if new_roots.is_empty() {
            return "Rien n'a été dupliqué.".to_string();
        }
        self.push_undo();
        self.vehicle = candidate;
        for (root, new_root) in roots.iter().zip(new_roots.iter()) {
            if let Some(name) = self.group_names.get(root).cloned() {
                self.group_names.insert(new_root.clone(), format!("{name} copie"));
            }
        }
        self.selection = new_roots.iter().cloned().collect();
        self.selected = new_roots.first().cloned();
        format!("{} sous-ensemble(s) dupliqué(s) sur des points libres", new_roots.len())
    }

    /// Capture the selection (or the selected subtree) into the local library.
    pub fn save_subassembly(&mut self, kind: SubassemblyKind) -> String {
        let ids: Vec<String> = if self.selection.is_empty() {
            self.selected
                .as_deref()
                .map(|id| self.vehicle.subtree_ids(id))
                .unwrap_or_default()
        } else {
            let mut ids = Vec::new();
            for id in &self.selection {
                ids.extend(self.vehicle.subtree_ids(id));
            }
            ids
        };
        if ids.is_empty() {
            return "Sélectionnez les pièces du sous-ensemble à enregistrer.".to_string();
        }
        let base = self
            .selected
            .as_deref()
            .and_then(|id| self.group_name(id))
            .map(str::to_string)
            .unwrap_or_else(|| format!("Ensemble {}", self.library.len() + 1));
        let mut name = base.clone();
        let mut suffix = 2;
        while self.library.iter().any(|entry| entry.name == name) {
            name = format!("{base} {suffix}");
            suffix += 1;
        }
        match Subassembly::capture(&self.vehicle, &ids, name.clone(), kind) {
            Ok(entry) => {
                self.library.push(entry);
                self.library_dirty = true;
                format!("« {name} » enregistré dans la bibliothèque ({})", kind.label())
            }
            Err(error) => error,
        }
    }

    /// Insert a saved set. Inner edges are replayed first, then each captured root is attached to
    /// a free mount of the selected part (or of the current stack).
    pub fn apply_subassembly(&mut self, index: usize, owned: &BTreeSet<String>) -> String {
        let Some(entry) = self.library.get(index).cloned() else {
            return "Sous-ensemble introuvable.".to_string();
        };
        if let Err(error) = entry.validate() {
            return format!("Sous-ensemble invalide : {error}");
        }
        if let Some(missing) = entry
            .parts
            .iter()
            .flat_map(|part| std::iter::once(part.spec.required_capability()).chain(part.spec.extra_capabilities()))
            .find(|capability| !owned.contains(*capability))
        {
            return format!(
                "Recherche requise : {}",
                omega_rules::research::capability_label(missing)
            );
        }
        let mut candidate = self.vehicle.clone();
        let mut new_ids = Vec::new();
        for part in &entry.parts {
            let id = self.fresh_id("sub");
            new_ids.push(id.clone());
            candidate.parts.push(omega_rules::PlacedPart {
                id,
                spec: part.spec,
                attach: None,
                symmetry: None,
                size_scale: part.size_scale,
            });
        }
        let first_spec = entry.parts.first().map(|part| part.spec);
        let target = if self.vehicle.parts.is_empty() || entry.parts.first().is_none_or(|part| part.parent.is_some()) {
            None
        } else {
            let selected = self.selected.as_deref();
            let mut target = first_spec.and_then(|spec| {
                let parent = selected?;
                self.free_mount(&candidate, parent, &spec)
                    .map(|(node, angle)| (parent.to_string(), node, angle))
            });
            if target.is_none() {
                target = first_spec.and_then(|spec| {
                    let parent = self.vehicle.topmost_id()?;
                    self.free_mount(&candidate, &parent, &spec)
                        .map(|(node, angle)| (parent, node, angle))
                });
            }
            if target.is_none() {
                return "Aucun point libre pour insérer ce sous-ensemble : sélectionnez un corps.".to_string();
            }
            target
        };
        let mut attached_roots = Vec::new();
        for (position, part) in entry.parts.iter().enumerate() {
            let id = new_ids[position].clone();
            let (parent, node, angle) = match part.parent {
                Some(parent) => {
                    let Some(parent_id) = new_ids.get(parent) else {
                        return "Sous-ensemble incohérent : parent absent.".to_string();
                    };
                    (
                        parent_id.clone(),
                        part.node.unwrap_or(omega_rules::NodeId::Top),
                        part.angle_deg,
                    )
                }
                None if position == 0 => match &target {
                    Some((parent, node, angle)) => (parent.clone(), *node, *angle),
                    None => {
                        attached_roots.push(id);
                        continue;
                    }
                },
                None => {
                    // A second captured root (a pair of fins, for example) needs its own free
                    // mount under the same parent, or above the first inserted part.
                    let mounted = target.as_ref().and_then(|(parent, _, _)| {
                        self.free_mount(&candidate, parent, &part.spec)
                            .map(|(node, angle)| (parent.clone(), node, angle))
                    });
                    let mounted = mounted.or_else(|| {
                        let parent = attached_roots.first()?;
                        self.free_mount(&candidate, parent, &part.spec)
                            .map(|(node, angle)| (parent.clone(), node, angle))
                    });
                    let Some((parent, node, angle)) = mounted else {
                        return "Points libres insuffisants pour tous les sous-ensembles.".to_string();
                    };
                    (parent, node, angle)
                }
            };
            if let Err(error) = candidate.attach(&id, &parent, node, angle) {
                return format!("Insertion refusée : {error}");
            }
            if let Some(attach) = candidate.part_mut(&id).and_then(|placed| placed.attach.as_mut()) {
                attach.offset_x_m = part.offset_x_m;
                attach.offset_y_m = part.offset_y_m;
                attach.offset_z_m = part.offset_z_m;
                attach.rotation_x_deg = part.rotation_x_deg;
                attach.rotation_y_deg = part.rotation_y_deg;
                attach.rotation_z_deg = part.rotation_z_deg;
                attach.separation = part.separation;
            }
            if part.parent.is_none() {
                attached_roots.push(id);
            }
        }
        if attached_roots.is_empty() {
            return "Sous-ensemble sans racine.".to_string();
        }
        self.push_undo();
        self.vehicle = candidate;
        if let Some(root) = attached_roots.first()
            && self.group_names.values().all(|name| name != &entry.name)
        {
            self.group_names.insert(root.clone(), entry.name.clone());
        }
        self.selection = attached_roots.iter().cloned().collect();
        self.selected = attached_roots.first().cloned();
        format!("« {} » inséré ({} pièces)", entry.name, entry.parts.len())
    }

    pub fn delete_subassembly(&mut self, index: usize) -> String {
        if index >= self.library.len() {
            return "Sous-ensemble introuvable.".to_string();
        }
        let entry = self.library.remove(index);
        self.library_dirty = true;
        format!("« {} » retiré de la bibliothèque", entry.name)
    }

    pub fn export_subassembly(&mut self, index: usize) -> String {
        let Some(entry) = self.library.get(index) else {
            return "Sous-ensemble introuvable.".to_string();
        };
        match subassembly::export(entry) {
            Ok(path) => format!("« {} » exporté vers {}", entry.name, path.display()),
            Err(error) => error,
        }
    }

    pub fn import_subassemblies(&mut self) -> String {
        match subassembly::import_all() {
            Ok((entries, problems)) => {
                let mut added = 0;
                for entry in entries {
                    if let Some(existing) = self.library.iter_mut().find(|old| old.name == entry.name) {
                        *existing = entry;
                    } else {
                        self.library.push(entry);
                        added += 1;
                    }
                }
                self.library_dirty = true;
                let mut message = format!("{added} sous-ensemble(s) importé(s)");
                if !problems.is_empty() {
                    message.push_str(&format!(" | {} fichier(s) ignoré(s)", problems.len()));
                }
                message
            }
            Err(error) => error,
        }
    }

    /// Construction problems the dedicated control panel lists. Rules issues are classified so an
    /// occupied mount, a loose part and a category conflict stay distinguishable, then the
    /// editor-only locks and named-subassembly connectivity are added.
    pub fn assembly_report(&self) -> Vec<AssemblyIssue> {
        let mut report: Vec<AssemblyIssue> = self
            .vehicle
            .issues()
            .into_iter()
            .map(|issue| {
                let message = issue.message.to_lowercase();
                let kind = if message.contains("both use") || message.contains("already used") {
                    AssemblyIssueKind::Occupied
                } else if message.contains("not connected")
                    || message.contains("no root")
                    || message.contains("root parts")
                    || message.contains("unknown part")
                    || message.contains("cycle")
                {
                    AssemblyIssueKind::Unconnected
                } else {
                    AssemblyIssueKind::Incompatible
                };
                AssemblyIssue {
                    kind,
                    severity: issue.severity,
                    message: issue.message,
                    part: issue.part,
                }
            })
            .collect();
        for id in &self.locked {
            if self.vehicle.part(id).is_some() {
                report.push(AssemblyIssue {
                    kind: AssemblyIssueKind::Locked,
                    severity: IssueSeverity::Warning,
                    message: format!("{id} est verrouillée : elle et son sous-arbre refusent les éditions"),
                    part: Some(id.clone()),
                });
            }
        }
        for (root, name) in &self.group_names {
            if self.vehicle.part(root).is_none() {
                continue;
            }
            let ids = self.vehicle.subtree_ids(root);
            if self.vehicle.part(root).is_some_and(|part| part.attach.is_none()) && self.vehicle.parts.len() > 1 {
                report.push(AssemblyIssue {
                    kind: AssemblyIssueKind::Subassembly,
                    severity: IssueSeverity::Error,
                    message: format!("sous-ensemble « {name} » déconnecté du reste du véhicule"),
                    part: Some(root.clone()),
                });
            }
            let stages: BTreeSet<usize> = ids.iter().filter_map(|id| self.vehicle.stage_of(id)).collect();
            if stages.len() > 1 {
                report.push(AssemblyIssue {
                    kind: AssemblyIssueKind::Subassembly,
                    severity: IssueSeverity::Warning,
                    message: format!(
                        "sous-ensemble « {name} » réparti sur {} étages : vérifiez ses liaisons",
                        stages.len()
                    ),
                    part: Some(root.clone()),
                });
            }
        }
        report.sort_by(|a, b| {
            let error = |issue: &AssemblyIssue| issue.severity != IssueSeverity::Error;
            error(a)
                .cmp(&error(b))
                .then_with(|| a.kind.cmp(&b.kind))
                .then_with(|| a.message.cmp(&b.message))
        });
        report
    }

    fn cycle_aero(&mut self, choice: AeroChoice, owned: &BTreeSet<String>) -> String {
        let Some(id) = self.selected.clone() else {
            return "Select a part first".to_string();
        };
        let Some(part) = self.vehicle.part(&id) else {
            return "That part is no longer there".to_string();
        };
        if self.is_locked(&id) {
            return format!("{id} is locked; unlock it first");
        }
        if choice == AeroChoice::Angle {
            if !matches!(part.spec, PartSpec::Wing { .. }) {
                return "The angle setting is only available on wings".to_string();
            }
            let current = part.attach.as_ref().map_or(0.0, |attach| attach.rotation_x_deg);
            let next = if current >= 15.0 { -15.0 } else { current + 5.0 };
            let members = self.vehicle.symmetry_members(&id);
            if let Some(locked) = members.iter().find(|member| self.edit_locked(member)) {
                return format!("{locked} est dans le même groupe de symétrie et verrouillée.");
            }
            let delta = next - current;
            self.push_undo();
            for member in &members {
                if let Some(attach) = self.vehicle.part_mut(member).and_then(|part| part.attach.as_mut()) {
                    attach.rotation_x_deg = (attach.rotation_x_deg + delta).rem_euclid(360.0);
                }
            }
            return format!("{id}: ANGLE {next:+.0}° (copies liées)");
        }
        match cycled(part.spec, choice) {
            Some(spec) => {
                if let Some(missing) = spec
                    .extra_capabilities()
                    .into_iter()
                    .find(|capability| !owned.contains(*capability))
                {
                    return format!(
                        "Locked: research {} first",
                        omega_rules::research::capability_label(missing)
                    );
                }
                if let Some(locked) = self
                    .vehicle
                    .symmetry_members(&id)
                    .iter()
                    .find(|member| self.is_locked(member))
                {
                    return format!("{locked} est dans le même groupe de symétrie et verrouillée.");
                }
                self.push_undo();
                if let Err(error) = self.replace_in_symmetry_group(&id, spec) {
                    self.undo.pop();
                    return error;
                }
                format!(
                    "{id}: {} {} (copies liées)",
                    choice.label(),
                    choice_value(&spec, choice)
                )
            }
            None => "This part has no such setting".to_string(),
        }
    }

    /// A free radial mount for a surface: the selection first, then the rest of the assembly from
    /// the top down. Surfaces are leaves, so a body with a free radial slot is required.
    fn aero_mount(&self, spec: &PartSpec) -> Option<(String, omega_rules::NodeId, f64)> {
        let candidates = std::iter::once(self.selected.clone())
            .flatten()
            .chain(self.vehicle.poses().into_iter().rev().map(|pose| pose.id));
        for id in candidates {
            let Some(part) = self.vehicle.part(&id) else {
                continue;
            };
            for index in 0..omega_rules::assembly::radial_slots(&part.spec) {
                let node = omega_rules::NodeId::Radial(index);
                if self.vehicle.node_status(&id, node, spec).is_ok() {
                    return Some((id, node, f64::from(index) * 90.0));
                }
            }
        }
        None
    }

    /// Remove every selected subtree as one undoable edit. Locked roots are refused as a group so
    /// a multi-delete can never silently skip one member.
    pub fn delete_selected(&mut self) -> String {
        self.wheel_gesture = None;
        let roots = self.selected_roots();
        if roots.is_empty() {
            return "Select a part to delete".to_string();
        }
        if let Some(locked) = roots.iter().find(|id| self.edit_locked(id)) {
            return format!("{locked} contains a locked part; unlock it first");
        }
        // A linked set is removed as one unit: deleting a single fin should not leave its paired
        // fins behind as an accidental asymmetric design.
        let mut targets: BTreeSet<String> = roots.iter().cloned().collect();
        for root in &roots {
            for member in self.vehicle.symmetry_members(root) {
                if self.vehicle.part(&member).is_some() {
                    targets.insert(member);
                }
            }
        }
        if let Some(locked) = targets.iter().find(|id| self.edit_locked(id)) {
            return format!("{locked} contient une pièce verrouillée : déverrouillez-la d'abord.");
        }
        self.push_undo();
        let mut removed = 0;
        for id in &targets {
            removed += self.vehicle.remove_subtree(id).len();
        }
        self.retain_editor_state();
        self.select_only(self.vehicle.topmost_id());
        let label = if roots.len() == 1 {
            format!("Removed {}", roots[0])
        } else {
            format!("Removed {} selected group(s)", roots.len())
        };
        let label = if targets.len() > roots.len() {
            format!("{label} and {} symmetry copy(ies)", targets.len() - roots.len())
        } else {
            label
        };
        if removed > roots.len() {
            format!("{label} and {} attached part(s)", removed - roots.len())
        } else {
            label
        }
    }

    /// Add a template on a node (or above the stack when `target` is `None`). A radial drop with
    /// symmetry creates linked copies on consecutive free radial slots.
    pub fn add_at(
        &mut self,
        template: PartTemplate,
        target: Option<(String, omega_rules::NodeId, f64)>,
        owned: &BTreeSet<String>,
    ) -> String {
        self.wheel_gesture = None;
        let spec = template.spec();
        if !owned.contains(spec.required_capability()) {
            return format!("Locked: research {} first", spec.required_capability());
        }
        if template.is_computer()
            && let Some(existing) = self
                .vehicle
                .parts
                .iter()
                .find(|p| matches!(p.spec, PartSpec::Computer { .. }))
                .map(|p| p.id.clone())
        {
            if self.is_locked(&existing) {
                return format!("{existing} is locked; unlock it first");
            }
            self.push_undo();
            self.vehicle.replace_part(&existing, spec);
            self.select_only(Some(existing.clone()));
            return format!("Flight computer {existing} set to {}", template.label());
        }
        // A surface is a leaf on a radial mount: an explicit radial drop is used as is, a palette
        // click finds the first free radial slot. Everything else mounts above the stack.
        let target = if spec.is_aero() {
            match target {
                Some((parent, node, angle)) if node.is_radial() && self.vehicle.part(&parent).is_some() => {
                    Some((parent, node, angle))
                }
                _ => self.aero_mount(&spec),
            }
        } else {
            match target {
                None if !self.vehicle.is_empty() => {
                    self.vehicle.topmost_id().map(|id| (id, omega_rules::NodeId::Top, 0.0))
                }
                other => other,
            }
        };
        if spec.is_aero() && target.is_none() {
            return "No free radial mount: select a body part with a free side".to_string();
        }
        let mut placements: Vec<omega_rules::assembly::SymmetryPlacement> = Vec::new();
        if let Some((parent, node, angle)) = &target {
            if self.vehicle.part(parent).is_none() {
                return "That part is no longer there".to_string();
            }
            if node.is_radial() {
                let first = match node {
                    omega_rules::NodeId::Radial(index) => *index,
                    _ => 0,
                };
                let count = match self.symmetry_kind {
                    SymmetryKind::Radial => self.symmetry,
                    _ => 2,
                };
                placements = self
                    .vehicle
                    .symmetry_placements(parent, self.symmetry_kind, first, count);
            } else {
                placements.push(omega_rules::assembly::SymmetryPlacement {
                    node: *node,
                    angle_deg: *angle,
                    index: 0,
                });
            }
        }
        if let Some((parent, _, _)) = &target {
            if self.is_locked(parent) {
                return format!("{parent} is locked; unlock it first");
            }
            for placement in &placements {
                if let Err(error) = self.vehicle.node_status(parent, placement.node, &spec) {
                    return format!("Cannot attach there: {error}");
                }
            }
        }
        self.push_undo();
        let group = (placements.len() > 1).then(|| {
            let key = format!("sym-{}", self.next_symmetry);
            self.next_symmetry += 1;
            key
        });
        let mut created = Vec::new();
        let placements: Vec<Option<omega_rules::assembly::SymmetryPlacement>> = if placements.is_empty() {
            vec![None]
        } else {
            placements.into_iter().map(Some).collect()
        };
        for placement in placements {
            let id = self.fresh_id(template.prefix());
            let symmetry = group.as_ref().map(|key| {
                encode_symmetry(
                    key,
                    self.symmetry_kind,
                    placement.map_or(0, |placement| placement.index),
                )
            });
            self.vehicle.parts.push(omega_rules::PlacedPart {
                id: id.clone(),
                spec,
                attach: None,
                symmetry,
                size_scale: 1.0,
            });
            if let (Some((parent, _, _)), Some(placement)) = (&target, placement)
                && let Err(error) = self.vehicle.attach(&id, parent, placement.node, placement.angle_deg)
            {
                for created_id in created.iter().chain(std::iter::once(&id)) {
                    self.vehicle.remove_subtree(created_id);
                }
                self.undo.pop();
                return format!("Cannot attach there: {error}");
            }
            created.push(id);
        }
        self.note_recent(template);
        self.select_only(created.last().cloned());
        let stage = self
            .vehicle
            .stage_of(created.last().map(String::as_str).unwrap_or(""))
            .unwrap_or(0);
        let count = if created.len() > 1 {
            format!("{} copies {}", created.len(), self.symmetry_kind.label())
        } else {
            created.last().cloned().unwrap_or_default()
        };
        if target.is_none() {
            format!("Added {count} as the root part")
        } else {
            format!("Added {count} to stage {}", stage + 1)
        }
    }

    /// Rotate a mounted aerodynamic surface around its attachment axis.
    /// The angle is a presentation/assembly choice and remains an abstract gameplay parameter.
    pub fn rotate_selected(&mut self, direction: i8) -> String {
        self.wheel_gesture = None;
        let Some(id) = self.selected.clone() else {
            return "Select an aerodynamic surface first".to_string();
        };
        let Some(part) = self.vehicle.part(&id) else {
            return "That part is no longer there".to_string();
        };
        if self.is_locked(&id) {
            return format!("{id} is locked; unlock it first");
        }
        if !part.spec.is_aero() {
            return "Only aerodynamic surfaces can be rotated here".to_string();
        }
        if part.attach.is_none() {
            return "This surface has no attachment axis".to_string();
        }
        let members = self.vehicle.symmetry_members(&id);
        if let Some(locked) = members.iter().find(|member| self.is_locked(member)) {
            return format!("{locked} est dans le même groupe de symétrie et verrouillée.");
        }
        self.push_undo();
        for member in &members {
            if let Some(part) = self.vehicle.part_mut(member)
                && let Some(attach) = part.attach.as_mut()
            {
                // Every copy keeps its own mount angle; the same twist delta is applied to all.
                attach.angle_deg = (attach.angle_deg + f64::from(direction) * 15.0).rem_euclid(360.0);
            }
        }
        let angle = self
            .vehicle
            .part(&id)
            .and_then(|part| part.attach.as_ref())
            .map_or(0.0, |attach| attach.angle_deg);
        format!("{id}: rotation {angle:.0}° (copies liées)")
    }

    /// Rotation and mirror a symmetry copy receives relative to the edited copy. Radial sets
    /// rotate around the parent's long axis, axial sets turn by 180°, bilateral sets mirror the
    /// X translation and the X/Z rotations.
    fn symmetry_variant(vehicle: &Vehicle, source: &str, member: &str) -> (f64, bool, bool) {
        if source == member {
            return (0.0, false, false);
        }
        let (Some(source_group), Some(member_group)) = (
            vehicle.part(source).and_then(|part| part.symmetry.as_deref()),
            vehicle.part(member).and_then(|part| part.symmetry.as_deref()),
        ) else {
            return (0.0, false, false);
        };
        let (source_key, source_kind, source_index) = decode_symmetry(source_group);
        let (member_key, member_kind, member_index) = decode_symmetry(member_group);
        if source_key != member_key || source_kind != member_kind {
            return (0.0, false, false);
        }
        let count = vehicle
            .parts
            .iter()
            .filter(|part| {
                part.symmetry.as_deref().is_some_and(|group| {
                    let (key, kind, _) = decode_symmetry(group);
                    key == source_key && kind == source_kind
                })
            })
            .count()
            .max(1) as f64;
        match source_kind {
            SymmetryKind::Radial => (
                (f64::from(member_index) - f64::from(source_index)) * 360.0 / count,
                false,
                false,
            ),
            SymmetryKind::Axial => (
                if member_index % 2 != source_index % 2 {
                    180.0
                } else {
                    0.0
                },
                false,
                false,
            ),
            SymmetryKind::Bilateral => (0.0, member_index % 2 != source_index % 2, false),
        }
    }

    /// Apply one local delta to a part, transformed by its symmetry variant. Returns false when
    /// the part cannot be transformed (root or missing attachment).
    fn apply_attach_delta(
        vehicle: &mut Vehicle,
        id: &str,
        axis: usize,
        amount: f64,
        rotate: bool,
        variant: (f64, bool, bool),
    ) -> bool {
        let Some(attach) = vehicle.part_mut(id).and_then(|part| part.attach.as_mut()) else {
            return false;
        };
        if rotate {
            let mut degrees = amount;
            if axis == 1 {
                degrees += variant.0;
            } else if variant.1 || (axis == 0 && variant.2) {
                degrees = -degrees;
            }
            let value = match axis {
                0 => &mut attach.rotation_x_deg,
                1 => &mut attach.rotation_y_deg,
                _ => &mut attach.rotation_z_deg,
            };
            *value = (*value + degrees).rem_euclid(360.0);
        } else {
            let (mut dx, dy, mut dz) = match axis {
                0 => (amount, 0.0, 0.0),
                1 => (0.0, amount, 0.0),
                _ => (0.0, 0.0, amount),
            };
            if variant.0 != 0.0 {
                let radians = variant.0.to_radians();
                let (sin, cos) = radians.sin_cos();
                let rotated_x = dx * cos + dz * sin;
                let rotated_z = -dx * sin + dz * cos;
                dx = rotated_x;
                dz = rotated_z;
            }
            if variant.1 {
                dx = -dx;
            }
            if variant.2 {
                dz = -dz;
            }
            attach.offset_x_m += dx;
            attach.offset_y_m += dy;
            attach.offset_z_m += dz;
        }
        true
    }

    /// Preview a KSP gizmo transform from the drag's original design, without adding undo entries.
    /// Every selected subtree root receives the same local-axis delta, and linked symmetry copies
    /// receive the mirrored/rotated equivalent, so a multi-selection moves as one gesture while
    /// each hierarchy stays attached to its own parent.
    pub fn preview_transform(&mut self, before: &Vehicle, axis: usize, amount: f64, rotate: bool) -> String {
        self.wheel_gesture = None;
        self.vehicle = before.clone();
        let roots = self.selected_roots();
        if roots.is_empty() {
            return "Select a part first".to_string();
        }
        if axis > 2 {
            return "Unknown transform axis".to_string();
        }
        let mut changed = 0;
        let mut applied: BTreeSet<String> = BTreeSet::new();
        for id in roots {
            if applied.contains(&id) {
                continue;
            }
            let Some(part) = self.vehicle.part(&id) else {
                continue;
            };
            if part.attach.is_none() || self.edit_locked(&id) {
                continue;
            }
            if Self::apply_attach_delta(&mut self.vehicle, &id, axis, amount, rotate, (0.0, false, false)) {
                applied.insert(id.clone());
                changed += 1;
            }
            for member in self.vehicle.symmetry_members(&id) {
                if member == id || applied.contains(&member) || self.edit_locked(&member) {
                    continue;
                }
                let variant = Self::symmetry_variant(&self.vehicle, &id, &member);
                if Self::apply_attach_delta(&mut self.vehicle, &member, axis, amount, rotate, variant) {
                    applied.insert(member);
                    changed += 1;
                }
            }
        }
        if changed == 0 {
            return "The selection is locked or cannot be transformed".to_string();
        }
        let action = if rotate { "rotation" } else { "translation" };
        format!("{changed} part(s): {action} {} {amount:+.2}", ["X", "Y", "Z"][axis])
    }

    /// Commit a gizmo preview as one undoable edit when the mouse is released.
    pub fn commit_transform(&mut self, before: Vehicle) {
        if self.vehicle != before {
            self.undo.push(before);
            if self.undo.len() > 50 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
    }

    /// Apply a discrete transform as one undoable action.
    #[cfg(test)]
    pub fn transform_selected(&mut self, axis: usize, amount: f64, rotate: bool) -> String {
        let before = self.vehicle.clone();
        let message = self.preview_transform(&before, axis, amount, rotate);
        self.commit_transform(before);
        message
    }

    /// The six editable local transform components of a mounted part, in the order
    /// X/Y/Z translation then X/Y/Z rotation.
    pub fn transform_values(&self, id: &str) -> Option<[f64; 6]> {
        let attach = self.vehicle.part(id)?.attach.as_ref()?;
        Some([
            attach.offset_x_m,
            attach.offset_y_m,
            attach.offset_z_m,
            attach.rotation_x_deg,
            attach.rotation_y_deg,
            attach.rotation_z_deg,
        ])
    }

    /// Replace all six local transform components. One call is one undo entry, which lets the
    /// inspector's reset buttons stay a single user action. Values are checked before the design
    /// is touched, so a bad entry cannot leave a half-updated part behind.
    pub fn set_transform_values(&mut self, id: &str, values: [f64; 6]) -> Result<(), String> {
        for (index, value) in values.iter().enumerate() {
            validate_transform_value(*value, index >= 3)?;
        }
        let Some(part) = self.vehicle.part(id) else {
            return Err("Cette pièce n'existe plus.".to_string());
        };
        if part.attach.is_none() {
            return Err("La pièce racine ne peut pas être transformée.".to_string());
        }
        if self.edit_locked(id) {
            return Err(format!("{id} est verrouillée : déverrouillez-la d'abord."));
        }
        let members = self.vehicle.symmetry_members(id);
        if let Some(locked) = members.iter().find(|member| self.edit_locked(member)) {
            return Err(format!("{locked} est dans le même groupe de symétrie et verrouillée."));
        }
        let current = self
            .transform_values(id)
            .ok_or_else(|| "Cette pièce n'existe plus.".to_string())?;
        let delta = [
            values[0] - current[0],
            values[1] - current[1],
            values[2] - current[2],
            values[3] - current[3],
            values[4] - current[4],
            values[5] - current[5],
        ];
        let before = self.vehicle.clone();
        let Some(attach) = self.vehicle.part_mut(id).and_then(|part| part.attach.as_mut()) else {
            return Err("Cette pièce n'existe plus.".to_string());
        };
        attach.offset_x_m = values[0];
        attach.offset_y_m = values[1];
        attach.offset_z_m = values[2];
        attach.rotation_x_deg = values[3];
        attach.rotation_y_deg = values[4];
        attach.rotation_z_deg = values[5];
        for member in members.iter().filter(|member| member.as_str() != id) {
            let variant = Self::symmetry_variant(&self.vehicle, id, member);
            for axis in 0..3 {
                Self::apply_attach_delta(&mut self.vehicle, member, axis, delta[axis], false, variant);
                Self::apply_attach_delta(&mut self.vehicle, member, axis, delta[3 + axis], true, variant);
            }
        }
        if self.vehicle != before {
            self.commit_transform(before);
        }
        Ok(())
    }

    /// Set one local transform component as one undoable action.
    pub fn set_transform_value(&mut self, id: &str, axis: usize, rotate: bool, value: f64) -> Result<(), String> {
        if axis > 2 {
            return Err("Axe de transformation inconnu.".to_string());
        }
        let mut values = self
            .transform_values(id)
            .ok_or_else(|| "Cette pièce n'existe plus.".to_string())?;
        values[axis + if rotate { 3 } else { 0 }] = value;
        self.set_transform_values(id, values)
    }

    /// Move an existing part (with its subtree) to another node.
    pub fn move_part(&mut self, id: &str, parent: &str, node: omega_rules::NodeId, angle: f64) -> String {
        self.wheel_gesture = None;
        let Some(part) = self.vehicle.part(id) else {
            return "That part is no longer there".to_string();
        };
        if part.attach.is_none() {
            return "The root part stays at the bottom".to_string();
        }
        if self.edit_locked(id) {
            return format!("{id} contains a locked part; unlock it first");
        }
        if self.is_locked(parent) {
            return format!("{parent} is locked; unlock it first");
        }
        let was_linked = part.symmetry.is_some();
        let before = self.vehicle.clone();
        match self.vehicle.attach(id, parent, node, angle) {
            Ok(()) => {
                // A reattached copy leaves its set: the other copies would otherwise keep a
                // spatial relationship the designer just broke by dragging this one away.
                if was_linked && let Some(part) = self.vehicle.part_mut(id) {
                    part.symmetry = None;
                }
                self.undo.push(before);
                self.redo.clear();
                self.select_only(Some(id.to_string()));
                if was_linked {
                    format!("Moved {id} to {} {}; symmetry link broken", parent, node.label())
                } else {
                    format!("Moved {id} to {} {}", parent, node.label())
                }
            }
            Err(error) => format!("Cannot move {id}: {error}"),
        }
    }

    fn add_part(&mut self, template: PartTemplate, owned: &BTreeSet<String>) -> String {
        let spec = template.spec();
        let capability = spec.required_capability();
        if !owned.contains(capability) {
            return format!("Locked: research {capability} first");
        }
        if template.is_computer()
            && let Some(existing) = self
                .vehicle
                .parts
                .iter()
                .find(|p| matches!(p.spec, PartSpec::Computer { .. }))
                .map(|p| p.id.clone())
        {
            self.vehicle.replace_part(&existing, spec);
            self.select_only(Some(existing.clone()));
            return format!("Flight computer {existing} set to {}", template.label());
        }
        let parent = [self.selected.clone(), self.vehicle.topmost_id()]
            .into_iter()
            .flatten()
            .find(|id| self.vehicle.node_status(id, omega_rules::NodeId::Top, &spec).is_ok());
        if parent.is_none() && !self.vehicle.is_empty() {
            return format!(
                "Cannot add {}: the nose cone closes the stack; remove or select another mount first",
                template.label()
            );
        }
        if let Some(parent) = &parent
            && self.is_locked(parent)
        {
            return format!("{parent} is locked; unlock it first");
        }
        let id = self.fresh_id(template.prefix());
        self.vehicle.parts.push(omega_rules::PlacedPart {
            id: id.clone(),
            spec,
            attach: None,
            symmetry: None,
            size_scale: 1.0,
        });
        if let Some(parent) = parent
            && let Err(error) = self.vehicle.attach(&id, &parent, omega_rules::NodeId::Top, 0.0)
        {
            self.vehicle.remove_subtree(&id);
            return format!("Cannot attach {id}: {error}");
        }
        self.note_recent(template);
        self.select_only(Some(id.clone()));
        match self.vehicle.stage_of(&id) {
            Some(stage) => format!("Added {id} to stage {}", stage + 1),
            None => format!("Added {id}"),
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(&self.vehicle).map_err(|e| e.to_string())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let vehicle: Vehicle = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        for part in &vehicle.parts {
            part.spec.validate().map_err(|e| format!("{}: {e}", part.id))?;
        }
        let mut session = Self {
            selected: vehicle.topmost_id(),
            vehicle,
            ..Self::default()
        };
        session.select_only(session.selected.clone());
        session.sync_next_id();
        Ok(session)
    }
}

// ----------------------------------------------------------------------------------------------
// Layout and 3D visuals
// ----------------------------------------------------------------------------------------------

/// Human summary of what changed between two missile designs, by part id.
pub(crate) fn vehicle_diff(before: &Vehicle, after: &Vehicle) -> String {
    use std::collections::BTreeMap;
    let index = |vehicle: &Vehicle| -> BTreeMap<String, (usize, PartSpec)> {
        vehicle
            .parts
            .iter()
            .map(|part| (part.id.clone(), (vehicle.stage_of(&part.id).unwrap_or(0), part.spec)))
            .collect()
    };
    let old = index(before);
    let new = index(after);
    let mut added = 0;
    let mut removed = 0;
    let mut changed = 0;
    let mut moved = 0;
    for (id, (stage, spec)) in &new {
        match old.get(id) {
            None => added += 1,
            Some((old_stage, old_spec)) => {
                if old_spec != spec {
                    changed += 1;
                } else if old_stage != stage {
                    moved += 1;
                }
            }
        }
    }
    removed += old.keys().filter(|id| !new.contains_key(*id)).count();
    let mass = |vehicle: &Vehicle| -> f64 {
        vehicle
            .parts
            .iter()
            .map(|part| {
                let stats = part.spec.stats();
                stats.dry_mass_kg + stats.propellant_kg
            })
            .sum()
    };
    let delta = mass(after) - mass(before);
    if added + removed + changed + moved == 0 && delta.abs() < 0.05 {
        return "no changes".to_string();
    }
    let mut parts = Vec::new();
    for (count, label) in [
        (added, "added"),
        (removed, "removed"),
        (changed, "changed"),
        (moved, "moved"),
    ] {
        if count > 0 {
            parts.push(format!("{count} {label}"));
        }
    }
    parts.push(format!("mass {delta:+.1} kg"));
    parts.join(", ")
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Shape {
    Body,
    Motor,
    Nozzle,
    Turbofan,
    Sensor,
    Disc,
    /// Flat plate: span outward from the mount, chord along the body.
    Surface,
    /// Axial body transition; the radii come from the part box.
    Adapter(AdapterShape),
    /// Player-placed nose cone with its chosen profile.
    Nose(NoseShape),
}

/// Surface finish of a part. The workshop uses one coherent military palette instead of a colour
/// per family, with a little metalness on engines and a faint emissive accent on electronics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Finish {
    pub base: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
}

const fn paint(base: [f32; 3], metallic: f32, roughness: f32) -> Finish {
    Finish {
        base,
        metallic,
        roughness,
        emissive: [0.0; 3],
    }
}

const fn lit(base: [f32; 3], metallic: f32, roughness: f32, emissive: [f32; 3]) -> Finish {
    Finish {
        base,
        metallic,
        roughness,
        emissive,
    }
}

// Muted airframe colours: olive drab, desert sand, gunmetal and graphite read as one family.
const OLIVE_DRAB: [f32; 3] = [0.32, 0.35, 0.24];
const DESERT_SAND: [f32; 3] = [0.50, 0.45, 0.33];
const GUNMETAL: [f32; 3] = [0.22, 0.25, 0.28];
const TITANIUM: [f32; 3] = [0.40, 0.43, 0.46];
const GRAPHITE: [f32; 3] = [0.12, 0.14, 0.16];
const NAVY_GREY: [f32; 3] = [0.24, 0.31, 0.39];
const SLATE_BLUE: [f32; 3] = [0.30, 0.34, 0.38];
const HAZARD_AMBER: [f32; 3] = [0.70, 0.47, 0.13];
const RADAR_GREEN: [f32; 3] = [0.27, 0.50, 0.34];
const SENSOR_CYAN: [f32; 3] = [0.26, 0.52, 0.59];
const AVIONICS_VIOLET: [f32; 3] = [0.38, 0.31, 0.55];
const POWER_AMBER: [f32; 3] = [0.62, 0.45, 0.17];
const DIAG_GREEN: [f32; 3] = [0.28, 0.54, 0.31];
const CONTROL_AMBER: [f32; 3] = [0.66, 0.48, 0.17];

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PartBox {
    pub id: String,
    pub x: f32,
    pub z: f32,
    pub y_bottom: f32,
    pub height: f32,
    pub radius: f32,
    /// Actual radii of the two axial faces; equal except on an adapter.
    pub bottom_radius: f32,
    pub top_radius: f32,
    /// Full span of a surface plate, 0 for bodies.
    pub span: f32,
    /// Clockwise mount angle in degrees, used to orient the plate.
    pub angle: f32,
    pub rotation_x_deg: f32,
    pub rotation_y_deg: f32,
    pub rotation_z_deg: f32,
    /// Wing-only planform and sweep, used by the 3D preview mesh.
    pub planform: Option<WingPlanform>,
    pub sweep_deg: f32,
    pub shape: Shape,
    pub finish: Finish,
}

fn appearance(spec: &PartSpec) -> (Shape, Finish) {
    match *spec {
        PartSpec::SolidMotor { .. } => (Shape::Motor, paint(OLIVE_DRAB, 0.15, 0.72)),
        PartSpec::FuelTank { .. } => (Shape::Body, paint(DESERT_SAND, 0.20, 0.66)),
        PartSpec::LiquidEngine { .. } => (Shape::Nozzle, paint(GUNMETAL, 0.65, 0.34)),
        PartSpec::AirBreathing { .. } => (Shape::Turbofan, paint(TITANIUM, 0.55, 0.38)),
        PartSpec::Sensor {
            kind: SensorKind::Pitot,
        } => (Shape::Sensor, lit(SENSOR_CYAN, 0.30, 0.45, [0.03, 0.16, 0.20])),
        PartSpec::Sensor {
            kind: SensorKind::Barometer,
        } => (Shape::Disc, paint(SENSOR_CYAN, 0.20, 0.55)),
        PartSpec::Sensor {
            kind: SensorKind::Radar(RadarClass::Basic),
        } => (Shape::Disc, lit(RADAR_GREEN, 0.15, 0.50, [0.03, 0.12, 0.05])),
        PartSpec::Sensor {
            kind: SensorKind::Radar(RadarClass::Active),
        } => (Shape::Disc, lit(RADAR_GREEN, 0.15, 0.50, [0.05, 0.20, 0.08])),
        PartSpec::Computer { .. } => (Shape::Disc, lit(GRAPHITE, 0.40, 0.42, [0.02, 0.12, 0.14])),
        PartSpec::Decoupler => (Shape::Disc, paint(HAZARD_AMBER, 0.20, 0.55)),
        PartSpec::MissionModule { .. } => (Shape::Body, paint(NAVY_GREY, 0.30, 0.60)),
        PartSpec::Explosive { grade, .. } => {
            let tint = match grade {
                ExplosiveGrade::Standard => [0.42, 0.29, 0.20],
                ExplosiveGrade::Improved => [0.52, 0.25, 0.15],
                ExplosiveGrade::Advanced => [0.60, 0.19, 0.12],
            };
            (Shape::Body, paint(tint, 0.24, 0.50))
        }
        PartSpec::Adapter { shape, .. } => (Shape::Adapter(shape), paint(TITANIUM, 0.35, 0.52)),
        PartSpec::NoseCone { shape, .. } => (Shape::Nose(shape), paint(DESERT_SAND, 0.22, 0.58)),
        PartSpec::Wing { .. } => (Shape::Surface, paint(SLATE_BLUE, 0.25, 0.58)),
        PartSpec::FinSet { .. } => (Shape::Surface, paint(OLIVE_DRAB, 0.20, 0.62)),
        // Fixed surfaces are grey, moving ones amber, so the two read differently in 3D.
        PartSpec::ControlSurface {
            kind: ControlKind::Fixed,
            ..
        } => (Shape::Surface, paint(GUNMETAL, 0.35, 0.50)),
        PartSpec::ControlSurface { .. } => (Shape::Surface, paint(CONTROL_AMBER, 0.30, 0.50)),
        // Mission electronics read as small discs, one colour per category group.
        PartSpec::Avionics {
            module: AvionicsModule::ControlAugment,
        } => (Shape::Disc, paint(AVIONICS_VIOLET, 0.30, 0.45)),
        PartSpec::Avionics {
            module: AvionicsModule::InertialNav,
        } => (Shape::Disc, paint(SENSOR_CYAN, 0.30, 0.45)),
        PartSpec::Avionics {
            module: AvionicsModule::Datalink,
        } => (Shape::Disc, paint([0.29, 0.40, 0.53], 0.35, 0.45)),
        PartSpec::Avionics {
            module: AvionicsModule::PowerCell,
        } => (Shape::Disc, lit(POWER_AMBER, 0.25, 0.50, [0.14, 0.07, 0.01])),
        PartSpec::Avionics {
            module: AvionicsModule::Diagnostics,
        } => (Shape::Disc, paint(DIAG_GREEN, 0.20, 0.50)),
    }
}

/// Air-breathing vehicles are the cruise family: the workshop lays them out like an aircraft
/// instead of standing the stack on its tail.
pub(crate) fn horizontal(vehicle: &Vehicle) -> bool {
    vehicle
        .parts
        .iter()
        .any(|part| matches!(part.spec, PartSpec::AirBreathing { .. }))
}

/// Presentation rotation applied to the whole vehicle. The quarter-turn family keeps the nose on
/// +X and the radial mounts in the horizontal plane, so wings read as wings.
pub(crate) fn orientation(vehicle: &Vehicle) -> Quat {
    if horizontal(vehicle) {
        Quat::from_axis_angle(Vec3::ONE.normalize(), -std::f32::consts::TAU / 3.0)
    } else {
        Quat::IDENTITY
    }
}

/// World position of the vehicle's local origin. Air-breathing vehicles are raised so their hull
/// clears the assembly pad.
pub(crate) fn base(vehicle: &Vehicle) -> Vec3 {
    ANCHOR
        + if horizontal(vehicle) {
            Vec3::Y * HORIZONTAL_LIFT
        } else {
            Vec3::ZERO
        }
}

/// Vehicle-local metres to a workshop world position. Picking, node markers, gizmos and the
/// camera all use this single conversion so the horizontal layout edits like the vertical one.
pub(crate) fn world_point(vehicle: &Vehicle, x: f64, y: f64, z: f64) -> Vec3 {
    base(vehicle) + orientation(vehicle) * (Vec3::new(x as f32, y as f32, z as f32) * VISUAL_SCALE)
}

/// The information panels cover the lower part of the workshop. For a horizontal vehicle the
/// camera aims below it so the hull sits in the open strip above the panels; narrow windows have
/// less open space, so the offset grows.
pub(crate) fn framing_bias(vehicle: &Vehicle, viewport_height: f32) -> f32 {
    if !horizontal(vehicle) {
        return 0.0;
    }
    let narrow = (1.0 - (viewport_height - 768.0) / 192.0).clamp(0.0, 1.0);
    2.4 + 0.8 * narrow
}

pub(crate) fn framing_target(vehicle: &Vehicle, point: Vec3, viewport_height: f32) -> Vec3 {
    point + Vec3::NEG_Y * framing_bias(vehicle, viewport_height)
}

/// Keep an already framed target in the free strip when the viewport height changes. Only the
/// bias delta moves, so a manual pan or orbit is preserved.
pub(crate) fn reframe_on_resize(vehicle: &Vehicle, target: Vec3, old_height: f32, new_height: f32) -> Vec3 {
    target + Vec3::NEG_Y * (framing_bias(vehicle, new_height) - framing_bias(vehicle, old_height))
}

/// Full local rotation of a part, including the vehicle's presentation orientation.
pub(crate) fn part_rotation(vehicle: &Vehicle, rotation_x: f32, rotation_y: f32, rotation_z: f32) -> Quat {
    orientation(vehicle)
        * Quat::from_euler(
            EulerRot::XYZ,
            rotation_x.to_radians(),
            rotation_y.to_radians(),
            rotation_z.to_radians(),
        )
}

/// 3D boxes for the attachment tree: axial stacks keep their old look, radial boosters sit around
/// the parent body at the angle stored on their mount.
pub(crate) fn layout(vehicle: &Vehicle) -> Vec<PartBox> {
    vehicle
        .poses()
        .into_iter()
        .filter_map(|pose| {
            let part = vehicle.part(&pose.id)?;
            let spec = part.spec;
            let (shape, finish) = appearance(&spec);
            let (planform, sweep_deg) = match spec {
                PartSpec::Wing {
                    planform, sweep_deg, ..
                } => (Some(planform), sweep_deg as f32),
                _ => (None, 0.0),
            };
            let angle = part.attach.as_ref().map_or(0.0, |attach| match attach.node {
                omega_rules::NodeId::Radial(index) => {
                    // The mount spacing belongs to the parent body (eight mounts since 0.42),
                    // not to the surface that hangs from it.
                    let mount = vehicle
                        .part(&attach.parent)
                        .map_or(0.0, |parent| omega_rules::assembly::radial_angle(&parent.spec, index));
                    mount + attach.angle_deg
                }
                _ => attach.angle_deg,
            });
            let (bottom_radius, top_radius) = match spec {
                PartSpec::Adapter {
                    diameter_m,
                    exit_diameter_m,
                    ..
                } => ((diameter_m / 2.0) as f32, (exit_diameter_m / 2.0) as f32),
                _ => (pose.radius_m as f32, pose.radius_m as f32),
            };
            Some(PartBox {
                id: pose.id,
                x: pose.x_m as f32,
                z: pose.z_m as f32,
                y_bottom: (pose.y_m - pose.height_m / 2.0) as f32,
                height: pose.height_m as f32,
                radius: pose.radius_m as f32,
                bottom_radius,
                top_radius,
                span: if spec.is_aero() {
                    (pose.radius_m * 2.0) as f32
                } else {
                    0.0
                },
                angle: angle as f32,
                rotation_x_deg: pose.rotation_x_deg as f32,
                rotation_y_deg: pose.rotation_y_deg as f32,
                rotation_z_deg: pose.rotation_z_deg as f32,
                planform,
                sweep_deg,
                shape,
                finish,
            })
        })
        .collect()
}

#[derive(Component)]
pub(crate) struct MissileRoot;

pub(crate) fn setup_missile(mut commands: Commands) {
    commands.spawn((
        Transform::from_translation(ANCHOR).with_scale(Vec3::splat(VISUAL_SCALE)),
        Visibility::Hidden,
        MissileRoot,
    ));
}

/// Validate one numeric transform entry before it reaches a vehicle.
pub(crate) fn validate_transform_value(value: f64, rotate: bool) -> Result<(), String> {
    if !value.is_finite() {
        return Err("La valeur doit être un nombre fini.".to_string());
    }
    let limit = if rotate {
        MAX_TRANSFORM_ROTATION_DEG
    } else {
        MAX_TRANSFORM_OFFSET_M
    };
    if value.abs() > limit {
        return Err(format!("Valeur hors limites : maximum ±{limit:.0}."));
    }
    Ok(())
}

impl MissileSession {
    pub(crate) fn load_active(&mut self) {
        let library = self.load_library();
        match save::read(SaveFile::Vehicle) {
            Ok(Some(bytes)) => match Self::from_bytes(&bytes) {
                Ok(loaded) => *self = loaded,
                Err(error) => {
                    warn!("Could not read the saved vehicle: {error}");
                    *self = Self::new_default();
                }
            },
            Ok(None) => *self = Self::new_default(),
            Err(error) => {
                warn!("Could not read the saved vehicle: {error}");
                *self = Self::new_default();
            }
        }
        self.library = library;
        self.library_dirty = false;
    }

    fn load_library(&self) -> Vec<Subassembly> {
        match save::read(SaveFile::Subassemblies) {
            Ok(Some(bytes)) => match subassembly::parse_library(&bytes) {
                Ok(entries) => entries,
                Err(error) => {
                    warn!("Could not read the subassembly library: {error}");
                    Vec::new()
                }
            },
            Ok(None) => Vec::new(),
            Err(error) => {
                warn!("Could not read the subassembly library: {error}");
                Vec::new()
            }
        }
    }

    pub fn library_bytes(&self) -> Result<Vec<u8>, String> {
        subassembly::library_bytes(&self.library)
    }
}

pub(crate) fn load_missile(mut session: ResMut<MissileSession>) {
    session.load_active();
}

pub(crate) fn persist_missile(session: &MissileSession) -> Result<(), String> {
    session
        .to_bytes()
        .and_then(|bytes| save::write(SaveFile::Vehicle, &bytes))
}

pub(crate) fn persist_library(session: &MissileSession) -> Result<(), String> {
    session
        .library_bytes()
        .and_then(|bytes| save::write(SaveFile::Subassemblies, &bytes))
}

/// Rebuild the 3D stack whenever the vehicle or the selection changes.
pub(crate) fn sync_missile_visuals(
    mut commands: Commands,
    campus: Res<CampusState>,
    session: Res<MissileSession>,
    mut root: Query<(Entity, &mut Visibility, &mut Transform), With<MissileRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut last: Local<String>,
) {
    let Ok((entity, mut visibility, mut root_transform)) = root.single_mut() else {
        return;
    };
    let show = campus.view == CampusView::Missile;
    root_transform.translation = base(&session.vehicle);
    let wanted = if show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *visibility != wanted {
        *visibility = wanted;
    }
    root_transform.rotation = orientation(&session.vehicle);
    if !show {
        return;
    }
    let signature = format!("{:?}|{:?}", session.vehicle, session.selection);
    if *last == signature {
        return;
    }
    *last = signature;
    commands.entity(entity).despawn_related::<Children>();
    let boxes = layout(&session.vehicle);
    let hidden = session.hidden_parts();
    let dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.12, 0.14),
        metallic: 0.55,
        perceptual_roughness: 0.42,
        cull_mode: None,
        ..default()
    });
    let hazard = materials.add(StandardMaterial {
        base_color: Color::srgb(0.68, 0.44, 0.11),
        metallic: 0.20,
        perceptual_roughness: 0.52,
        cull_mode: None,
        ..default()
    });
    let exhaust = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.10, 0.03),
        emissive: LinearRgba::rgb(1.5, 0.42, 0.06),
        perceptual_roughness: 0.8,
        cull_mode: None,
        ..default()
    });
    let ceramic = materials.add(StandardMaterial {
        base_color: Color::srgb(0.62, 0.63, 0.60),
        metallic: 0.05,
        perceptual_roughness: 0.55,
        cull_mode: None,
        ..default()
    });
    commands.entity(entity).with_children(|root| {
        for b in boxes.iter().filter(|b| !hidden.contains(&b.id)) {
            let selected = session.is_selected(&b.id);
            let finish = b.finish;
            let material = materials.add(StandardMaterial {
                base_color: Color::srgb(finish.base[0], finish.base[1], finish.base[2]),
                metallic: finish.metallic,
                perceptual_roughness: finish.roughness,
                emissive: if selected {
                    LinearRgba::rgb(0.20, 0.50, 0.62)
                } else {
                    LinearRgba::rgb(finish.emissive[0], finish.emissive[1], finish.emissive[2])
                },
                cull_mode: None,
                ..default()
            });
            let mid = Vec3::new(b.x, b.y_bottom + b.height / 2.0, b.z);
            let rotation = Quat::from_euler(
                EulerRot::XYZ,
                b.rotation_x_deg.to_radians(),
                (b.rotation_y_deg - b.angle).to_radians(),
                b.rotation_z_deg.to_radians(),
            );
            let local = |offset: Vec3| mid + rotation * offset;
            let front = b.height * 0.5;
            let tail = -front;
            match b.shape {
                Shape::Body | Shape::Motor => {
                    spawn_part_mesh(
                        root,
                        &mut meshes,
                        rounded_prism_mesh(b.radius, b.height, 0.78),
                        material.clone(),
                        mid,
                        rotation,
                    );
                    // Panel joints break the long body into readable sections.
                    let band = (b.height * 0.04).clamp(0.02, 0.06);
                    let joints: &[f32] = if b.shape == Shape::Motor {
                        &[0.94]
                    } else {
                        &[0.06, 0.94]
                    };
                    for fraction in joints {
                        let y = -b.height * 0.5 + b.height * fraction;
                        spawn_part_mesh(
                            root,
                            &mut meshes,
                            rounded_prism_mesh(b.radius * 1.025, band, 0.78),
                            dark.clone(),
                            local(Vec3::new(0.0, y, 0.0)),
                            rotation,
                        );
                    }
                    if b.shape == Shape::Motor {
                        // Solid motors end in a dark nozzle skirt and throat disc.
                        spawn_part_mesh(
                            root,
                            &mut meshes,
                            rounded_prism_mesh(b.radius * 1.02, band * 1.2, 0.78),
                            dark.clone(),
                            local(Vec3::new(0.0, tail + band * 0.6, 0.0)),
                            rotation,
                        );
                        spawn_part_mesh(
                            root,
                            &mut meshes,
                            Cylinder::new((b.radius * 0.58).max(0.02), 0.012),
                            exhaust.clone(),
                            local(Vec3::new(0.0, tail + 0.008, 0.0)),
                            rotation,
                        );
                    }
                }
                Shape::Nozzle => {
                    spawn_part_mesh(
                        root,
                        &mut meshes,
                        lathe_mesh(
                            b.radius,
                            b.height,
                            &[(0.0, 1.0), (0.16, 0.78), (0.55, 0.52), (1.0, 0.46)],
                            20,
                            true,
                        ),
                        material.clone(),
                        mid,
                        rotation,
                    );
                    let glow = Cylinder::new((b.radius * 0.62).max(0.02), (b.height * 0.02).clamp(0.006, 0.02));
                    spawn_part_mesh(
                        root,
                        &mut meshes,
                        glow,
                        exhaust.clone(),
                        local(Vec3::new(0.0, tail + 0.012, 0.0)),
                        rotation,
                    );
                }
                Shape::Turbofan => {
                    spawn_part_mesh(
                        root,
                        &mut meshes,
                        // The intake faces the nose (+Y in vehicle space), the exhaust points aft.
                        lathe_mesh(
                            b.radius,
                            b.height,
                            &[(0.0, 0.68), (0.15, 0.76), (0.55, 0.92), (0.85, 1.0), (1.0, 0.95)],
                            24,
                            true,
                        ),
                        material.clone(),
                        mid,
                        rotation,
                    );
                    let intake = Cylinder::new((b.radius * 0.84).max(0.02), 0.02);
                    spawn_part_mesh(
                        root,
                        &mut meshes,
                        intake,
                        dark.clone(),
                        local(Vec3::new(0.0, front - 0.012, 0.0)),
                        rotation,
                    );
                    let spinner = Cone::new((b.radius * 0.30).max(0.01), (b.height * 0.20).max(0.02));
                    spawn_part_mesh(
                        root,
                        &mut meshes,
                        spinner,
                        dark.clone(),
                        local(Vec3::new(0.0, front - b.height * 0.11, 0.0)),
                        rotation,
                    );
                    spawn_part_mesh(
                        root,
                        &mut meshes,
                        Cylinder::new((b.radius * 0.55).max(0.015), 0.018),
                        dark.clone(),
                        local(Vec3::new(0.0, tail + 0.01, 0.0)),
                        rotation,
                    );
                }
                Shape::Sensor => {
                    let base = (b.height * 0.38).max(0.03);
                    let spike = (b.height * 0.54).max(0.03);
                    spawn_part_mesh(
                        root,
                        &mut meshes,
                        rounded_prism_mesh((b.radius * 0.62).max(0.02), base, 0.82),
                        material.clone(),
                        local(Vec3::new(0.0, tail + base * 0.5, 0.0)),
                        rotation,
                    );
                    spawn_part_mesh(
                        root,
                        &mut meshes,
                        Cylinder::new((b.radius * 0.10).max(0.006), spike),
                        dark.clone(),
                        local(Vec3::new(0.0, tail + base + spike * 0.5, 0.0)),
                        rotation,
                    );
                    let tip = Cone::new((b.radius * 0.13).max(0.008), (b.height * 0.08).max(0.01));
                    spawn_part_mesh(
                        root,
                        &mut meshes,
                        tip,
                        ceramic.clone(),
                        local(Vec3::new(0.0, front - b.height * 0.04, 0.0)),
                        rotation,
                    );
                }
                Shape::Nose(shape) => {
                    spawn_part_mesh(
                        root,
                        &mut meshes,
                        nose_mesh(b.radius, b.height, shape),
                        material.clone(),
                        mid,
                        rotation,
                    );
                }
                Shape::Adapter(shape) => {
                    spawn_part_mesh(
                        root,
                        &mut meshes,
                        adapter_mesh(b.bottom_radius, b.top_radius, b.height, shape),
                        material.clone(),
                        mid,
                        rotation,
                    );
                    // Two dark collars mark the faces that meet the neighbouring bodies.
                    let band = (b.height * 0.035).clamp(0.012, 0.04);
                    for (radius, y) in [(b.bottom_radius, tail + band * 0.5), (b.top_radius, front - band * 0.5)] {
                        spawn_part_mesh(
                            root,
                            &mut meshes,
                            rounded_prism_mesh(radius * 1.02, band, 0.78),
                            dark.clone(),
                            local(Vec3::new(0.0, y, 0.0)),
                            rotation,
                        );
                    }
                }
                Shape::Disc => {
                    spawn_part_mesh(
                        root,
                        &mut meshes,
                        rounded_prism_mesh(b.radius, b.height, 0.62),
                        material.clone(),
                        mid,
                        rotation,
                    );
                    // A status strip on the front face keeps electronics readable at a glance.
                    let strip = Cuboid::new((b.radius * 1.15).max(0.03), (b.height * 0.18).clamp(0.006, 0.02), 0.01);
                    spawn_part_mesh(
                        root,
                        &mut meshes,
                        strip,
                        hazard.clone(),
                        local(Vec3::new(0.0, 0.0, b.radius * 0.62 + 0.004)),
                        rotation,
                    );
                }
                Shape::Surface => {
                    let planform = b.planform.unwrap_or(WingPlanform::Straight);
                    spawn_part_mesh(
                        root,
                        &mut meshes,
                        surface_mesh(b.span, b.height, planform, b.sweep_deg, 0.06),
                        material.clone(),
                        mid,
                        rotation,
                    );
                }
            }
        }
    });
}

fn spawn_part_mesh(
    parent: &mut ChildSpawnerCommands<'_>,
    meshes: &mut Assets<Mesh>,
    mesh: impl Into<Mesh>,
    material: Handle<StandardMaterial>,
    translation: Vec3,
    rotation: Quat,
) {
    parent.spawn((
        Mesh3d(meshes.add(mesh.into())),
        MeshMaterial3d(material),
        Transform::from_translation(translation).with_rotation(rotation),
    ));
}

/// Small indexed mesh buffer. The workshop builds its own shapes so the missile reads as a
/// rounded airframe instead of a pile of cylinders; no image texture is needed.
#[derive(Default)]
struct MeshData {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

impl MeshData {
    fn vertex(&mut self, position: [f32; 3], normal: [f32; 3]) -> u32 {
        let index = self.positions.len() as u32;
        self.positions.push(position);
        self.normals.push(normal);
        index
    }

    fn triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend([a, b, c]);
    }

    fn quad(&mut self, a: u32, b: u32, c: u32, d: u32) {
        self.indices.extend([a, b, c, a, c, d]);
    }

    fn build(self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

fn normalize3(value: [f32; 3]) -> [f32; 3] {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2])
        .sqrt()
        .max(1e-6);
    [value[0] / length, value[1] / length, value[2] / length]
}

/// Cross-section of a rounded rectangular body: four arcs joined by flat sides. It is the visual
/// counterpart of the abstract cylinder the rules use.
fn rounded_section(radius: f32, depth_ratio: f32, corner_ratio: f32, segments: usize) -> Vec<([f32; 2], [f32; 2])> {
    let half_x = radius.max(0.01);
    let half_z = (radius * depth_ratio).max(0.01);
    let corner = (radius * corner_ratio).min(half_x.min(half_z) * 0.85).max(0.001);
    let center_x = half_x - corner;
    let center_z = half_z - corner;
    let segments = segments.max(1);
    let mut points = Vec::with_capacity(4 * (segments + 1));
    for (sign_x, sign_z, start) in [
        (1.0_f32, 1.0_f32, 0.0_f32),
        (-1.0, 1.0, 90.0),
        (-1.0, -1.0, 180.0),
        (1.0, -1.0, 270.0),
    ] {
        for step in 0..=segments {
            let angle = (start + 90.0 * step as f32 / segments as f32).to_radians();
            let (sin, cos) = angle.sin_cos();
            points.push((
                [sign_x * center_x + corner * cos, sign_z * center_z + corner * sin],
                [cos, sin],
            ));
        }
    }
    points
}

fn rounded_prism_mesh(radius: f32, height: f32, depth_ratio: f32) -> Mesh {
    rounded_body_mesh(radius, radius, height, depth_ratio, None)
}

fn adapter_mesh(bottom_radius: f32, top_radius: f32, height: f32, shape: AdapterShape) -> Mesh {
    rounded_body_mesh(bottom_radius, top_radius, height, 0.78, Some(shape))
}

/// One continuous rounded body from a bottom section to a top section. An adapter passes two
/// different radii and a profile; every other body passes the same radius and no profile.
fn rounded_body_mesh(
    bottom_radius: f32,
    top_radius: f32,
    height: f32,
    depth_ratio: f32,
    shape: Option<AdapterShape>,
) -> Mesh {
    let bottom_radius = bottom_radius.max(0.01);
    let top_radius = top_radius.max(0.01);
    let height = height.max(0.01);
    let half_height = height * 0.5;
    let rings = if (bottom_radius - top_radius).abs() < 1e-4 {
        2
    } else {
        12
    };
    let blend = |t: f32| match shape {
        None | Some(AdapterShape::Conical) => t,
        Some(AdapterShape::Ogive) => t * t * (3.0 - 2.0 * t),
        Some(AdapterShape::Stepped) => ((t - 0.4) / 0.2).clamp(0.0, 1.0),
    };
    let radius_at = |t: f32| bottom_radius + (top_radius - bottom_radius) * blend(t);
    let mut mesh = MeshData::default();
    let mut vertices: Vec<Vec<u32>> = Vec::with_capacity(rings + 1);
    for ring in 0..=rings {
        let t = ring as f32 / rings as f32;
        let section = rounded_section(radius_at(t), depth_ratio, 0.34, 4);
        let previous = ring.saturating_sub(1) as f32 / rings as f32;
        let next = (ring + 1).min(rings) as f32 / rings as f32;
        let slope = (radius_at(next) - radius_at(previous)) / ((next - previous) * height).max(1e-6);
        let y = -half_height + height * t;
        let mut row = Vec::with_capacity(section.len());
        for (point, normal) in section {
            row.push(mesh.vertex([point[0], y, point[1]], normalize3([normal[0], -slope, normal[1]])));
        }
        vertices.push(row);
    }
    for pair in vertices.windows(2) {
        for index in 0..pair[0].len() {
            let end = (index + 1) % pair[0].len();
            mesh.quad(pair[0][index], pair[0][end], pair[1][end], pair[1][index]);
        }
    }
    for (y, radius, up) in [(-half_height, bottom_radius, false), (half_height, top_radius, true)] {
        let section = rounded_section(radius, depth_ratio, 0.34, 4);
        let normal = if up { [0.0, 1.0, 0.0] } else { [0.0, -1.0, 0.0] };
        let center = mesh.vertex([0.0, y, 0.0], normal);
        for index in 0..section.len() {
            let start = section[index].0;
            let end = section[(index + 1) % section.len()].0;
            let a = mesh.vertex([start[0], y, start[1]], normal);
            let b = mesh.vertex([end[0], y, end[1]], normal);
            if up {
                mesh.triangle(center, b, a);
            } else {
                mesh.triangle(center, a, b);
            }
        }
    }
    mesh.build()
}

/// Lathe around the body axis. `profile` pairs a height fraction with a radius scale; the normal
/// follows the profile tangent so a bell nozzle shades smoothly.
fn lathe_mesh(radius: f32, height: f32, profile: &[(f32, f32)], segments: usize, cap_top: bool) -> Mesh {
    let radius = radius.max(0.01);
    let height = height.max(0.01);
    let segments = segments.max(3);
    let mut rings = Vec::with_capacity(profile.len());
    for (index, &(fraction, scale)) in profile.iter().enumerate() {
        let previous = profile[index.saturating_sub(1)];
        let next = profile[(index + 1).min(profile.len() - 1)];
        let dr = (next.1 - previous.1) * radius;
        let dy = (next.0 - previous.0) * height;
        let length = (dr * dr + dy * dy).sqrt().max(1e-6);
        rings.push((fraction, scale, dy / length, -dr / length));
    }
    let mut mesh = MeshData::default();
    let mut vertices: Vec<Vec<u32>> = Vec::with_capacity(rings.len());
    for &(fraction, scale, radial, axial) in &rings {
        let y = -height * 0.5 + fraction * height;
        let ring_radius = (radius * scale).max(0.001);
        let mut ring = Vec::with_capacity(segments + 1);
        for step in 0..=segments {
            let angle = std::f32::consts::TAU * step as f32 / segments as f32;
            let (sin, cos) = angle.sin_cos();
            ring.push(mesh.vertex(
                [ring_radius * cos, y, ring_radius * sin],
                [cos * radial, axial, sin * radial],
            ));
        }
        vertices.push(ring);
    }
    for pair in vertices.windows(2) {
        for step in 0..segments {
            mesh.quad(pair[0][step], pair[0][step + 1], pair[1][step + 1], pair[1][step]);
        }
    }
    if cap_top && let Some(top) = vertices.last() {
        let center = mesh.vertex([0.0, height * 0.5, 0.0], [0.0, 1.0, 0.0]);
        for step in 0..segments {
            mesh.triangle(center, top[step], top[step + 1]);
        }
    }
    mesh.build()
}

/// One continuous tapered plate instead of eight abutting boxes: the planform and sweep read as a
/// single wing, and thin surfaces no longer show seams.
fn surface_mesh(span: f32, chord: f32, planform: WingPlanform, sweep_deg: f32, thickness: f32) -> Mesh {
    const SEGMENTS: usize = 10;
    let span = span.max(0.05);
    let chord = chord.max(0.05);
    let half_thickness = (thickness * 0.5).max(0.004);
    let mut mesh = MeshData::default();
    let mut top = Vec::with_capacity(SEGMENTS + 1);
    let mut bottom = Vec::with_capacity(SEGMENTS + 1);
    for index in 0..=SEGMENTS {
        let t = index as f32 / SEGMENTS as f32;
        let taper = match planform {
            WingPlanform::Straight | WingPlanform::Swept => 1.0,
            WingPlanform::Delta => 1.0 - 0.92 * t,
            WingPlanform::Trapezoidal => 1.0 - 0.45 * t,
            WingPlanform::VariableGeometry => 1.0 - 0.28 * t,
        };
        let chord_here = (chord * taper).max(0.025);
        let sweep = (sweep_deg / 60.0) * chord * 0.45 * t;
        let leading = chord * 0.5 - sweep;
        let trailing = leading - chord_here;
        let x = -span * 0.5 + span * t;
        let top_leading = mesh.vertex([x, leading, half_thickness], [0.0, 0.0, 1.0]);
        let top_trailing = mesh.vertex([x, trailing, half_thickness], [0.0, 0.0, 1.0]);
        let bottom_leading = mesh.vertex([x, leading, -half_thickness], [0.0, 0.0, -1.0]);
        let bottom_trailing = mesh.vertex([x, trailing, -half_thickness], [0.0, 0.0, -1.0]);
        top.push((top_leading, top_trailing));
        bottom.push((bottom_leading, bottom_trailing));
    }
    for index in 0..SEGMENTS {
        mesh.quad(top[index].0, top[index].1, top[index + 1].1, top[index + 1].0);
        mesh.quad(
            bottom[index].0,
            bottom[index + 1].0,
            bottom[index + 1].1,
            bottom[index].1,
        );
        mesh.quad(top[index].0, bottom[index].0, bottom[index + 1].0, top[index + 1].0);
        mesh.quad(top[index].1, top[index + 1].1, bottom[index + 1].1, bottom[index].1);
    }
    mesh.quad(top[0].0, top[0].1, bottom[0].1, bottom[0].0);
    mesh.quad(top[SEGMENTS].0, bottom[SEGMENTS].0, bottom[SEGMENTS].1, top[SEGMENTS].1);
    mesh.build()
}

fn nose_mesh(radius: f32, height: f32, shape: NoseShape) -> Mesh {
    const SEGMENTS: usize = 24;
    const RINGS: usize = 10;
    let radius = radius.max(0.02);
    let height = height.max(0.05);
    let half_height = height * 0.5;
    let profile = |t: f32| match shape {
        NoseShape::Conical => 1.0 - t,
        NoseShape::Ogive => (1.0 - t).powf(0.62),
        NoseShape::Parabolic => 1.0 - t * t,
    };
    let mut mesh = MeshData::default();
    let mut vertices: Vec<Vec<u32>> = Vec::with_capacity(RINGS);
    for index in 0..RINGS {
        let t = index as f32 / RINGS as f32;
        let previous_t = (t - 1.0 / RINGS as f32).max(0.0);
        let next_t = (t + 1.0 / RINGS as f32).min(1.0);
        let radius_at = |t: f32| (radius * profile(t)).max(0.001);
        let dr = radius_at(next_t) - radius_at(previous_t);
        let dy = (next_t - previous_t) * height;
        let length = (dr * dr + dy * dy).sqrt().max(1e-6);
        let radial = dy / length;
        let axial = -dr / length;
        let ring_radius = radius_at(t);
        let y = -half_height + t * height;
        let mut ring = Vec::with_capacity(SEGMENTS);
        for step in 0..SEGMENTS {
            let angle = std::f32::consts::TAU * step as f32 / SEGMENTS as f32;
            let (sin, cos) = angle.sin_cos();
            ring.push(mesh.vertex(
                [ring_radius * cos, y, ring_radius * sin],
                [cos * radial, axial, sin * radial],
            ));
        }
        vertices.push(ring);
    }
    for pair in vertices.windows(2) {
        for step in 0..SEGMENTS {
            let next = (step + 1) % SEGMENTS;
            mesh.quad(pair[0][step], pair[0][next], pair[1][next], pair[1][step]);
        }
    }
    let tip = mesh.vertex([0.0, half_height, 0.0], [0.0, 1.0, 0.0]);
    if let Some(last) = vertices.last() {
        for step in 0..SEGMENTS {
            let next = (step + 1) % SEGMENTS;
            mesh.triangle(last[step], last[next], tip);
        }
    }
    let base = mesh.vertex([0.0, -half_height, 0.0], [0.0, -1.0, 0.0]);
    for step in 0..SEGMENTS {
        let next = (step + 1) % SEGMENTS;
        mesh.triangle(base, vertices[0][step], vertices[0][next]);
    }
    mesh.build()
}

/// Centre the camera on the vehicle when the workshop is opened.
pub(crate) fn focus_missile_camera(
    campus: Res<CampusState>,
    session: Res<MissileSession>,
    windows: Query<&Window>,
    mut cameras: Query<(&mut Transform, &mut OrbitCameraRig), With<crate::scene::SceneCamera>>,
    mut previous: Local<Option<CampusView>>,
) {
    let current = campus.view;
    let entered = *previous != Some(CampusView::Missile) && current == CampusView::Missile;
    *previous = Some(current);
    if !entered {
        return;
    }
    let Ok((mut transform, mut rig)) = cameras.single_mut() else {
        return;
    };
    let points: Vec<Vec3> = session
        .vehicle
        .poses()
        .iter()
        .map(|pose| world_point(&session.vehicle, pose.x_m, pose.y_m, pose.z_m))
        .collect();
    let (target, extent) = if points.is_empty() {
        (ANCHOR, 8.0)
    } else {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for point in points {
            min = min.min(point);
            max = max.max(point);
        }
        ((min + max) * 0.5, (max - min).length().max(1.0))
    };
    let cruise = horizontal(&session.vehicle);
    let viewport_height = windows.iter().next().map_or(960.0, Window::height);
    rig.target = framing_target(&session.vehicle, target, viewport_height);
    rig.distance = (extent * 1.45 + 6.0).clamp(12.0, 45.0);
    rig.goal = None;
    // A light rear 3/4 keeps the lit side, the wings and the motor nozzle in view; the older
    // -1.05 rad angle looked almost down the fuselage and hid the nose behind the radar cap.
    rig.yaw = if cruise { -0.4 } else { 0.7 };
    rig.pitch = if cruise { 0.72 } else { 0.30 };
    let planar = rig.pitch.cos() * rig.distance;
    transform.translation = rig.target
        + Vec3::new(
            rig.yaw.sin() * planar,
            rig.pitch.sin() * rig.distance,
            rig.yaw.cos() * planar,
        );
    transform.look_at(rig.target, Vec3::Y);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn everything_owned() -> BTreeSet<String> {
        let tree = omega_rules::default_tree();
        let state = omega_rules::ResearchState {
            completed: tree.iter().map(|n| n.id.clone()).collect(),
            active: None,
        };
        state.owned_capabilities(&tree)
    }

    fn starting() -> BTreeSet<String> {
        omega_rules::ResearchState::default().owned_capabilities(&omega_rules::default_tree())
    }

    fn select(session: &mut MissileSession, id: &str) {
        session.apply(MissileAction::Select { id: id.to_string() }, &starting());
    }

    #[test]
    fn the_palette_covers_every_category_with_unique_labels() {
        let mut labels = BTreeSet::new();
        for template in PartTemplate::ALL {
            assert!(labels.insert(template.label()), "duplicate label {}", template.label());
        }
        let categories: BTreeSet<omega_rules::PartCategory> =
            PartTemplate::ALL.iter().map(|t| t.spec().category()).collect();
        assert_eq!(
            categories.len(),
            omega_rules::PartCategory::ALL.len(),
            "every palette category has at least one part"
        );
    }

    #[test]
    fn default_preset_is_a_valid_two_stage_vehicle() {
        let session = MissileSession::new_default();
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
        assert_eq!(session.vehicle.stage_count(), 2);
        assert_eq!(session.selected.as_deref(), Some("module-6"));
    }

    #[test]
    fn a_fixed_size_part_gets_a_mounting_diameter_range() {
        let mut session = MissileSession::new_default();
        let owned = starting();
        select(&mut session, "computer-4");
        assert_eq!(
            params_of(session.vehicle.part("computer-4").unwrap()),
            vec![(Param::Size, 0.3)]
        );
        session.apply(MissileAction::Adjust(Param::Size, 1), &owned);
        session.apply(MissileAction::Adjust(Param::Size, 1), &owned);
        let part = session.vehicle.part("computer-4").unwrap();
        assert!((part.diameter_m() - 0.4).abs() < 1e-9);
        let bytes = session.to_bytes().unwrap();
        let restored = MissileSession::from_bytes(&bytes).unwrap();
        assert!((restored.vehicle.part("computer-4").unwrap().diameter_m() - 0.4).abs() < 1e-9);
    }

    #[test]
    fn the_player_places_and_parameters_the_nose_cone() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        select(&mut session, "module-6");
        let added = session.apply(MissileAction::AddPart(PartTemplate::Nose), &owned);
        assert!(added.contains("Added"), "{added}");
        let nose = session.selected.clone().unwrap();
        assert!(matches!(
            session.vehicle.part(&nose).unwrap().spec,
            PartSpec::NoseCone { .. }
        ));
        session.apply(MissileAction::Adjust(Param::Length, 1), &owned);
        session.apply(MissileAction::Adjust(Param::Diameter, -1), &owned);
        session.apply(MissileAction::CycleAero(AeroChoice::NoseShape), &owned);
        let PartSpec::NoseCone {
            length_m,
            diameter_m,
            shape,
        } = session.vehicle.part(&nose).unwrap().spec
        else {
            panic!("the nose part keeps its spec");
        };
        assert!((length_m - 0.7).abs() < 1e-9);
        assert!((diameter_m - 0.35).abs() < 1e-9);
        assert_eq!(shape, NoseShape::Parabolic);
        let blocked = session.apply(MissileAction::AddPart(PartTemplate::FuelTank), &owned);
        assert!(blocked.contains("nose cone closes the stack"), "{blocked}");
        session.apply(MissileAction::RemovePart, &owned);
        assert!(
            layout(&session.vehicle)
                .iter()
                .all(|part| !matches!(part.shape, Shape::Nose(_))),
            "no nose cap is generated once the player removes the part"
        );
    }

    #[test]
    fn adjusting_a_part_resizes_it_within_limits() {
        let mut session = MissileSession::new_default();
        let owned = starting();
        select(&mut session, "motor-1");
        let before = session.vehicle.part("motor-1").unwrap().spec.stats();
        session.apply(MissileAction::Adjust(Param::Length, 1), &owned);
        session.apply(MissileAction::Adjust(Param::BurnTime, -1), &owned);
        let after = session.vehicle.part("motor-1").unwrap().spec.stats();
        assert!(after.propellant_kg > before.propellant_kg);
        assert!(
            after.thrust_n > before.thrust_n,
            "longer motor and shorter burn = more thrust"
        );
        for _ in 0..200 {
            session.apply(MissileAction::Adjust(Param::BurnTime, -1), &owned);
        }
        assert_eq!(params_of(session.vehicle.part("motor-1").unwrap())[2].1, 1.0);
        assert!(session.vehicle.part("motor-1").unwrap().spec.validate().is_ok());
        assert!(
            session
                .apply(MissileAction::Adjust(Param::Scale, 1), &owned)
                .contains("no such setting")
        );
    }

    #[test]
    fn locked_parts_cannot_be_added_until_researched() {
        let mut session = MissileSession::new_default();
        let count = session.vehicle.parts.len();
        let message = session.apply(MissileAction::AddPart(PartTemplate::RadarActive), &starting());
        assert!(
            message.contains("Locked") && message.contains("sensor:radar_active"),
            "{message}"
        );
        assert_eq!(session.vehicle.parts.len(), count);
        let ok = session.apply(MissileAction::AddPart(PartTemplate::RadarActive), &everything_owned());
        assert!(ok.contains("Added"), "{ok}");
        assert_eq!(session.vehicle.parts.len(), count + 1);
    }

    #[test]
    fn a_new_part_is_attached_above_the_selected_part() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        select(&mut session, "module-6");
        session.apply(MissileAction::AddPart(PartTemplate::FuelTank), &owned);
        let tank = session.selected.clone().unwrap();
        assert_eq!(
            session.vehicle.part(&tank).unwrap().attach.as_ref().unwrap().parent,
            "module-6"
        );
        assert_eq!(session.vehicle.stage_of(&tank), Some(1));
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
    }

    #[test]
    fn a_second_computer_replaces_the_first() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.apply(MissileAction::AddPart(PartTemplate::ComputerAdvanced), &owned);
        let computers: Vec<_> = session
            .vehicle
            .parts
            .iter()
            .filter(|p| matches!(p.spec, PartSpec::Computer { .. }))
            .collect();
        assert_eq!(computers.len(), 1);
        assert_eq!(session.vehicle.computer(), Some(ComputerClass::Advanced));
        assert_eq!(computers[0].id, "computer-4", "the existing computer keeps its id");
    }

    #[test]
    fn removing_a_part_warns_when_a_subtree_goes_with_it() {
        let mut session = MissileSession::new_default();
        let owned = starting();
        select(&mut session, "decoupler-2");
        let before = session.vehicle.parts.len();
        let message = session.apply(MissileAction::RemovePart, &owned);
        assert!(message.contains("attached part(s)"), "{message}");
        assert_eq!(
            session.vehicle.parts.len(),
            1,
            "the decoupler and the whole upper stack are gone"
        );
        assert!(session.vehicle.validate().is_err());
        session.apply(MissileAction::Clear, &owned);
        assert!(session.vehicle.parts.is_empty());
        assert!(session.vehicle.validate().is_err());
        assert_eq!(session.vehicle.parts.len(), before - before);
    }

    #[test]
    fn the_aero_presets_are_valid_and_show_the_airframe() {
        let mut session = MissileSession::default();
        session.load_preset(2);
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
        let trainer = session.vehicle.aero();
        assert_eq!(trainer.surface_count, 4);
        assert!(trainer.stability > 60.0, "stability {}", trainer.stability);
        assert_eq!(session.vehicle.name, "Fin trainer");

        session.load_preset(3);
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
        assert!(session.vehicle.aero().lift > 40.0);
        assert_eq!(session.vehicle.name, "Winged cruise");
        assert_eq!(session.vehicle.parts.len(), 7);
    }

    #[test]
    fn presets_respect_research_and_ids_stay_unique() {
        let mut session = MissileSession::new_default();
        assert!(session.apply(MissileAction::Preset(1), &starting()).contains("locked"));
        assert_eq!(
            session.vehicle.name, "Two-stage rocket",
            "a locked preset does not replace the design"
        );
        assert!(
            session
                .apply(MissileAction::Preset(1), &everything_owned())
                .contains("Cruise vehicle")
        );
        let owned = everything_owned();
        for _ in 0..3 {
            session.apply(MissileAction::AddPart(PartTemplate::FuelTank), &owned);
        }
        let ids: Vec<&str> = session.vehicle.parts.iter().map(|p| p.id.as_str()).collect();
        let unique: BTreeSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "{ids:?}");
    }

    #[test]
    fn vehicle_diff_reports_edits_and_mass() {
        let session = MissileSession::new_default();
        assert_eq!(vehicle_diff(&session.vehicle, &session.vehicle), "no changes");
        let mut edited = session.vehicle.clone();
        edited.replace_part(
            "motor-1",
            PartSpec::SolidMotor {
                length_m: 3.0,
                diameter_m: 0.4,
                burn_time_s: 20.0,
            },
        );
        let summary = vehicle_diff(&session.vehicle, &edited);
        assert!(summary.contains("1 changed") && summary.contains("mass +"), "{summary}");
        let mut removed = session.vehicle.clone();
        removed.remove_subtree("module-6");
        assert!(vehicle_diff(&session.vehicle, &removed).contains("1 removed"));
    }

    #[test]
    fn layout_places_the_stack_and_radial_parts() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        let boxes = layout(&session.vehicle);
        assert_eq!(boxes.len(), 6);
        let motor = boxes.iter().find(|b| b.id == "motor-1").unwrap();
        let sustainer = boxes.iter().find(|b| b.id == "motor-3").unwrap();
        assert!(sustainer.y_bottom > motor.y_bottom);
        assert_eq!(motor.x, 0.0);
        session.apply(MissileAction::AddPart(PartTemplate::SolidMotor), &owned);
        let booster = session.selected.clone().unwrap();
        session.vehicle.detach(&booster).unwrap();
        session
            .vehicle
            .attach(&booster, "motor-1", omega_rules::NodeId::Radial(0), 0.0)
            .unwrap();
        let boxes = layout(&session.vehicle);
        let radial = boxes.iter().find(|b| b.id == booster).unwrap();
        assert!(radial.x.abs() > 0.1, "a radial part sits off the centre line");
    }

    #[test]
    fn an_adapter_changes_diameter_and_shape_without_a_step() {
        let mut session = MissileSession::default();
        let owned = everything_owned();
        session.apply(MissileAction::Clear, &owned);
        session.add_at(PartTemplate::SolidMotor, None, &owned);
        let motor = session.selected.clone().unwrap();
        session.vehicle.replace_part(
            &motor,
            PartSpec::SolidMotor {
                length_m: 1.5,
                diameter_m: 0.6,
                burn_time_s: 20.0,
            },
        );
        session.add_at(PartTemplate::Adapter, None, &owned);
        let adapter = session.selected.clone().unwrap();
        session.vehicle.replace_part(
            &adapter,
            PartSpec::Adapter {
                length_m: 0.5,
                diameter_m: 0.6,
                exit_diameter_m: 0.3,
                shape: AdapterShape::Conical,
            },
        );
        session.add_at(PartTemplate::FuelTank, None, &owned);
        let tank = session.selected.clone().unwrap();
        session.vehicle.replace_part(
            &tank,
            PartSpec::FuelTank {
                length_m: 1.0,
                diameter_m: 0.3,
            },
        );
        session.add_at(PartTemplate::ComputerBasic, None, &owned);
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
        assert!(
            !session.vehicle.warnings().iter().any(|w| w.contains("diameter step")),
            "the adapter fills both diameter steps: {:?}",
            session.vehicle.warnings()
        );

        let boxes = layout(&session.vehicle);
        let adapter_box = boxes.iter().find(|part| part.id == adapter).unwrap();
        let tank_box = boxes.iter().find(|part| part.id == tank).unwrap();
        assert!((adapter_box.bottom_radius - 0.3).abs() < 1e-5);
        assert!((adapter_box.top_radius - 0.15).abs() < 1e-5);
        assert_eq!(adapter_box.shape, Shape::Adapter(AdapterShape::Conical));
        assert!((tank_box.y_bottom - (adapter_box.y_bottom + adapter_box.height)).abs() < 1e-5);

        select(&mut session, &adapter);
        session.apply(MissileAction::CycleAero(AeroChoice::AdapterShape), &owned);
        let changed = layout(&session.vehicle)
            .into_iter()
            .find(|part| part.id == adapter)
            .unwrap();
        assert_eq!(changed.shape, Shape::Adapter(AdapterShape::Ogive));
        session.apply(MissileAction::Adjust(Param::ExitDiameter, 1), &owned);
        assert_eq!(params_of(session.vehicle.part(&adapter).unwrap())[2].1, 0.35);
    }

    #[test]
    fn cruise_vehicles_are_laid_out_horizontally() {
        let rocket = MissileSession::new_default();
        assert!(!horizontal(&rocket.vehicle));
        assert_eq!(orientation(&rocket.vehicle), Quat::IDENTITY);

        let mut cruise = MissileSession::default();
        cruise.load_preset(1);
        assert!(horizontal(&cruise.vehicle));
        // The horizontal presentation is lifted clear of the assembly pad.
        let deck = ANCHOR.y + HORIZONTAL_LIFT;
        let nose = world_point(&cruise.vehicle, 0.0, 1.0, 0.0);
        assert!((nose.x - (ANCHOR.x + VISUAL_SCALE)).abs() < 1e-5, "{nose}");
        assert!((nose.y - deck).abs() < 1e-5, "{nose}");
        let wing_side = world_point(&cruise.vehicle, 1.0, 0.0, 0.0);
        assert!((wing_side.z - (ANCHOR.z + VISUAL_SCALE)).abs() < 1e-5, "{wing_side}");
        assert!((wing_side.y - deck).abs() < 1e-5, "{wing_side}");
    }

    #[test]
    fn a_resize_reapplies_exactly_the_fresh_framing_bias() {
        let mut cruise = MissileSession::default();
        cruise.load_preset(1);
        let point = world_point(&cruise.vehicle, 0.0, 1.0, 0.0);
        let narrow = framing_target(&cruise.vehicle, point, 768.0);
        let wide = framing_target(&cruise.vehicle, point, 960.0);
        let resized = reframe_on_resize(&cruise.vehicle, narrow, 768.0, 960.0);
        assert!((resized - wide).length() < 1e-5, "{resized} vs {wide}");
    }

    #[test]
    fn body_sections_are_rounded_rectangles_not_circles() {
        let section = rounded_section(0.4, 0.78, 0.34, 4);
        assert_eq!(section.len(), 20);
        let width = section
            .iter()
            .map(|(point, _)| point[0])
            .fold(f32::NEG_INFINITY, f32::max)
            - section.iter().map(|(point, _)| point[0]).fold(f32::INFINITY, f32::min);
        let depth = section
            .iter()
            .map(|(point, _)| point[1])
            .fold(f32::NEG_INFINITY, f32::max)
            - section.iter().map(|(point, _)| point[1]).fold(f32::INFINITY, f32::min);
        assert!((width - 0.8).abs() < 1e-5, "{width}");
        assert!((depth - 0.624).abs() < 1e-5, "{depth}");
        assert!(depth < width, "the section is a flattened rectangle");
        assert!(
            section
                .iter()
                .all(|(point, _)| point[0].abs() <= 0.4 + 1e-6 && point[1].abs() <= 0.312 + 1e-6)
        );
    }

    #[test]
    fn radial_symmetry_creates_linked_copies() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.symmetry = 3;
        let message = session.add_at(
            PartTemplate::SolidMotor,
            Some(("motor-1".into(), omega_rules::NodeId::Radial(0), 0.0)),
            &owned,
        );
        assert!(message.contains("3 copies"), "{message}");
        let boosters: Vec<_> = session
            .vehicle
            .parts
            .iter()
            .filter(|p| {
                p.symmetry.as_deref().is_some_and(|group| {
                    let (key, kind, _) = omega_rules::assembly::decode_symmetry(group);
                    key == "sym-1" && kind == omega_rules::assembly::SymmetryKind::Radial
                })
            })
            .collect();
        assert_eq!(boosters.len(), 3);
        let slots: BTreeSet<_> = boosters
            .iter()
            .filter_map(|p| p.attach.as_ref().map(|a| a.node))
            .collect();
        assert_eq!(slots.len(), 3);
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
        session.undo();
        assert!(session.vehicle.parts.iter().all(|p| p.symmetry.is_none()));
    }

    #[test]
    fn a_subtree_can_be_moved_and_bad_moves_are_refused() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        let message = session.move_part("module-6", "motor-3", omega_rules::NodeId::Radial(0), 0.0);
        assert!(message.starts_with("Moved"), "{message}");
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
        let refused = session.move_part("motor-1", "module-6", omega_rules::NodeId::Top, 0.0);
        assert!(refused.contains("root"), "moving the root is refused: {refused}");
        let cycle = session.move_part("motor-3", "module-6", omega_rules::NodeId::Top, 0.0);
        assert!(cycle.contains("root") || cycle.contains("cycle"), "{cycle}");
        assert!(session.vehicle.part("module-6").unwrap().attach.is_some());
        let _ = owned;
    }

    #[test]
    fn clearing_then_undoing_keeps_ids_unique() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.apply(MissileAction::Clear, &owned);
        session.undo();
        session.add_at(PartTemplate::SolidMotor, None, &owned);
        session.add_at(PartTemplate::FuelTank, None, &owned);
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
        let ids: BTreeSet<_> = session.vehicle.parts.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids.len(), session.vehicle.parts.len());
    }

    #[test]
    fn a_wheel_gesture_is_one_undo_entry() {
        let mut session = MissileSession::new_default();
        select(&mut session, "motor-1");
        let start = Instant::now();
        let before = session.vehicle.part("motor-1").unwrap().spec;
        session.adjust_wheel_at(Param::Length, 1, start);
        session.adjust_wheel_at(Param::Length, 1, start + Duration::from_millis(120));
        session.adjust_wheel_at(Param::Length, 1, start + Duration::from_millis(240));
        assert_eq!(session.undo.len(), 1, "one snapshot for the whole gesture");
        session.undo();
        assert_eq!(session.vehicle.part("motor-1").unwrap().spec, before);
        // A pause ends the gesture, so the next notch is a new entry.
        select(&mut session, "motor-1");
        session.adjust_wheel_at(Param::Length, 1, start + Duration::from_secs(2));
        session.adjust_wheel_at(Param::Length, 1, start + Duration::from_secs(3));
        assert_eq!(session.undo.len(), 2);
        assert_eq!(session.wheel_gesture.as_ref().unwrap().0.1, Param::Length);
    }

    #[test]
    fn a_button_edit_closes_the_wheel_gesture() {
        let mut session = MissileSession::new_default();
        select(&mut session, "motor-1");
        let owned = everything_owned();
        let start = Instant::now();
        session.adjust_wheel_at(Param::Length, 1, start);
        session.apply(MissileAction::Adjust(Param::Length, 1), &owned);
        session.adjust_wheel_at(Param::Length, 1, start + Duration::from_millis(100));
        assert_eq!(session.undo.len(), 3, "three separate edits");
    }

    #[test]
    fn undo_and_redo_walk_the_edit_history() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        let before = session.vehicle.parts.len();
        session.add_at(PartTemplate::FuelTank, None, &owned);
        assert_eq!(session.vehicle.parts.len(), before + 1);
        assert!(session.undo().contains("Undid"));
        assert_eq!(session.vehicle.parts.len(), before);
        assert!(session.redo().contains("Redid"));
        assert_eq!(session.vehicle.parts.len(), before + 1);
        session.delete_selected();
        assert!(session.undo().contains("Undid"), "deletion is undoable too");
    }

    #[test]
    fn legacy_files_load_and_save_as_schema_seven() {
        let legacy = br#"{"name":"Old","stages":[{"parts":[{"id":"m","spec":{"part":"solid_motor","length_m":2.0,"diameter_m":0.4,"burn_time_s":20.0}},{"id":"pc","spec":{"part":"computer","class":"basic"}}]}]}"#;
        let session = MissileSession::from_bytes(legacy).unwrap();
        assert_eq!(session.vehicle.stage_count(), 1);
        assert_eq!(session.vehicle.aero().surface_count, 0);
        let saved = String::from_utf8(session.to_bytes().unwrap()).unwrap();
        assert!(saved.contains("\"schema_version\": 7"), "{saved}");
        let again = MissileSession::from_bytes(saved.as_bytes()).unwrap();
        assert_eq!(again.vehicle, session.vehicle);
    }

    #[test]
    fn mission_electronics_need_a_power_cell_before_a_trial() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        let message = session.add_at(PartTemplate::InertialNav, None, &owned);
        assert!(message.contains("Added"), "{message}");
        let errors = session.vehicle.validate().unwrap_err().join(" | ");
        assert!(errors.contains("no power source"), "{errors}");

        let message = session.add_at(PartTemplate::PowerCell, None, &owned);
        assert!(message.contains("Added"), "{message}");
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
        assert_eq!(session.vehicle.avionics().mission_modules, 2);
    }

    #[test]
    fn wings_are_added_to_a_radial_mount_and_mirror_with_symmetry() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.symmetry = 4;
        let message = session.add_at(
            PartTemplate::Wing,
            Some(("motor-1".into(), omega_rules::NodeId::Radial(0), 0.0)),
            &owned,
        );
        assert!(message.contains("4 copies"), "{message}");
        let aero = session.vehicle.aero();
        assert_eq!(aero.surface_count, 4);
        assert!(aero.wing_area_m2 > 1.0);
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
        let slots: BTreeSet<_> = session
            .vehicle
            .parts
            .iter()
            .filter(|part| part.id.starts_with("wing-"))
            .filter_map(|part| part.attach.as_ref().map(|attach| attach.node))
            .collect();
        assert_eq!(slots.len(), 4);
        // A palette click finds another free radial mount instead of stacking on the axis.
        session.symmetry = 1;
        let added = session.add_at(PartTemplate::Wing, None, &owned);
        assert!(added.contains("Added"), "{added}");
        assert_eq!(session.vehicle.aero().surface_count, 5);
        let extra = session.selected.clone().unwrap();
        assert!(
            session
                .vehicle
                .part(&extra)
                .unwrap()
                .attach
                .as_ref()
                .unwrap()
                .node
                .is_radial()
        );
    }

    #[test]
    fn clicking_an_aero_part_finds_a_free_radial_slot() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.apply(MissileAction::Clear, &owned);
        session.apply(MissileAction::AddPart(PartTemplate::SolidMotor), &owned);
        session.apply(MissileAction::AddPart(PartTemplate::ComputerBasic), &owned);
        let motor = session
            .vehicle
            .parts
            .iter()
            .find(|part| matches!(part.spec, PartSpec::SolidMotor { .. }))
            .map(|part| part.id.clone())
            .unwrap();
        session.apply(MissileAction::Select { id: motor.clone() }, &owned);
        let message = session.add_at(PartTemplate::FinSet, None, &owned);
        assert!(message.contains("Added"), "{message}");
        let fin = session.selected.clone().unwrap();
        let attach = session.vehicle.part(&fin).unwrap().attach.as_ref().unwrap();
        assert_eq!(attach.parent, motor);
        assert!(attach.node.is_radial());
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
    }

    #[test]
    fn ksp_transform_gizmo_edits_all_axes_and_undoes() {
        let mut session = MissileSession::new_default();
        select(&mut session, "module-6");
        let before = session
            .vehicle
            .poses()
            .into_iter()
            .find(|pose| pose.id == "module-6")
            .unwrap();
        assert!(session.transform_selected(0, 0.5, false).contains("X"));
        assert!(session.transform_selected(1, 0.25, false).contains("Y"));
        assert!(session.transform_selected(2, -0.75, false).contains("Z"));
        assert!(session.transform_selected(0, 15.0, true).contains("X"));
        assert!(session.transform_selected(1, 25.0, true).contains("Y"));
        assert!(session.transform_selected(2, 35.0, true).contains("Z"));
        let part = session.vehicle.part("module-6").unwrap();
        let attach = part.attach.as_ref().unwrap();
        assert_eq!(
            (attach.offset_x_m, attach.offset_y_m, attach.offset_z_m),
            (0.5, 0.25, -0.75)
        );
        assert_eq!(
            (attach.rotation_x_deg, attach.rotation_y_deg, attach.rotation_z_deg),
            (15.0, 25.0, 35.0)
        );
        let after = session
            .vehicle
            .poses()
            .into_iter()
            .find(|pose| pose.id == "module-6")
            .unwrap();
        assert!((after.x_m - before.x_m - 0.5).abs() < 1e-9);
        assert!((after.y_m - before.y_m - 0.25).abs() < 1e-9);
        assert!((after.z_m - before.z_m + 0.75).abs() < 1e-9);
        for _ in 0..6 {
            session.undo();
        }
        let restored = session.vehicle.part("module-6").unwrap().attach.as_ref().unwrap();
        assert_eq!(
            (restored.offset_x_m, restored.offset_y_m, restored.offset_z_m),
            (0.0, 0.0, 0.0)
        );
        assert_eq!(
            (
                restored.rotation_x_deg,
                restored.rotation_y_deg,
                restored.rotation_z_deg
            ),
            (0.0, 0.0, 0.0)
        );
    }

    #[test]
    fn inspector_values_are_exact_and_one_undo_entry() {
        let mut session = MissileSession::new_default();
        select(&mut session, "module-6");
        let before = session.transform_values("module-6").unwrap();
        assert_eq!(before, [0.0; 6]);
        let target = [12.345, -6.25, 0.001, -12.3, 45.6, -359.9];
        session.set_transform_values("module-6", target).unwrap();
        assert_eq!(session.transform_values("module-6"), Some(target));
        assert_eq!(session.undo.len(), 1, "one entry for a whole reset or direct entry");
        session.undo();
        assert_eq!(session.transform_values("module-6"), Some(before));
        assert_eq!(session.redo.len(), 1);
        session.redo();
        assert_eq!(session.transform_values("module-6"), Some(target));
    }

    #[test]
    fn invalid_inspector_values_leave_the_vehicle_untouched() {
        let mut session = MissileSession::new_default();
        select(&mut session, "module-6");
        session
            .set_transform_values("module-6", [1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
            .unwrap();
        let valid = session.transform_values("module-6").unwrap();
        let undo_len = session.undo.len();
        for invalid in [
            [f64::NAN, 0.0, 0.0, 0.0, 0.0, 0.0],
            [f64::INFINITY, 0.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 1001.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 3600.5],
        ] {
            assert!(session.set_transform_values("module-6", invalid).is_err());
            assert_eq!(session.transform_values("module-6"), Some(valid));
            assert_eq!(session.undo.len(), undo_len);
        }
        let root = session
            .vehicle
            .parts
            .iter()
            .find(|part| part.attach.is_none())
            .map(|part| part.id.clone())
            .unwrap();
        assert!(session.set_transform_values(&root, [0.0; 6]).is_err());
        assert!(session.set_transform_value("module-6", 3, false, 1.0).is_err());
    }

    #[test]
    fn selected_aerodynamic_surface_rotates_and_can_be_undone() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.apply(MissileAction::Clear, &owned);
        session.apply(MissileAction::AddPart(PartTemplate::SolidMotor), &owned);
        let motor = session.selected.clone().unwrap();
        session.add_at(
            PartTemplate::FinSet,
            Some((motor, omega_rules::NodeId::Radial(0), 0.0)),
            &owned,
        );
        let fin = session.selected.clone().unwrap();

        assert!(session.rotate_selected(1).contains("15"));
        assert_eq!(
            session.vehicle.part(&fin).unwrap().attach.as_ref().unwrap().angle_deg,
            15.0
        );
        session.undo();
        assert_eq!(
            session.vehicle.part(&fin).unwrap().attach.as_ref().unwrap().angle_deg,
            0.0
        );
    }

    #[test]
    fn cycling_design_choices_changes_the_figures_and_respects_research() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.add_at(
            PartTemplate::Wing,
            Some(("motor-1".into(), omega_rules::NodeId::Radial(0), 0.0)),
            &owned,
        );
        let wing = session.selected.clone().unwrap();
        let before = session.vehicle.aero();
        session.apply(MissileAction::CycleAero(AeroChoice::Planform), &owned);
        let delta = session.vehicle.aero();
        assert_ne!(delta.lift, before.lift);
        assert!(delta.lift > before.lift, "a delta wing lifts more");
        session.undo();
        assert_eq!(session.vehicle.aero().lift, before.lift);

        session.apply(MissileAction::Select { id: wing.clone() }, &owned);
        session.apply(MissileAction::CycleAero(AeroChoice::Light), &owned);
        let light = session.vehicle.aero();
        assert!(light.mass_kg < before.mass_kg);
        session.undo();

        // Experimental profiles are gated by the advanced aerodynamics research.
        let mut limited = everything_owned();
        limited.remove("aero:advanced_profiles");
        session.apply(MissileAction::Select { id: wing.clone() }, &owned);
        session.apply(MissileAction::CycleAero(AeroChoice::Profile), &limited); // LOW DRAG
        session.apply(MissileAction::CycleAero(AeroChoice::Profile), &limited); // HIGH LIFT
        let refused = session.apply(MissileAction::CycleAero(AeroChoice::Profile), &limited);
        assert!(refused.contains("Locked"), "experimental profile: {refused}");
        let accepted = session.apply(MissileAction::CycleAero(AeroChoice::Profile), &owned);
        assert!(accepted.contains("EXPERIMENTAL"), "{accepted}");
    }

    #[test]
    fn wing_angle_is_editable_undoable_and_changes_gameplay_figures() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.add_at(
            PartTemplate::Wing,
            Some(("motor-1".into(), omega_rules::NodeId::Radial(0), 0.0)),
            &owned,
        );
        let wing = session.selected.clone().unwrap();
        let before = session.vehicle.aero();

        let message = session.apply(MissileAction::CycleAero(AeroChoice::Angle), &owned);
        assert!(message.contains("+5°"), "{message}");
        let attach = session.vehicle.part(&wing).unwrap().attach.as_ref().unwrap();
        assert_eq!(attach.rotation_x_deg, 5.0);
        let after = session.vehicle.aero();
        assert!(after.lift < before.lift);
        assert!(after.drag_index > before.drag_index);

        assert_eq!(
            choice_value_for_part(session.vehicle.part(&wing).unwrap(), AeroChoice::Angle),
            "+5°"
        );
        session.undo();
        let restored = session.vehicle.part(&wing).unwrap().attach.as_ref().unwrap();
        assert_eq!(restored.rotation_x_deg, 0.0);
        assert_eq!(session.vehicle.aero(), before);
    }

    #[test]
    fn wing_planform_is_preserved_in_the_visual_layout() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.add_at(
            PartTemplate::Wing,
            Some(("motor-1".into(), omega_rules::NodeId::Radial(0), 0.0)),
            &owned,
        );
        let wing_id = session.selected.clone().unwrap();
        let first = layout(&session.vehicle)
            .into_iter()
            .find(|part| part.id == wing_id)
            .unwrap();
        assert_eq!(first.planform, Some(WingPlanform::Swept));
        assert_eq!(first.sweep_deg, 20.0);

        session.apply(MissileAction::CycleAero(AeroChoice::Planform), &owned);
        let changed = layout(&session.vehicle)
            .into_iter()
            .find(|part| part.id == wing_id)
            .unwrap();
        assert_eq!(changed.planform, Some(WingPlanform::Delta));
    }

    #[test]
    fn the_aero_comparison_keeps_the_previous_configuration() {
        let mut session = MissileSession::new_default();
        assert!(session.comparison.is_none());
        session.apply(MissileAction::CompareAero, &starting());
        let saved = session.comparison.clone().unwrap();
        assert_eq!(saved.surface_count, 0);
        let owned = everything_owned();
        session.add_at(
            PartTemplate::FinSet,
            Some(("motor-1".into(), omega_rules::NodeId::Radial(0), 0.0)),
            &owned,
        );
        assert!(session.vehicle.aero().stability > saved.stability);
        session.apply(MissileAction::ClearComparison, &owned);
        assert!(session.comparison.is_none());
    }
    #[test]
    fn ctrl_click_toggles_a_multiple_selection_and_escape_clears_it() {
        let mut session = MissileSession::new_default();
        session.apply(MissileAction::Select { id: "module-6".into() }, &starting());
        session.apply(
            MissileAction::ToggleSelect {
                id: "computer-4".into(),
            },
            &starting(),
        );
        assert_eq!(session.selection.len(), 2);
        assert_eq!(session.selected.as_deref(), Some("computer-4"));

        session.apply(MissileAction::ToggleSelect { id: "module-6".into() }, &starting());
        assert_eq!(session.selection.len(), 1);
        assert_eq!(session.selected.as_deref(), Some("computer-4"));

        session.apply(MissileAction::ClearSelection, &starting());
        assert!(session.selection.is_empty());
        assert!(session.selected.is_none());
    }

    #[test]
    fn a_group_selection_covers_the_whole_subtree() {
        let mut session = MissileSession::new_default();
        let parent = session
            .vehicle
            .parts
            .iter()
            .find(|part| !session.vehicle.children(&part.id).is_empty())
            .map(|part| part.id.clone())
            .expect("the default design has a parent part");
        let expected: BTreeSet<String> = session.vehicle.subtree_ids(&parent).into_iter().collect();
        session.apply(MissileAction::SelectGroup { id: parent.clone() }, &starting());
        assert_eq!(session.selection, expected);
        assert_eq!(session.selected.as_deref(), Some(parent.as_str()));
    }

    #[test]
    fn collapsing_and_renaming_a_group_change_only_the_tree_view() {
        let mut session = MissileSession::new_default();
        let parent = session
            .vehicle
            .parts
            .iter()
            .find(|part| !session.vehicle.children(&part.id).is_empty())
            .map(|part| part.id.clone())
            .expect("the default design has a parent part");
        let rows_before = session.tree_rows().len();
        session.apply(MissileAction::ToggleCollapsed { id: parent.clone() }, &starting());
        let rows = session.tree_rows();
        assert!(rows.iter().any(|row| row.id == parent));
        assert!(
            !rows
                .iter()
                .any(|row| row.depth > 0 && row.id != parent && session.vehicle.is_descendant(&row.id, &parent))
        );
        assert!(rows.len() < rows_before);

        session.apply(MissileAction::BeginRename { id: parent.clone() }, &starting());
        session.rename_draft = "Booster".to_string();
        session.apply(MissileAction::CommitRename, &starting());
        assert_eq!(session.group_name(&parent), Some("Booster"));
    }

    #[test]
    fn searching_the_palette_matches_labels_and_help_text() {
        let mut session = MissileSession::new_default();
        session.palette_search = "coiffe".to_string();
        assert!(session.palette_matches(PartTemplate::Nose));
        assert!(!session.palette_matches(PartTemplate::SolidMotor));
        session.palette_search = "wing".to_string();
        assert!(session.palette_matches(PartTemplate::Wing));
        session.palette_search.clear();
        assert!(session.palette_matches(PartTemplate::SolidMotor));
    }

    #[test]
    fn a_locked_child_protects_its_ancestor_from_deletion_and_movement() {
        let mut session = MissileSession::new_default();
        let (parent, child) = session
            .vehicle
            .parts
            .iter()
            .find_map(|part| {
                session
                    .vehicle
                    .children(&part.id)
                    .first()
                    .map(|child| (part.id.clone(), child.id.clone()))
            })
            .expect("the default design has a parent and a child");
        session.apply(MissileAction::ToggleLock { id: child.clone() }, &starting());
        session.apply(MissileAction::Select { id: parent.clone() }, &starting());
        let blocked = session.apply(MissileAction::RemovePart, &starting());
        assert!(blocked.contains("locked"), "{blocked}");
        assert!(session.vehicle.part(&child).is_some());
        assert!(session.vehicle.part(&parent).is_some());

        session.apply(MissileAction::ToggleLock { id: child }, &starting());
        session.apply(MissileAction::RemovePart, &starting());
        assert!(session.vehicle.part(&parent).is_none());
    }

    #[test]
    fn a_multi_selection_moves_every_independent_root_by_the_same_delta() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.add_at(
            PartTemplate::FinSet,
            Some(("motor-1".into(), omega_rules::NodeId::Radial(0), 0.0)),
            &owned,
        );
        let a = session.selected.clone().unwrap();
        session.add_at(
            PartTemplate::FinSet,
            Some(("motor-1".into(), omega_rules::NodeId::Radial(1), 0.0)),
            &owned,
        );
        let b = session.selected.clone().unwrap();
        session.apply(MissileAction::Select { id: a.clone() }, &starting());
        session.apply(MissileAction::ToggleSelect { id: b.clone() }, &starting());
        let offset =
            |session: &MissileSession, id: &str| session.vehicle.part(id).unwrap().attach.as_ref().unwrap().offset_x_m;
        let before_a = offset(&session, &a);
        let before_b = offset(&session, &b);
        let before = session.vehicle.clone();
        let message = session.preview_transform(&before, 0, 0.25, false);
        assert!(message.contains("2 part(s)"), "{message}");
        assert!((offset(&session, &a) - (before_a + 0.25)).abs() < 1e-9);
        assert!((offset(&session, &b) - (before_b + 0.25)).abs() < 1e-9);
    }

    #[test]
    fn symmetry_modes_link_propagate_and_break_the_set() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.symmetry = 2;
        session.symmetry_kind = SymmetryKind::Bilateral;
        session.add_at(
            PartTemplate::FinSet,
            Some(("motor-1".into(), omega_rules::NodeId::Radial(0), 0.0)),
            &owned,
        );
        let selected = session.selected.clone().unwrap();
        let members = session.vehicle.symmetry_members(&selected);
        assert_eq!(members.len(), 2, "a bilateral drop creates two copies");
        assert_eq!(
            session.vehicle.symmetry_kind_of(&selected),
            Some(SymmetryKind::Bilateral)
        );
        let before = session.vehicle.part(&selected).unwrap().spec;
        session.apply(MissileAction::Adjust(Param::Span, 1), &owned);
        let after = session.vehicle.part(&selected).unwrap().spec;
        assert_ne!(before, after);
        for member in &members {
            assert_eq!(session.vehicle.part(member).unwrap().spec, after, "{member}");
        }

        session.apply(MissileAction::BreakSymmetry, &owned);
        assert!(session.vehicle.part(&selected).unwrap().symmetry.is_none());
        let other = members.iter().find(|member| *member != &selected).unwrap();
        assert_eq!(session.vehicle.symmetry_members(other).len(), 1);
        let other_spec = session.vehicle.part(other).unwrap().spec;
        session.apply(MissileAction::Select { id: other.clone() }, &owned);
        session.apply(MissileAction::Adjust(Param::Span, 1), &owned);
        assert_ne!(session.vehicle.part(other).unwrap().spec, other_spec);
        assert_eq!(
            session.vehicle.part(&selected).unwrap().spec,
            after,
            "broken link stops propagation"
        );
    }

    #[test]
    fn selected_parts_can_be_reassociated_into_a_symmetry_set() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.add_at(
            PartTemplate::FinSet,
            Some(("motor-1".into(), omega_rules::NodeId::Radial(0), 0.0)),
            &owned,
        );
        let a = session.selected.clone().unwrap();
        session.add_at(
            PartTemplate::FinSet,
            Some(("motor-1".into(), omega_rules::NodeId::Radial(2), 0.0)),
            &owned,
        );
        let b = session.selected.clone().unwrap();
        session.apply(MissileAction::Select { id: a.clone() }, &owned);
        session.apply(MissileAction::ToggleSelect { id: b.clone() }, &owned);
        session.symmetry_kind = SymmetryKind::Axial;
        let message = session.apply(MissileAction::LinkSymmetry, &owned);
        assert!(message.contains("liées"), "{message}");
        assert_eq!(session.vehicle.symmetry_kind_of(&a), Some(SymmetryKind::Axial));
        assert_eq!(session.vehicle.symmetry_members(&a).len(), 2);
        assert!(session.vehicle.symmetry_members(&a).contains(&b));
    }

    #[test]
    fn a_symmetry_transform_reaches_its_mirrored_copy() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.symmetry = 2;
        session.add_at(
            PartTemplate::FinSet,
            Some(("motor-1".into(), omega_rules::NodeId::Radial(0), 0.0)),
            &owned,
        );
        let selected = session.selected.clone().unwrap();
        let members = session.vehicle.symmetry_members(&selected);
        assert_eq!(members.len(), 2);
        session
            .set_transform_values(&selected, [0.5, 0.0, 0.0, 0.0, 0.0, 0.0])
            .unwrap();
        let offsets: Vec<f64> = members
            .iter()
            .map(|member| {
                session
                    .vehicle
                    .part(member)
                    .unwrap()
                    .attach
                    .as_ref()
                    .unwrap()
                    .offset_x_m
            })
            .collect();
        assert!(offsets.contains(&0.5), "{offsets:?}");
        assert!(offsets.contains(&-0.5), "{offsets:?}");
    }

    #[test]
    fn duplicating_a_subtree_preserves_its_inner_edges() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.apply(MissileAction::Select { id: "motor-3".into() }, &owned);
        let before = session.vehicle.parts.len();
        let message = session.apply(MissileAction::DuplicateSelection, &owned);
        assert!(message.contains("dupliqué"), "{message}");
        assert!(session.vehicle.parts.len() > before);
        let copy_root = session.selected.clone().unwrap();
        assert_ne!(copy_root, "motor-3");
        assert_eq!(
            session.vehicle.children(&copy_root).len(),
            1,
            "the cloned stack remains linked"
        );
        assert_eq!(session.vehicle.subtree_ids(&copy_root).len(), 4);
        session.apply(MissileAction::Undo, &owned);
        assert_eq!(session.vehicle.parts.len(), before);
    }

    #[test]
    fn the_subassembly_library_round_trips_and_reinserts_structure() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.apply(MissileAction::Select { id: "motor-3".into() }, &owned);
        let saved = session.apply(MissileAction::SaveSubassembly(SubassemblyKind::Stage), &owned);
        assert!(saved.contains("enregistré"), "{saved}");
        assert_eq!(session.library.len(), 1);
        let bytes = session.library_bytes().unwrap();
        let entries = subassembly::parse_library(&bytes).unwrap();

        let mut other = MissileSession {
            library: entries,
            ..Default::default()
        };
        let inserted = other.apply_subassembly(0, &owned);
        assert!(inserted.contains("inséré"), "{inserted}");
        assert_eq!(other.vehicle.parts.len(), 4);
        let root = other.vehicle.root_id().unwrap().to_string();
        assert_eq!(other.vehicle.children(&root).len(), 1);
        assert_eq!(other.vehicle.subtree_ids(&root).len(), 4);
        assert!(other.vehicle.validate().is_ok(), "{:?}", other.vehicle.validate());
    }

    #[test]
    fn the_assembly_report_finds_loose_parts_and_locks() {
        let mut session = MissileSession::new_default();
        session.vehicle.detach("decoupler-2").unwrap();
        session.locked.insert("motor-1".into());
        let report = session.assembly_report();
        assert!(report.iter().any(|issue| {
            issue.kind == AssemblyIssueKind::Unconnected && issue.part.as_deref() == Some("decoupler-2")
        }));
        assert!(
            report
                .iter()
                .any(|issue| { issue.kind == AssemblyIssueKind::Locked && issue.part.as_deref() == Some("motor-1") })
        );
    }
}
