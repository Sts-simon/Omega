//! Shared look and small widgets for the Omega interface: palette, cards, status chips and
//! buttons whose colour depends on a tone and on hover/press state.

/// Bevy 0.20.1 deprecates `Interaction` in favour of `picking::hover::Hovered` and `ui::Pressed`,
/// but the `bevy_ui` focus system in this release still only drives `Interaction`: nothing in the
/// engine maintains the replacement components for UI nodes yet. Re-exporting the deprecated type
/// here keeps the migration to a single place, and `expect` fails the build once a Bevy upgrade
/// makes the alias unnecessary, so the cleanup cannot be forgotten.
#[expect(
    deprecated,
    reason = "bevy_ui 0.20.1 focus only maintains Interaction; Hovered/Pressed are not wired for UI nodes yet"
)]
pub(crate) type UiInteraction = bevy::ui::Interaction;

use crate::interface::{UiAction, UiActionButton};
use bevy::prelude::*;

pub const PANEL: Color = Color::srgba(0.016, 0.030, 0.044, 0.93);
pub const CARD: Color = Color::srgba(0.050, 0.078, 0.104, 0.97);
pub const CARD_SELECTED: Color = Color::srgba(0.060, 0.120, 0.150, 0.98);
pub const TEXT: Color = Color::srgb(0.92, 0.95, 0.97);
pub const TEXT_DIM: Color = Color::srgb(0.58, 0.67, 0.73);
pub const ACCENT: Color = Color::srgb(0.36, 0.84, 0.94);
pub const GOOD: Color = Color::srgb(0.22, 0.66, 0.42);
pub const WARN: Color = Color::srgb(0.88, 0.64, 0.18);
pub const BAD: Color = Color::srgb(0.82, 0.30, 0.30);
pub const NEUTRAL: Color = Color::srgb(0.30, 0.40, 0.52);
pub const PURPLE: Color = Color::srgb(0.58, 0.46, 0.86);

#[derive(Component)]
pub struct NavStatus;

/// Visual weight of a button.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonTone {
    Secondary,
    Primary,
    Danger,
    Ghost,
    Disabled,
}

impl ButtonTone {
    pub fn background(self, interaction: UiInteraction) -> Color {
        let (idle, hover, pressed) = match self {
            Self::Secondary => ((0.110, 0.170, 0.220), (0.170, 0.270, 0.330), (0.120, 0.400, 0.350)),
            Self::Primary => ((0.100, 0.440, 0.520), (0.130, 0.560, 0.640), (0.170, 0.680, 0.560)),
            Self::Danger => ((0.380, 0.130, 0.130), (0.500, 0.170, 0.170), (0.620, 0.220, 0.220)),
            Self::Ghost => ((0.040, 0.065, 0.088), (0.100, 0.165, 0.210), (0.120, 0.300, 0.330)),
            Self::Disabled => ((0.060, 0.080, 0.095), (0.060, 0.080, 0.095), (0.060, 0.080, 0.095)),
        };
        let (r, g, b) = match interaction {
            UiInteraction::None => idle,
            UiInteraction::Hovered => hover,
            UiInteraction::Pressed => pressed,
        };
        Color::srgb(r, g, b)
    }
}

