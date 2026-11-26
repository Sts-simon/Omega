//! Balance audit (0.46): one deterministic place to inspect the trade-offs between part mass,
//! fictional cost, electrical/propellant consumption, complexity, reliability and the abstract
//! contract economy. The values are gameplay numbers; this module does not model engineering.
//!
//! `report()` is printed by `omega --balance-report` and committed as `docs/BALANCE.md`. It is a
//! presentation layer over the same `stats`, avionics and contract functions the game uses, so a
//! tuning change cannot silently disagree with the document.

use crate::aero;
use crate::avionics::{self, AvionicsModule};
use crate::contracts::{ContractOffer, generate_contract_board};
use crate::research::default_tree;
use crate::vehicle::{
    AdapterShape, AirfoilProfile, ComputerClass, ControlKind, ExplosiveGrade, FinLayout, NoseShape, PartSpec,
    RadarClass, SensorKind, StabilizerRole, WingPlanform,
};
use std::fmt::Write as _;

/// One part's abstract balance sheet. Mass is dry mass; propellant is the fuel a tank or motor
/// carries; cost includes the mass-derived airframe cost and any electronics cost.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PartBalance {
    pub dry_mass_kg: f64,
    pub propellant_kg: f64,
    pub cost_cr: f64,
    pub power_w: f64,
    pub complexity: f64,
    pub reliability: f64,
}

impl PartBalance {
    pub fn wet_mass_kg(self) -> f64 {
        self.dry_mass_kg + self.propellant_kg
    }
}

/// Human label used by the audit table. The workshop has its own richer French labels; this is
/// deliberately stable and language-neutral so generated reports and tests are easy to diff.
pub fn label(spec: &PartSpec) -> &'static str {
    use PartSpec::*;
    match spec {
        SolidMotor { .. } => "Solid motor",
        FuelTank { .. } => "Fuel tank",
        LiquidEngine { .. } => "Liquid engine",
        AirBreathing { .. } => "Turbofan",
        Sensor { kind } => match kind {
            SensorKind::Pitot => "Pitot",
            SensorKind::Barometer => "Barometer",
            SensorKind::Radar(RadarClass::Basic) => "Radar basic",
            SensorKind::Radar(RadarClass::Active) => "Radar active",
        },
        Computer { class } => match class {
            ComputerClass::Basic => "Computer 1",
            ComputerClass::Standard => "Computer 2",
            ComputerClass::Advanced => "Computer 3",
        },
        Decoupler => "Decoupler",
        MissionModule { .. } => "Mission module",
        Explosive { grade, .. } => match grade {
            ExplosiveGrade::Standard => "Charge standard",
            ExplosiveGrade::Improved => "Charge improved",
            ExplosiveGrade::Advanced => "Charge advanced",
        },
        Adapter { .. } => "Adapter",
        NoseCone { .. } => "Nose cone",
        Wing { light, .. } => {
            if *light {
                "Wing light"
            } else {
                "Wing"
            }
        }
        FinSet { light, .. } => {
            if *light {
                "Fin set light"
            } else {
                "Fin set"
            }
        }
        ControlSurface { kind, light, .. } => match (kind, light) {
            (ControlKind::Fixed, _) => "Control fixed",
            (_, true) => "Control light",
            _ => "Control moving",
        },
        Avionics { module } => match module {
            AvionicsModule::ControlAugment => "Control augment",
            AvionicsModule::InertialNav => "Inertial platform",
            AvionicsModule::Datalink => "Datalink",
            AvionicsModule::PowerCell => "Power cell",
            AvionicsModule::Diagnostics => "Self-test",
        },
    }
}

