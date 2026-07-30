#!/usr/bin/env bash
# Uploads game builds to itch.io with butler.
# Requires `butler login` to have been run once beforehand.
# this script only really works on windows with git bash for now
set -euo pipefail

# put your username and game project on itch here
ITCH_PROJECT="valorzard/rust-kiss3d-game-template"
RUST_BINARY_NAME="rust-kiss3d-game-template"
# Tag every upload with the commit it was built from so itch's version history is
# traceable. --dirty marks builds made from an uncommitted tree, so a version on
# itch can't silently claim to be a clean commit it isn't.
VERSION="$(git describe --tags --always --dirty)"

# Web (HTML5) build; Trunk.toml already sets public_url = "." so assets resolve
# from the subpath itch serves games under.
trunk build --release
butler push ./dist "$ITCH_PROJECT:html5" --userversion "$VERSION"

staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT
mkdir "$staging/windows" "$staging/linux"

# Native Windows build. Assets are fetched at runtime next to the exe
# (see src/asset_fetch.rs), so ship the assets/ folder alongside the binary.
cargo build --release
cp "target/release/$RUST_BINARY_NAME.exe" "$staging/windows/$RUST_BINARY_NAME.exe"
cp -r ./assets "$staging/windows/assets"
butler push "$staging/windows" "$ITCH_PROJECT:windows" --userversion "$VERSION"

# Native Linux build, cross-compiled in Docker
cross build --target x86_64-unknown-linux-gnu --release
cp "target/x86_64-unknown-linux-gnu/release/$RUST_BINARY_NAME" "$staging/linux/$RUST_BINARY_NAME"
cp -r ./assets "$staging/linux/assets"
butler push "$staging/linux" "$ITCH_PROJECT:linux" --userversion "$VERSION"

butler status "$ITCH_PROJECT"