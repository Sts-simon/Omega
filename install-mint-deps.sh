#!/usr/bin/env bash
set -euo pipefail

sudo apt update
sudo apt install -y \
  build-essential pkg-config libx11-dev libasound2-dev libudev-dev \
  libxkbcommon-x11-0 libwayland-dev libxkbcommon-dev mesa-vulkan-drivers

if ! command -v rustup >/dev/null 2>&1; then
  echo "rustup is not installed. Install it with:"
  echo "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  exit 0
fi

rustup update stable
rustup component add rustfmt clippy --toolchain stable

echo "Dependencies are installed. Run: cargo run"