/// Compute the part-level figures. The global `DesignMetrics` remains the delivery authority;
/// this view explains where a part contributes before the whole design is assembled.
pub fn part_balance(spec: &PartSpec) -> PartBalance {
    let stats = spec.stats();
    let electronics = avionics::spec_of(spec);
    let effect = aero::surface_effect(spec);
    let aero_mass = aero::surface_mass_kg(spec).unwrap_or(0.0);
    let dry_mass = if aero_mass > 0.0 { aero_mass } else { stats.dry_mass_kg };
    let propellant = stats.propellant_kg;
    let surface_cost = if effect.is_some() { 850.0 } else { 0.0 };
    let electronics_cost = electronics.map_or(0.0, |figure| figure.cost_cr as f64);
    let cost = electronics_cost + (dry_mass + propellant) * 18.0 + surface_cost;
    let power = electronics.map_or(0.0, |figure| figure.power_w);
    // The light-structure choice is a build modifier rather than a JSON effect: mirror the
    // vehicle-level penalty here so the inspector does not claim a free mass saving.
    let light = matches!(
        spec,
        PartSpec::Wing { light: true, .. }
            | PartSpec::FinSet { light: true, .. }
            | PartSpec::ControlSurface { light: true, .. }
    );
    let complexity = effect.map_or(0.0, |table| table.complexity)
        + electronics.map_or(0.0, |figure| figure.complexity)
        + if light { 0.10 } else { 0.0 };
    let reliability = effect.map_or(0.0, |table| table.reliability)
        + electronics.map_or(0.0, |figure| figure.reliability)
        - if light { 0.05 } else { 0.0 };
    PartBalance {
        dry_mass_kg: dry_mass,
        propellant_kg: propellant,
        cost_cr: cost,
        power_w: power,
        complexity,
        reliability,
    }
}

