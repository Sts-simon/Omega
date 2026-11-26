# Omega — Conception de véhicules militaires

<<<<<<< Updated upstream
**Version 0.39.0** · Jeu fictif développé en Rust et Bevy.
=======
**Version 0.47.0 (version candidate)** · Jeu fictif développé en Rust et Bevy.
>>>>>>> Stashed changes

[Documentation anglaise](README.md) · [Dépôt GitHub](https://github.com/Sts-simon/Omega)

## Présentation

Omega est un jeu de conception militaire fictif. Le joueur accepte des contrats, assemble un véhicule dans un atelier 3D, configure un programme embarqué abstrait, réalise des essais reproductibles puis remet un dossier technique.

Toutes les valeurs sont des paramètres de gameplay fictifs. Elles ne constituent pas des données d'ingénierie réelles. La charge explosive est un module abstrait : sa masse et son rendement sont des valeurs de jeu, sans modèle chimique ni modèle d'arme réel.

## Boucle de jeu

1. **Contrats** : consulter les offres, leurs exigences et leurs échéances.
2. **Atelier** : assembler les pièces dans une vue 3D, organiser les étages et vérifier la conception.
3. **Programme** : composer un programme visuel à partir des blocs compatibles avec les équipements installés.
4. **Essais** : lire le tableau de bord, comparer les conceptions, consulter l'historique et exporter les rapports.
5. **Livraison** : remettre les preuves liées à la conception et au programme.

## Atelier de construction

L'atelier reprend plusieurs principes d'ergonomie de KSP :

- Vue 3D centrale dédiée à la construction.
- Maquette détaillée : sections rectangulaires arrondies, joints de panneaux, tuyères en cloche, nacelles, ailes continues et coiffe choisie par le joueur, dans une palette militaire sobre.
- Les véhicules à réaction (missiles de croisière) s'éditent à l'horizontale, avec la caméra et les poignées alignées sur cette vue.
- Palette de pièces filtrable par catégorie : propulsion, structure, aérodynamique, capteurs et avionique.
- Glisser-déposer d'une pièce vers un point d'attache visible, avec fantôme transparent à taille
  réelle, accrochage automatique au point le plus proche et validation verte ou rouge.
- Déplacement d'une pièce avec tout son sous-ensemble, duplication d'un sous-arbre sur un point
  libre et bibliothèque de sous-ensembles (surfaces, électronique, structure, étage, configuration)
  avec export et import JSON.
- Symétrie radiale de 2 à 8 copies, symétrie bilatérale et symétrie axiale ; les réglages et les
  transformations se propagent aux copies, le lien peut être rompu ou réassocié.
- Panneau de contrôle de l'assemblage : pièces sans connexion, points occupés, incompatibilités,
  verrous et sous-ensembles mal connectés, chaque ligne sélectionnant la pièce concernée.
- Annulation et rétablissement.
- Arbre d'assemblage présentant les pièces par nom, point d'attache et étage.
- Inspecteur pour consulter et régler la pièce sélectionnée.
- Adaptateurs axiaux : longueur, diamètre d'entrée, diamètre de sortie et profil conique, ogival
  ou à épaulement. Ils absorbent le saut de diamètre entre deux corps et leur masse dépend du profil.
- Diamètre de montage réglable par pas de 5 cm pour les capteurs, ordinateurs, séparateurs, modules
  et avionique : une pièce à section fixe s'aligne sur un corps plus large sans adaptateur.
- Coiffe plaçable et réglable : longueur, diamètre de base et profil conique, ogival ou parabolique.
  Elle n'est plus générée automatiquement ; elle ferme le sommet de la pile et se retire pour
  ajouter une autre pièce au-dessus.
- Charges explosives : masse réglable, rendement abstrait et trois grades. Les grades amélioré et
  avancé se débloquent au centre de recherche et convertissent mieux la masse en rendement.
- **Sélection lisible et multiple.** Survol en cyan, sélection en ambre (rouge si verrouillée),
  `Ctrl+clic` pour ajouter ou retirer une pièce, `Échap` pour tout désélectionner, sélection depuis
  la 3D comme depuis l'arbre. Un groupe sélectionné se déplace d'un seul geste : le même delta
  local est appliqué à chaque racine indépendante, sans casser la hiérarchie.
- **Arbre outillé.** Plier/déplier, compter les pièces d'un groupe, le renommer, le sélectionner
  entièrement, le masquer temporairement (la 3D et le picking l'ignorent, les règles le comptent
  toujours) et le verrouiller. Un verrou couvre le sous-ensemble ; supprimer ou déplacer un
  ancêtre qui contient une pièce verrouillée est refusé.
- **Caméra complète.** `F` centre la sélection (plusieurs pièces incluses), `Début` cadre le
  véhicule, `1` à `5` donnent face, côté, dessus, arrière et isométrique, avec une transition
  douce. `V` bascule libre/orbitale et `[` / `]` règlent la vitesse. Les mêmes commandes existent
  en boutons dans la palette.
- **Palette pratique.** `/` ouvre la recherche par nom ou description, les catégories filtrent,
  `+`/`*` gère les favoris, les pièces récentes restent en tête, et le survol affiche un aperçu 3D
  tournant, le rôle de la pièce et la technologie encore nécessaire.
- **Aide contextuelle.** L'inspecteur donne le rôle de la pièce, les conséquences de ses réglages,
  l'incompatibilité à connaître et la recherche manquante, à côté des paramètres, des actions de
  verrouillage/masquage/groupe et du renommage.
- Panneaux de conformité, d'aérodynamique et de bilan des étages.

## Contrats et profils

Les offres ne sont plus seulement des seuils par famille : chaque famille peut proposer plusieurs
profils (frappe profonde, raid furtif, frappe de précision, supériorité aérienne, interception longue
portée, défense ponctuelle, défense de zone, frappe de théâtre et rase-mottes naval). Chaque profil a
son client, son brief, sa récompense, son échéance et ses exigences propres : portée, charge
explosive, rendement, apogée ou **discrétion**.

La discrétion est calculée à partir de l'altitude maximale de l'essai : un vol bas est discret, un vol
haut ne l'est pas. Un contrat à basse altitude exige donc un score de discrétion élevé, tandis qu'un
contrat à haute apogée exige d'atteindre une altitude minimale : un même profil de vol ne peut pas
prouver les deux. De plus, une preuve n'est acceptée que si la famille de vol du véhicule correspond
à celle du contrat, et un même appareil ne peut pas être livré à deux contrats, même renommé ou
recopié avec d'autres identifiants de pièces.

Depuis la 0.45, chaque offre porte aussi un contexte fictif, une famille de mission, une difficulté,
un budget de conception, une échéance, une récompense et une liste de sections de dossier demandées.
Les objectifs principaux combinent masse, fiabilité minimale et complexité maximale ; les objectifs
secondaires optionnels sont vérifiés à la livraison et ajoutent un bonus. Un dépassement de budget,
un objectif principal manqué ou un dossier incomplet refuse la remise sans supprimer le contrat.
Un dossier technique PDF est alors écrit dans `saves/slot_N/reports/` : vue générale, arbre
d'assemblage, équipements, paramètres, programme, campagnes d'essais compatibles, anomalies,
matrice des exigences et révision soumise.

La recherche présente maintenant chaque département comme un parcours de paliers avec les
dépendances, l'aperçu des pièces réellement débloquées et un tableau comparatif des technologies
disponibles. Dans l'atelier, les révisions peuvent être nommées, comparées à la tête, restaurées
(la restauration crée une nouvelle révision) et marquées comme soumises ; les essais associés sont
comptés par révision.

### Inspecteur de transformations

La pièce sélectionnée expose sa position locale en mètres et sa rotation locale en degrés. Cliquer
sur une valeur permet de la saisir au clavier : **Entrée** valide, **Échap** annule et une valeur
invalide, infinie ou hors limites est refusée sans modifier le véhicule. Les nombres décimaux et les
valeurs négatives sont acceptés.

Le sélecteur de précision règle le pas des boutons **−** et **+** : grossier (0,50 m / 15°), normal
(0,05 m / 5°), fin (0,01 m / 1°) ou très fin (0,001 m / 0,1°). Des boutons remettent un axe, toutes
les translations ou toutes les rotations à zéro, et **RESTAURER LA TRANSFORMATION INITIALE**
ramène le montage par défaut en une seule opération annulable. Un glissement du gizmo actualise
immédiatement les valeurs affichées et n'enregistre qu'une opération au relâchement ; une valeur
validée déplace le modèle 3D et le gizmo. La pièce racine reste verrouillée.

### Commandes de l'atelier

| Action | Commande |
|---|---|
| Ajouter une pièce | Glisser depuis la palette vers un point lumineux |
| Déplacer une pièce | Glisser une pièce existante vers un autre point |
| Sélectionner | Cliquer sur une pièce ou une ligne de l'arbre |
| Supprimer | Suppr |
| Changer le nombre de copies radiales | X |
| Changer le mode de symétrie | Maj + X |
| Rompre un lien de symétrie | Maj + K |
| Lier la sélection | Maj + J |
| Dupliquer le sous-ensemble | Maj + D |
| Annuler | Ctrl + Z |
| Rétablir | Ctrl + Y |
| Centrer la pièce sélectionnée | F |
| Cadrer tout le véhicule | Début (Home) |
| Tourner la caméra | Clic droit + glisser |
| Déplacer la caméra | Clic molette + glisser |
| Zoomer | Molette |
| Régler une pièce | Boutons − / + ou molette au-dessus de la pièce |
| Saisir une transformation | Cliquer une valeur, taper un nombre, Entrée |
| Annuler une saisie | Échap |
| Changer la précision | Grossier / Normal / Fin / Très fin dans l'inspecteur |

La touche **F** centre la caméra sur la pièce sélectionnée. **Début (Home)** recadre tout le véhicule, même après un déplacement important de la caméra.

Le filtre de catégories ne modifie pas la conception. Il sert uniquement à réduire la liste des pièces affichées.

## Interface

Les vues principales sont :
- **Île** : accès aux installations.
- **Contrats** : offres, exigences et dossiers.
- **Atelier** : construction 3D.
- **Recherche** : déblocage des technologies.
- **Programme** : canevas visuel de 21 blocs — déplacement libre, zoom et déplacement du canevas, connexions, sélection multiple, copier/coller, duplication, groupes, branches `SI/SINON`, `RÉPÉTER N`, `RÉPÉTER TANT QUE`, `WAIT UNTIL`, `BREAK`, routines réutilisables, débogueur pas-à-pas coloré et validation en temps réel.
- **Essais** : laboratoire à trois onglets — tableau de bord, comparaison A/B/C et historique des campagnes.

Les identifiants internes, les noms de fichiers et les formats de sauvegarde restent inchangés afin de préserver la compatibilité.

## Laboratoire d'essais

- **Tableau de bord.** Résumé du tir ou du lot, résultats par phase, courbes de vitesse, d'altitude, de masse et de poussée, événements importants, statistiques, conformité au contrat et anomalies classées.
- **Anomalies liées.** Une instabilité, une défaillance de composant, une ressource insuffisante, un programme incompatible ou une exigence contractuelle non satisfaite renvoie vers la pièce de l'atelier ou le bloc du programme concerné lorsque l'ancre est connue.
- **Comparaison.** L'instantané A, la conception courante B et la dernière campagne C sont confrontés sur la masse, le coût fictif, la stabilité, l'autorité, la complexité, la fiabilité, les résultats d'essais et les exigences du contrat. Chaque ligne donne la valeur absolue, l'écart B−A et la variation en pourcentage ; une base nulle affiche `—`.
- **Historique.** Chaque campagne conserve la révision de conception, la version du programme, les graines, le profil et les instantanés complets du véhicule et du programme dans `trials/history.json`. `RELANCER` rejoue exactement la campagne ; une campagne devient obsolète dès que la conception ou le programme courant diffère.
- **Exports.** Rapport HTML autonome avec graphiques SVG intégrés, export CSV des échantillons et dossier technique PDF avec courbe et résumé des modifications entre révisions, écrits dans `saves/slot_N/reports/` sans toucher à la campagne.

## Installation et compilation

Sous Linux Mint ou une distribution Linux compatible :

```bash
sudo apt update
sudo apt install build-essential pkg-config libx11-dev libasound2-dev libudev-dev \
  libxkbcommon-x11-0 libwayland-dev libxkbcommon-dev mesa-vulkan-drivers
```

Installer Rust stable avec rustup, puis :

```bash
cargo run
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Le jeu utilise Rust et Bevy 0.20.1. Une session graphique et un pilote Vulkan fonctionnel sont nécessaires.
Le guide détaillé (archive distribuable, SHA-256, dépannage Vulkan) est dans `docs/INSTALLATION.md`.

## Sauvegardes

L'écran principal gère six campagnes, de `saves/slot_1/` à `saves/slot_6/` : création, chargement, reprise et suppression. Le jeu attend qu'une campagne soit choisie avant de démarrer. La traduction et les améliorations de l'atelier ne modifient pas les schémas existants. Les conceptions, programmes, contrats et preuves conservent leurs identifiants et empreintes.

Chaque fichier est vérifié par empreinte et écrit atomiquement. En cas de fichier principal corrompu ou tronqué, le jeu restaure prudemment la génération précédente depuis son fichier `.bak` et le signale dans le journal ; sans copie valide, il refuse de charger le fichier au lieu de démarrer sur des données incohérentes. Une sauvegarde interrompue avant le manifeste laisse l'ancien état faire autorité.

## Équilibrage et contrôles sans fenêtre

`omega --balance-report` produit l'audit des pièces, des contrats et de la recherche, également disponible dans `docs/BALANCE.md`. Chaque pièce affiche sa masse, son carburant, son coût fictif, sa puissance, sa complexité, sa fiabilité, son avantage et sa limite ; les contrats sont contrôlés par difficulté, et les 22 technologies par département.

Les modes `--trial`, `--revisions`, `--aero-report`, `--export-design` et `--import-design` fonctionnent sans ouvrir de fenêtre. Ils servent aux vérifications en CI ou à distance.

## Périmètre

Omega reste un jeu militaire fictif. Il ne cherche pas à reproduire des calculs réels de guidage, de ciblage, de structure ou de propulsion. Les essais utilisent des modèles abstraits destinés au gameplay.