/// Traduction française des textes visibles. Les identifiants internes et les formats de sauvegarde restent inchangés.
pub fn fr(text: impl Into<String>) -> String {
    let text = text.into();
    let exact = match text.as_str() {
        "ISLAND" => "ÎLE",
        "CONTRACTS" => "CONTRATS",
        "WORKSHOP" => "ATELIER",
        "RESEARCH" => "RECHERCHE",
        "PROGRAM" => "PROGRAMME",
        "TRIALS" => "ESSAIS",
        "RESEARCH ISLAND" => "ÎLE DE RECHERCHE",
        "CONTRACT OFFICE" => "BUREAU DES CONTRATS",
        "MISSILE WORKSHOP" => "ATELIER DE CONCEPTION",
        "RESEARCH CENTRE" => "CENTRE DE RECHERCHE",
        "MISSILE PROGRAM" => "PROGRAMMATION EMBARQUÉE",
        "MISSILE PROCUREMENT" => "BUREAU DES CONTRATS MILITAIRES",
        "START FROM" => "MODÈLE DE DÉPART",
        "ROCKET" => "BALISTIQUE",
        "CRUISE" => "CROISIÈRE",
        "FINS" => "AILETTES",
        "WINGS" => "AILES",
        "EMPTY" => "VIDE",
        "ALL" => "TOUT",
        "AIRFRAME & PAYLOAD" => "STRUCTURE ET CHARGE",
        "AERODYNAMIC SURFACES" => "SURFACES AÉRODYNAMIQUES",
        "SENSORS" => "CAPTEURS",
        "AVIONICS" => "AVIONIQUE",
        "SOLID MOTOR" => "MOTEUR SOLIDE",
        "FUEL TANK" => "RÉSERVOIR",
        "LIQUID ENGINE" => "MOTEUR LIQUIDE",
        "TURBOFAN" => "PROPULSEUR À AIR",
        "PITOT TUBE" => "SONDE DE PRESSION",
        "BAROMETER" => "BAROMÈTRE",
        "RADAR BASIC" => "RADAR SIMPLE",
        "RADAR ACTIVE" => "RADAR ACTIF",
        "COMPUTER 1" => "ORDINATEUR 1",
        "COMPUTER 2" => "ORDINATEUR 2",
        "COMPUTER 3" => "ORDINATEUR 3",
        "DECOUPLER" => "SÉPARATEUR",
        "MISSION MODULE" => "MODULE DE MISSION",
        "WING" => "AILE",
        "FIN SET" => "JEU D'AILETTES",
        "CONTROL" => "COMMANDE",
        "Length (m)" => "Longueur (m)",
        "Diameter (m)" => "Diamètre (m)",
        "Burn time (s)" => "Durée de combustion (s)",
        "Engine scale" => "Échelle du moteur",
        "Mass (kg)" => "Masse (kg)",
        "Span (m)" => "Envergure (m)",
        "Chord (m)" => "Corde (m)",
        "Sweep (deg)" => "Flèche (°)",
        "PLANFORM" => "FORME EN PLAN",
        "PROFILE" => "PROFIL",
        "STABILISER" => "RÔLE DES AILETTES",
        "LAYOUT" => "CONFIGURATION",
        "SURFACE" => "SURFACE",
        "BUILD" => "CONSTRUCTION",
        "LIGHT" => "LÉGÈRE",
        "STANDARD" => "STANDARD",
        "SYMMETRY" => "SYMMÉTRIE",
        "UNDO" => "ANNULER",
        "REDO" => "RÉTABLIR",
        "REVISIONS" => "VERSIONS",
        "RECORD REVISION" => "ENREGISTRER UNE VERSION",
        "AERODYNAMICS" => "AÉRODYNAMIQUE",
        "CLEAR COMPARISON" => "EFFACER LA COMPARAISON",
        "SNAPSHOT A" => "ENREGISTRER A",
        "STABILITY" => "STABILITÉ",
        "LIFT" => "PORTANCE",
        "DRAG" => "TRAÎNÉE",
        "COMPLEXITY" => "COMPLEXITÉ",
        "RELIABILITY" => "FIABILITÉ",
        "AERO MASS" => "MASSE AÉRODYNAMIQUE",
        "STABLE BAND" => "ZONE STABLE",
        "UNSTABLE: CA AHEAD OF CG" => "INSTABLE : CA DEVANT LE CG",
        "MARGINAL" => "MARGINAL",
        "CONTRACT CHECK" => "CONFORMITÉ AU CONTRAT",
        "STAGE FIGURES" => "BILAN DES ÉTAGES",
        "ASSEMBLY TREE" => "ARBRE D'ASSEMBLAGE",
        "SELECTED PART" => "PIÈCE SÉLECTIONNÉE",
        "VALIDATION" => "VALIDATION",
        "DESIGN VALID" => "CONCEPTION VALIDE",
        "1 ERROR" => "1 ERREUR",
        "1 WARNING" => "1 AVERTISSEMENT",
        "No errors and no warnings." => "Aucune erreur ni aucun avertissement.",
        "REMOVE PART" => "SUPPRIMER LA PIÈCE",
        "SELECT" => "SÉLECTIONNER",
        "GO TO TRIALS" => "OUVRIR LES ESSAIS",
        "RE-FLY" => "RELANCER L'ESSAI",
        "DELIVER" => "LIVRER",
        "ACCEPT" => "ACCEPTER",
        "NEXT DAY" => "JOUR SUIVANT",
        "START RESEARCH" => "LANCER LA RECHERCHE",
        "RESEARCHED" => "DÉBLOQUÉ",
        "LOCKED" => "VERROUILLÉ",
        "LAB BUSY" => "LABORATOIRE OCCUPÉ",
        "NO CONTRACT" => "AUCUN CONTRAT",
        "TRIAL REQUIRED" => "ESSAI REQUIS",
        "EVIDENCE CURRENT" => "PREUVES À JOUR",
        "SELECTED" => "SÉLECTIONNÉ",
        "DEADLINE PASSED" => "ÉCHÉANCE DÉPASSÉE",
        "DOSSIER READY" => "DOSSIER PRÊT",
        "DOSSIER INCOMPLETE" => "DOSSIER INCOMPLET",
        "OFFERS" => "OFFRES",
        "ACTIVE CONTRACTS" => "CONTRATS ACTIFS",
        "DOSSIER" => "DOSSIER",
        "DELIVERED" => "LIVRÉS",
        "BLOCK LIBRARY" => "BIBLIOTHÈQUE DE BLOCS",
        "CLEAR PROGRAM" => "EFFACER LE PROGRAMME",
        "THREADS" => "TÂCHES",
        "+ THREAD" => "+ TÂCHE",
        "- THREAD" => "- TÂCHE",
        "SELECTED BLOCK" => "BLOC SÉLECTIONNÉ",
        "MOVE UP" => "MONTER",
        "MOVE DOWN" => "DESCENDRE",
        "REMOVE" => "SUPPRIMER",
        "FLIP VALUE" => "INVERSER LA VALEUR",
        "DUPLICATE" => "DUPLIQUER",
        "FLIGHT COMPUTER" => "ORDINATEUR DE BORD",
        "PROGRAM VALID" => "PROGRAMME VALIDE",
        "SAVED PROGRAMS" => "PROGRAMMES ENREGISTRÉS",
        "SAVE COPY" => "ENREGISTRER UNE COPIE",
        "DEBUGGER" => "DÉBOGUEUR",
        "RUN TRACE" => "LANCER LE TRAÇAGE",
        "TRIAL SETUP" => "CONFIGURATION DE L'ESSAI",
        "MISSION PROFILE" => "PROFIL DE MISSION",
        "AIR LAUNCH 200 m/s" => "LANCEMENT AÉRIEN 200 m/s",
        "VERTICAL LAUNCH" => "LANCEMENT VERTICAL",
        "SEED" => "GRAINE ALÉATOIRE",
        "LOCK" => "VERROUILLAGE",
        "NO LOCK" => "AUCUN VERROUILLAGE",
        "RUN TRIAL" => "LANCER L'ESSAI",
        "MONTE CARLO BATCH" => "SÉRIE MONTE-CARLO",
        "COMPARE" => "COMPARER",
        "INPUT HASH" => "EMPREINTE DES ENTRÉES",
        "TRAJECTORY" => "TRAJECTOIRE",
        "SPEED" => "VITESSE",
        "ALTITUDE" => "ALTITUDE",
        "MASS" => "MASSE",
        "THRUST" => "POUSSÉE",
        "METRICS" => "INDICATEURS",
        "Camera: right drag to orbit | middle drag to pan | wheel to zoom" => {
            "Caméra : clic droit + glisser pour tourner | clic molette + glisser pour déplacer | molette pour zoomer"
        }
        "This part has no adjustable setting." => "Cette pièce ne possède aucun réglage.",
        "Click a part in the tree above to inspect, resize or remove it." => {
            "Sélectionnez une pièce dans l'arbre pour l'inspecter, la régler ou la supprimer."
        }
        "No parts yet: add one from the palette." => "Aucune pièce : ajoutez-en une depuis la palette.",
        "root" => "racine",
        "root part" => "pièce racine",
        "No flight computer: add one to be able to program the vehicle." => {
            "Aucun ordinateur de bord : ajoutez-en un pour programmer le véhicule."
        }
        "No flight computer: add one in the missile workshop to program this vehicle." => {
            "Aucun ordinateur de bord : ajoutez-en un dans l'atelier pour programmer ce véhicule."
        }
        "No active contract. Accept one in the CONTRACT OFFICE to see its requirements here." => {
            "Aucun contrat actif. Acceptez une offre au bureau des contrats pour afficher ses exigences ici."
        }
        "No active contract. Accept an offer to open a dossier." => {
            "Aucun contrat actif. Acceptez une offre pour ouvrir un dossier."
        }
        "No aerodynamic surfaces. Drag a wing, fin set or control surface onto a radial mount." => {
            "Aucune surface aérodynamique. Faites glisser une aile ou des ailettes vers un point d'attache radial."
        }
        "Depth-first from the root. A decoupler starts a new stage." => {
            "Arbre hiérarchique depuis la racine. Un séparateur ouvre un nouvel étage."
        }
        "Drag a part onto a glowing node. Wings, fins and control surfaces need a free radial mount (X cycles symmetry). Delete removes, Ctrl+Z undoes." => {
            "Glissez une pièce vers un point d'attache lumineux dans la vue 3D. Les surfaces se montent sur un point radial libre. X change la symétrie ; Suppr supprime ; Ctrl+Z annule ; Ctrl+Y rétablit."
        }
        "A fictional missile programme: take a contract, design the vehicle, fly its trial, deliver the plan." => {
            "Programme militaire fictif : acceptez un contrat, concevez un véhicule, réalisez un essai puis livrez le dossier."
        }
        "Build a staged vehicle from resizable parts: tanks, motors, engines, sensors and a computer." => {
            "Assemblez un véhicule par étages à partir de pièces réglables, de capteurs et d'équipements."
        }
        "Spend credits and days to unlock engines, sensors, computers and missile families." => {
            "Dépensez des crédits et du temps pour débloquer des pièces et des familles de véhicules."
        }
        "Compose the onboard program from the blocks the installed hardware unlocks." => {
            "Composez le programme embarqué avec les blocs autorisés par les équipements installés."
        }
        "Client offers, requirements, deadlines and payment. Deliver a verified dossier." => {
            "Offres, exigences, échéances et rémunérations. Livrez un dossier vérifié."
        }
        "Fly the vehicle and its program: four plots, events, anomalies, seeded batches and A/B comparison." => {
            "Évaluez le véhicule et son programme : graphiques, événements, anomalies et comparaisons."
        }
        "The lab is free. Start a technology, then press NEXT DAY to let time pass." => {
            "Le laboratoire est disponible. Lancez une recherche, puis faites avancer le temps."
        }
        "No trial yet. Press RUN TRIAL to fly the current vehicle and program." => {
            "Aucun essai. Lancez un essai pour évaluer le véhicule et son programme."
        }
        "Run a trial to see figures." => "Lancez un essai pour afficher les résultats.",
        "No trial yet. Fly the vehicle and program in TRIALS to record evidence." => {
            "Aucun essai. Évaluez le véhicule dans l'onglet Essais pour enregistrer des preuves."
        }
        "None yet." => "Aucun pour le moment.",
        "None. Accept an offer above; the trials you fly afterwards become its evidence." => {
            "Aucun contrat. Acceptez une offre ; vos prochains essais serviront de preuves."
        }
        "The program is empty: add blocks to a thread." => "Le programme est vide : ajoutez des blocs à une tâche.",
        "This vehicle has no flight computer. Add one in the workshop first." => {
            "Ce véhicule n'a pas d'ordinateur de bord. Ajoutez-en un dans l'atelier."
        }
        "Program cleared" => "Programme effacé",
        "Block removed" => "Bloc supprimé",
        "Nothing to undo" => "Aucune action à annuler",
        "Nothing to redo" => "Aucune action à rétablir",
        "Comparison cleared" => "Comparaison effacée",
        "Select a part first" => "Sélectionnez d'abord une pièce",
        "Select a part to delete" => "Sélectionnez une pièce à supprimer",
        "Select a block first." => "Sélectionnez d'abord un bloc.",
        "Snapshot A first" => "Enregistrez d'abord le cliché A",
        "A/B compared" => "Comparaison A/B effectuée",
        "Batch started" => "Série lancée",
        "Batch already running." => "Une série est déjà en cours.",
        "Research grant required." => "Une technologie est nécessaire.",
        "Trial done" => "Essai terminé",
        _ => "",
    };
    if !exact.is_empty() {
        return exact.to_string();
    }
    let mut translated = text;
    for (from, to) in [
        ("STAGE ", "ÉTAGE "),
        ("Stage ", "Étage "),
        ("DAY ", "JOUR "),
        ("Day ", "Jour "),
        ("Total mass ", "Masse totale "),
        ("Mass ", "Masse "),
        ("Needs research: ", "Recherche nécessaire : "),
        ("Locked: research ", "Verrouillé : recherchez "),
        ("Preset locked: research ", "Modèle verrouillé : recherchez "),
        ("Loaded preset: ", "Modèle chargé : "),
        ("Added ", "Ajouté : "),
        ("Removed ", "Supprimé : "),
        ("Moved ", "Déplacé : "),
        ("to stage ", "vers l'étage "),
        ("to thread ", "dans la tâche "),
        ("No such ", "Aucun élément : "),
        (" | due day ", " | échéance jour "),
        (" active contracts", " contrats actifs"),
        (" delivered", " livrés"),
        (" cr", " crédits"),
        (" day(s)", " jour(s)"),
        (" thread(s)", " tâche(s)"),
        (" block(s)", " bloc(s)"),
        (" device(s)", " équipement(s)"),
    ] {
        translated = translated.replace(from, to);
    }
    translated
}