/// The concise benefit/limitation pair shown in the inspector and the generated audit.
pub fn tradeoff(spec: &PartSpec) -> (&'static str, &'static str) {
    use PartSpec::*;
    match spec {
        SolidMotor { .. } => (
            "Poussée immédiate, sans réservoir ni alimentation.",
            "Pas de rallumage ; une combustion courte alourdit le boîtier.",
        ),
        FuelTank { .. } => (
            "Carburant dense pour les vols longs à moteur liquide ou aérobie.",
            "La masse humide est portée dès le lancement et exige un moteur adapté.",
        ),
        LiquidEngine { .. } => (
            "Poussée modulable et combustion longue.",
            "Exige un réservoir dans le même étage ; une grande échelle est lourde et coûteuse.",
        ),
        AirBreathing { .. } => (
            "Très bon rendement en croisière.",
            "Exige de la vitesse, des ailes et un profil de vol compatible.",
        ),
        Sensor { kind } => match kind {
            SensorKind::Pitot => (
                "Débloque les blocs de programme fondés sur la vitesse.",
                "Ajoute une petite masse et un coût.",
            ),
            SensorKind::Barometer => (
                "Débloque les blocs de pression et d'altitude.",
                "Ajoute une petite masse et un coût.",
            ),
            SensorKind::Radar(RadarClass::Basic) => (
                "Autorise un jet d'accrochage basique.",
                "Plus lourd qu'un pitot et utile seulement avec un ordinateur.",
            ),
            SensorKind::Radar(RadarClass::Active) => (
                "Meilleure probabilité d'accrochage que le radar basique.",
                "Nettement plus lourd, plus coûteux et moins fiable.",
            ),
        },
        Computer { class } => match class {
            ComputerClass::Basic => (
                "Mémoire et tâches au meilleur coût.",
                "Plus petite capacité de programme.",
            ),
            ComputerClass::Standard => (
                "Plus de mémoire et de tâches pour les modules avancés.",
                "Plus de masse et de coût.",
            ),
            ComputerClass::Advanced => (
                "Capacité maximale de mémoire et de tâches.",
                "Le plus lourd, le plus coûteux et le moins fiable.",
            ),
        },
        Decoupler => (
            "Crée un étage indépendant.",
            "Ajoute de la masse et une étape de séparation au programme.",
        ),
        MissionModule { .. } => (
            "Transporte la charge utile du contrat.",
            "Chaque kilogramme est de la masse au lancement.",
        ),
        Explosive { grade, .. } => match grade {
            ExplosiveGrade::Standard => ("Disponible dès le départ.", "Rendement par kilogramme le plus faible."),
            ExplosiveGrade::Improved => (
                "Meilleur rendement à masse de charge égale.",
                "Exige un projet de recherche.",
            ),
            ExplosiveGrade::Advanced => (
                "Meilleur rendement par kilogramme.",
                "Exige la recherche de charge la plus profonde.",
            ),
        },
        Adapter { .. } => (
            "Évite une marche de diamètre et garde la pile propre.",
            "Ajoute de la masse de coque sans poussée.",
        ),
        NoseCone { .. } => (
            "Ferme la cellule et définit son profil.",
            "Pièce terminale ; la forme ajoute de la masse de coque.",
        ),
        Wing { light, .. } => {
            if *light {
                (
                    "Surface portante plus légère.",
                    "Plus de complexité et moins de fiabilité.",
                )
            } else {
                (
                    "Crée de la portance et déplace le centre aérodynamique.",
                    "Ajoute de la traînée, de la masse et un compromis de stabilité.",
                )
            }
        }
        FinSet { light, .. } => {
            if *light {
                ("Stabilisateur plus léger.", "Plus de complexité et moins de fiabilité.")
            } else {
                (
                    "Recule le centre aérodynamique pour stabiliser.",
                    "Ajoute de la traînée sans portance utile.",
                )
            }
        }
        ControlSurface { kind, light, .. } => match (kind, light) {
            (ControlKind::Fixed, _) => (
                "Stabilise sans dépendre de l'ordinateur.",
                "Aucune autorité de commande.",
            ),
            (_, true) => (
                "Surface mobile légère.",
                "Exige un ordinateur capable et réduit la fiabilité.",
            ),
            _ => (
                "Ajoute de l'autorité de commande.",
                "Ajoute de la traînée, de la complexité et exige un ordinateur.",
            ),
        },
        Avionics { module } => match module {
            AvionicsModule::ControlAugment => (
                "Plus d'autorité de commande.",
                "Consomme de la puissance et réduit la fiabilité.",
            ),
            AvionicsModule::InertialNav => (
                "Améliore la visée et les solutions d'accrochage.",
                "Exige de la puissance et un ordinateur standard.",
            ),
            AvionicsModule::Datalink => (
                "Améliore encore l'accrochage.",
                "Exige la navigation et de la puissance.",
            ),
            AvionicsModule::PowerCell => (
                "Alimente le bus de mission.",
                "Module avionique le plus lourd ; ajoute de la complexité.",
            ),
            AvionicsModule::Diagnostics => (
                "Réduit l'exposition aux pannes.",
                "Consomme de la puissance et ajoute un peu de complexité.",
            ),
        },
    }
}

