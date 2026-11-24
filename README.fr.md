# Omega — Conception de véhicules militaires

**Version 0.37.0** · Jeu fictif développé en Rust et Bevy.

[Documentation anglaise](README.md) · [Dépôt GitHub](https://github.com/Sts-simon/Omega)

## Présentation

Omega est un jeu de conception militaire fictif. Le joueur accepte des contrats, assemble un véhicule dans un atelier 3D, configure un programme embarqué abstrait, réalise des essais reproductibles puis remet un dossier technique.

Toutes les valeurs sont des paramètres de gameplay fictifs. Elles ne constituent pas des données d'ingénierie réelles.

## Boucle de jeu

1. **Contrats** : consulter les offres, leurs exigences et leurs échéances.
2. **Atelier** : assembler les pièces dans une vue 3D, organiser les étages et vérifier la conception.
3. **Programme** : composer un programme visuel à partir des blocs compatibles avec les équipements installés.
4. **Essais** : lancer des essais reproductibles, lire les rapports et comparer des versions.
5. **Livraison** : remettre les preuves liées à la conception et au programme.

## Atelier de construction

L'atelier reprend plusieurs principes d'ergonomie de KSP :

- Vue 3D centrale dédiée à la construction.
- Palette de pièces filtrable par catégorie : propulsion, structure, aérodynamique, capteurs et avionique.
- Glisser-déposer d'une pièce vers un point d'attache visible.
- Déplacement d'une pièce avec tout son sous-ensemble.
- Symétrie radiale, annulation et rétablissement.
- Arbre d'assemblage présentant les pièces par nom, point d'attache et étage.
- Inspecteur pour consulter et régler la pièce sélectionnée.
- Panneaux de conformité, d'aérodynamique et de bilan des étages.

### Commandes de l'atelier

| Action | Commande |
|---|---|
| Ajouter une pièce | Glisser depuis la palette vers un point lumineux |
| Déplacer une pièce | Glisser une pièce existante vers un autre point |
| Sélectionner | Cliquer sur une pièce ou une ligne de l'arbre |
| Supprimer | Suppr |
| Changer la symétrie | X |
| Annuler | Ctrl + Z |
| Rétablir | Ctrl + Y |
| Tourner la caméra | Clic droit + glisser |
| Déplacer la caméra | Clic molette + glisser |
| Zoomer | Molette |
| Régler une pièce | Boutons − / + ou molette au-dessus de la pièce |

Le filtre de catégories ne modifie pas la conception. Il sert uniquement à réduire la liste des pièces affichées.

## Interface

Les vues principales sont :
- **Île** : accès aux installations.
- **Contrats** : offres, exigences et dossiers.
- **Atelier** : construction 3D.
- **Recherche** : déblocage des technologies.
- **Programme** : édition et validation du programme embarqué.
- **Essais** : simulation et comparaison.

Les identifiants internes, les noms de fichiers et les formats de sauvegarde restent inchangés afin de préserver la compatibilité.

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

## Sauvegardes

Les sauvegardes restent dans `saves/slot_1/`. La traduction et les améliorations de l'atelier ne modifient pas les schémas existants. Les conceptions, programmes, contrats et preuves conservent leurs identifiants et empreintes.

## Périmètre

Omega reste un jeu militaire fictif. Il ne cherche pas à reproduire des calculs réels de guidage, de ciblage, de structure ou de propulsion. Les essais utilisent des modèles abstraits destinés au gameplay.
