# Installation sous Linux

Omega est un jeu de bureau Bevy. La version distribuable est une archive Linux x86_64 ; elle ne
contient pas les bibliothèques graphiques, audio et de fenêtrage du système.

## Prérequis

- Linux x86_64 avec une session graphique (X11 ou Wayland).
- Un pilote Vulkan fonctionnel (`mesa-vulkan-drivers` sur Debian/Ubuntu/Mint).
- Les bibliothèques d'exécution utilisées par Bevy : X11, Wayland, ALSA et udev.

Sur Linux Mint / Ubuntu / Debian :

```bash
sudo apt update
sudo apt install libx11-6 libxkbcommon-x11-0 libxkbcommon0 libwayland-client0 \
  libasound2 libudev1 mesa-vulkan-drivers
```

Pour compiler, ajoutez les en-têtes de développement :

```bash
sudo apt install build-essential pkg-config libx11-dev libasound2-dev libudev-dev \
  libxkbcommon-x11-0 libwayland-dev libxkbcommon-dev
```

## Lancer une archive distribuable

```bash
tar -xzf omega-v0.47.0-linux-x86_64.tar.gz
cd omega-v0.47.0-linux-x86_64
sha256sum -c ../omega-v0.47.0-linux-x86_64.tar.gz.sha256
./omega
```

Le binaire crée `saves/`, `trials/`, `programs/` et `reports/` dans le répertoire courant. Lancez-le
depuis le dossier où vous souhaitez conserver la campagne, ou déplacez ces dossiers avec la
sauvegarde.

## Compiler et lancer depuis les sources

```bash
rustup update stable
cargo build --locked --release
./target/release/omega
```

Le premier build compile Bevy et prend plusieurs minutes. La version de Rust est fixée par
`rust-toolchain.toml` ; Bevy est épinglé à `=0.20.1`.

## Créer l'archive locale

```bash
scripts/package-linux.sh
```

Le script construit `dist/omega-v<version>-linux-x86_64.tar.gz` et son fichier SHA-256, avec le
binaire, les deux README, la licence, la vision, ce guide et `docs/BALANCE.md`. Il ne publie rien et
ne crée pas de tag Git. La publication GitHub reste réservée au workflow `release.yml` déclenché par
un tag `v*` correspondant à la version de `Cargo.toml`.

## Dépannage

- `failed to create a Vulkan instance` : installez `mesa-vulkan-drivers` et vérifiez que la session
  graphique expose bien Vulkan.
- `error while loading shared libraries` : installez les bibliothèques d'exécution listées ci-dessus.
- Aucun écran disponible (SSH, conteneur) : l'interface Bevy ne peut pas démarrer. Utilisez les modes
  sans fenêtre (`omega --aero-report`, `--balance-report`, `--trial`, `--revisions`) pour vérifier
  les règles et les fichiers de campagne.

Les commandes `--trial` et `--revisions` lisent un slot existant ; elles ne remplacent pas une
campagne et n'écrivent aucun rapport dans le dossier de sauvegarde.