pub fn label(parent: &mut ChildSpawnerCommands<'_>, text: impl Into<String>, size: f32, color: Color) {
    parent.spawn((
        Text::new(fr(text)),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    ));
}

/// Small rounded pill with a coloured background.
pub fn chip(parent: &mut ChildSpawnerCommands<'_>, text: impl Into<String>, color: Color) {
    parent
        .spawn((
            Node {
                padding: UiRect {
                    left: Val::Px(8.0),
                    right: Val::Px(8.0),
                    top: Val::Px(3.0),
                    bottom: Val::Px(3.0),
                },
                border_radius: BorderRadius::all(Val::Px(9.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(color),
        ))
        .with_children(|c| label(c, text, 10.0, Color::srgb(0.04, 0.06, 0.08)));
}

/// Card container: rounded, with a coloured top edge.
pub fn card_node(width: Val, accent: Color) -> (Node, BackgroundColor, BorderColor) {
    (
        Node {
            width,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(7.0),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect {
                top: Val::Px(3.0),
                ..default()
            },
            border_radius: BorderRadius::all(Val::Px(7.0)),
            ..default()
        },
        BackgroundColor(CARD),
        BorderColor::all(accent),
    )
}

pub fn button(
    parent: &mut ChildSpawnerCommands<'_>,
    text: impl Into<String>,
    action: UiAction,
    tone: ButtonTone,
    width: Val,
) {
    parent
        .spawn((
            UiInteraction::default(),
            bevy::ui::FocusPolicy::Block,
            Node {
                width,
                min_height: Val::Px(32.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect {
                    left: Val::Px(10.0),
                    right: Val::Px(10.0),
                    top: Val::Px(6.0),
                    bottom: Val::Px(6.0),
                },
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(tone.background(UiInteraction::None)),
            tone,
            UiActionButton(action),
        ))
        .with_children(|b| {
            let color = if tone == ButtonTone::Disabled { TEXT_DIM } else { TEXT };
            label(b, text, 11.0, color);
        });
}

/// Button that shares the width of its row equally with its siblings.
pub fn button_row_item(
    parent: &mut ChildSpawnerCommands<'_>,
    text: impl Into<String>,
    action: UiAction,
    tone: ButtonTone,
) {
    parent
        .spawn((
            UiInteraction::default(),
            bevy::ui::FocusPolicy::Block,
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                min_height: Val::Px(28.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect {
                    left: Val::Px(3.0),
                    right: Val::Px(3.0),
                    top: Val::Px(5.0),
                    bottom: Val::Px(5.0),
                },
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(tone.background(UiInteraction::None)),
            tone,
            UiActionButton(action),
        ))
        .with_children(|b| label(b, text, 10.0, TEXT));
}

/// `48085` -> `48 085`.
pub fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn french_translation_covers_workshop_and_navigation_labels() {
        assert_eq!(fr("WORKSHOP"), "ATELIER");
        assert_eq!(fr("ASSEMBLY TREE"), "ARBRE D'ASSEMBLAGE");
        assert_eq!(fr("REMOVE PART"), "SUPPRIMER LA PIÈCE");
    }

    #[test]
    fn unknown_identifiers_are_not_modified() {
        assert_eq!(fr("family:cruise"), "family:cruise");
    }

    #[test]
    fn thousands_groups_digits() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1 000");
        assert_eq!(thousands(48_085), "48 085");
        assert_eq!(thousands(1_234_567), "1 234 567");
    }

    #[test]
    fn hovering_and_pressing_change_the_colour() {
        for tone in [
            ButtonTone::Secondary,
            ButtonTone::Primary,
            ButtonTone::Danger,
            ButtonTone::Ghost,
        ] {
            assert_ne!(
                tone.background(UiInteraction::None),
                tone.background(UiInteraction::Hovered)
            );
            assert_ne!(
                tone.background(UiInteraction::Hovered),
                tone.background(UiInteraction::Pressed)
            );
        }
        assert_eq!(
            ButtonTone::Disabled.background(UiInteraction::None),
            ButtonTone::Disabled.background(UiInteraction::Pressed)
        );
    }
}
