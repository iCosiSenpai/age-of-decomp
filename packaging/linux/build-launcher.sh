#!/bin/sh
# The Linux release's two programs, AgeOfDecomp (the launcher's window) and aod-cli, built on Ubuntu 22.04 (glibc
# 2.35): the oldest base Tauri 2's WebKitGTK 4.1 builds on, so both start on SteamOS and on any distribution at least
# as new (RecompLauncher's guide: build "in a GitHub Actions ubuntu:22.04 container"). The release workflow runs it in
# that container; on a developer's PC (the frontend built first with `pnpm build`, any system):
#   docker run --rm -v "$PWD:/src" -v aod-rust:/root/.cargo -v aod-target:/target -e CARGO_TARGET_DIR=/target \
#       -w /src ubuntu:22.04 sh packaging/linux/build-launcher.sh
# then the programs are in the volume's release/ (tools/release.py linux --bin-dir takes that folder).
set -eu
export DEBIAN_FRONTEND=noninteractive
if [ "$(id -u)" = 0 ] && command -v apt-get >/dev/null; then
    apt-get update -qq
    apt-get install -y -qq --no-install-recommends ca-certificates curl build-essential pkg-config file \
        libwebkit2gtk-4.1-dev libssl-dev libxdo-dev librsvg2-dev >/dev/null
fi
if ! command -v cargo >/dev/null; then
    [ -f "$HOME/.cargo/env" ] || curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    . "$HOME/.cargo/env"
fi
cd apps/launcher/src-tauri
cargo build --locked --release --bins --features tauri/custom-protocol