/// One representative of every part family, including the mutually exclusive build choices.
pub fn representative_parts() -> Vec<PartSpec> {
    vec![
        PartSpec::SolidMotor {
            length_m: 1.5,
            diameter_m: 0.4,
            burn_time_s: 15.0,
        },
        PartSpec::FuelTank {
            length_m: 1.5,
            diameter_m: 0.4,
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
        PartSpec::MissionModule { mass_kg: 40.0 },
        PartSpec::Explosive {
            mass_kg: 40.0,
            grade: ExplosiveGrade::Standard,
        },
        PartSpec::Explosive {
            mass_kg: 40.0,
            grade: ExplosiveGrade::Improved,
        },
        PartSpec::Explosive {
            mass_kg: 40.0,
            grade: ExplosiveGrade::Advanced,
        },
        PartSpec::Adapter {
            length_m: 0.4,
            diameter_m: 0.5,
            exit_diameter_m: 0.3,
            shape: AdapterShape::Conical,
        },
        PartSpec::NoseCone {
            length_m: 0.5,
            diameter_m: 0.4,
            shape: NoseShape::Ogive,
        },
        PartSpec::Wing {
            span_m: 1.0,
            chord_m: 0.3,
            sweep_deg: 15.0,
            planform: WingPlanform::Swept,
            profile: AirfoilProfile::Classic,
            light: false,
        },
        PartSpec::Wing {
            span_m: 1.0,
            chord_m: 0.3,
            sweep_deg: 15.0,
            planform: WingPlanform::Swept,
            profile: AirfoilProfile::Classic,
            light: true,
        },
        PartSpec::FinSet {
            span_m: 0.5,
            chord_m: 0.2,
            role: StabilizerRole::Rear,
            layout: FinLayout::Symmetric,
            light: false,
        },
        PartSpec::FinSet {
            span_m: 0.5,
            chord_m: 0.2,
            role: StabilizerRole::Rear,
            layout: FinLayout::Symmetric,
            light: true,
        },
        PartSpec::ControlSurface {
            span_m: 0.4,
            chord_m: 0.15,
            kind: ControlKind::Classic,
            light: false,
        },
        PartSpec::ControlSurface {
            span_m: 0.4,
            chord_m: 0.15,
            kind: ControlKind::Fixed,
            light: false,
        },
        PartSpec::Avionics {
            module: AvionicsModule::ControlAugment,
        },
        PartSpec::Avionics {
            module: AvionicsModule::InertialNav,
        },
        PartSpec::Avionics {
            module: AvionicsModule::Datalink,
        },
        PartSpec::Avionics {
            module: AvionicsModule::PowerCell,
        },
        PartSpec::Avionics {
            module: AvionicsModule::Diagnostics,
        },
    ]
}

fn contract_table(out: &mut String) {
    let offers = generate_contract_board(7, 1);
    out.push_str("\n## Contract offers (seed 7)\n\n");
    out.push_str("| # | Profile | Difficulty | Reward | Budget | Deadline | Primary goals |\n");
    out.push_str("|---:|---|---:|---:|---:|---:|---|\n");
    for offer in &offers {
        let goals: Vec<String> = offer.primary_goals.iter().map(|goal| goal.text()).collect();
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} |",
            offer.id,
            offer.profile_label(),
            offer.difficulty,
            offer.reward_credits,
            offer.design_budget_cr,
            offer.deadline_days,
            goals.join("; ")
        );
    }
    out.push_str(
        "\nDifficulty raises the reward and the design budget, tightens the deadline and raises the \
         reliability/complexity/mass goals. Secondary goals are optional and pay the stated bonus \
         only when all of them are met.\n",
    );
}

fn research_table(out: &mut String) {
    let tree = default_tree();
    out.push_str("\n## Research economy\n\n");
    out.push_str("| Department | Technologies | Credits | Days |\n");
    out.push_str("|---|---:|---:|---:|\n");
    for department in crate::Department::ALL {
        let nodes: Vec<_> = tree.iter().filter(|node| node.department == department).collect();
        let credits: u64 = nodes.iter().map(|node| node.cost_cr).sum();
        let days: u16 = nodes.iter().map(|node| node.duration_days).sum();
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            department.label(),
            nodes.len(),
            credits,
            days
        );
    }
    let credits: u64 = tree.iter().map(|node| node.cost_cr).sum();
    let days: u16 = tree.iter().map(|node| node.duration_days).sum();
    let _ = writeln!(
        out,
        "\nTotal: {} technologies, {} credits and {} research days. All prerequisite paths are \
         validated by `validate_tree`; every node grants a capability or a contract slot.\n",
        tree.len(),
        credits,
        days
    );
}

/// Full Markdown audit, used by the CLI and docs. The same table is covered by tests so a tuning
/// edit cannot leave the documentation stale without a signal.
pub fn report() -> String {
    let mut out = String::from(
        "# Omega balance audit\n\nGenerated by `omega --balance-report` from the same rules used \
         by the game. Every value is fictional and abstract.\n\n## Parts\n\n| Part | Dry mass | Propellant | Cost | Power | Complexity | Reliability | Advantage | Limitation |\n|---|---:|---:|---:|---:|---:|---:|---|---|\n",
    );
    for spec in representative_parts() {
        let balance = part_balance(&spec);
        let (advantage, limitation) = tradeoff(&spec);
        let _ = writeln!(
            out,
            "| {} | {:.1} kg | {:.1} kg | {:.0} cr | {:+.0} W | {:.2} | {:+.2} | {} | {} |",
            label(&spec),
            balance.dry_mass_kg,
            balance.propellant_kg,
            balance.cost_cr,
            balance.power_w,
            balance.complexity,
            balance.reliability,
            advantage,
            limitation
        );
    }
    contract_table(&mut out);
    research_table(&mut out);
    out.push_str(
        "\n## Reading the table\n\n- A larger dry mass raises launch mass and the fictional cost.\n\
         - Propellant is wet mass but is consumed during flight; tanks and motors are the only parts \
         that carry it.\n- Positive power draws from the mission bus; a negative figure supplies it.\n\
         - Complexity raises the delivery pressure and lowers aerodynamic reliability; the reliability \
         column is a relative contribution, not a percentage chance by itself.\n",
    );
    out
}

/// A compact contract-only summary used by balancing tests and the dashboard.
pub fn contract_offer_balance(offer: &ContractOffer) -> (u64, u64, u16, usize) {
    (
        offer.reward_credits,
        offer.design_budget_cr,
        offer.deadline_days,
        offer.primary_goals.len(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_representative_part_has_mass_cost_and_a_real_tradeoff() {
        for spec in representative_parts() {
            let balance = part_balance(&spec);
            assert!(balance.dry_mass_kg >= 0.0, "{}", label(&spec));
            assert!(balance.cost_cr > 0.0, "{}", label(&spec));
            let (advantage, limitation) = tradeoff(&spec);
            assert!(!advantage.is_empty(), "{}", label(&spec));
            assert!(!limitation.is_empty(), "{}", label(&spec));
        }
        let report = report();
        assert!(report.contains("## Parts"));
        assert!(report.contains("## Contract offers"));
        assert!(report.contains("## Research economy"));
    }

    #[test]
    fn light_surfaces_trade_mass_for_reliability() {
        let normal = representative_parts()
            .into_iter()
            .find(|spec| matches!(spec, PartSpec::Wing { light: false, .. }))
            .unwrap();
        let light = representative_parts()
            .into_iter()
            .find(|spec| matches!(spec, PartSpec::Wing { light: true, .. }))
            .unwrap();
        let normal = part_balance(&normal);
        let light = part_balance(&light);
        assert!(light.dry_mass_kg < normal.dry_mass_kg);
        assert!(light.reliability < normal.reliability);
        assert!(light.complexity > normal.complexity);
    }

    #[test]
    fn electronics_and_power_have_coherent_costs() {
        let basic = part_balance(&PartSpec::Computer {
            class: ComputerClass::Basic,
        });
        let advanced = part_balance(&PartSpec::Computer {
            class: ComputerClass::Advanced,
        });
        assert!(advanced.cost_cr > basic.cost_cr);
        assert!(advanced.reliability < basic.reliability);
        let cell = part_balance(&PartSpec::Avionics {
            module: AvionicsModule::PowerCell,
        });
        assert!(cell.power_w < 0.0, "a power cell supplies the bus");
        let augment = part_balance(&PartSpec::Avionics {
            module: AvionicsModule::ControlAugment,
        });
        assert!(augment.power_w > 0.0);
        assert!(augment.cost_cr > 1_000.0);
    }

    #[test]
    fn contract_balance_is_available_from_the_shared_offer() {
        let offer = generate_contract_board(7, 1).remove(0);
        let (reward, budget, deadline, goals) = contract_offer_balance(&offer);
        assert!(reward > 0 && budget > 0 && deadline >= 12);
        assert_eq!(goals, offer.primary_goals.len());
    }
}
