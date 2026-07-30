#!/usr/bin/env bash
# Uploads game builds to itch.io with butler.
# Requires `butler login` to have been run once beforehand.
# Runs on Linux or on Windows under Git Bash. Whichever platform you're on is
# built natively with cargo; the other one is cross-compiled in a container, so
# `cross` and a working Docker/Podman are needed either way.
set -euo pipefail

# put your username and game project on itch here
ITCH_PROJECT="valorzard/rust-kiss3d-game-template"
RUST_BINARY_NAME="rust-kiss3d-game-template"
# Tag every upload with the commit it was built from so itch's version history is
# traceable. --dirty marks builds made from an uncommitted tree, so a version on
# itch can't silently claim to be a clean commit it isn't.
VERSION="$(git describe --tags --always --dirty)"

# cross only publishes images for the *-gnu Windows target, not msvc.
WINDOWS_TARGET="x86_64-pc-windows-gnu"
LINUX_TARGET="x86_64-unknown-linux-gnu"

case "$(uname -s)" in
    Linux) HOST_OS="linux" ;;
    MINGW* | MSYS* | CYGWIN*) HOST_OS="windows" ;;
    *)
        echo "unsupported host '$(uname -s)': expected Linux or Windows (Git Bash)" >&2
        exit 1
        ;;
esac

# Builds for $1 ("windows" or "linux") and sets BINARY_PATH to the result.
# Uses a variable rather than command substitution so cargo's progress output
# still goes straight to the terminal.
build_binary() {
    local os="$1"
    local suffix="" target=""

    if [ "$os" = "windows" ]; then
        suffix=".exe"
        target="$WINDOWS_TARGET"
    else
        target="$LINUX_TARGET"
    fi

    if [ "$os" = "$HOST_OS" ]; then
        cargo build --release
        BINARY_PATH="target/release/$RUST_BINARY_NAME$suffix"
    else
        cross build --release --target "$target"
        BINARY_PATH="target/$target/release/$RUST_BINARY_NAME$suffix"
    fi
}

# Web (HTML5) build; Trunk.toml already sets public_url = "." so assets resolve
# from the subpath itch serves games under.
trunk build --release
butler push ./dist "$ITCH_PROJECT:html5" --userversion "$VERSION"

staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT

# Assets are fetched at runtime next to the executable (see src/asset_fetch.rs),
# so ship the assets/ folder alongside each binary.
for os in windows linux; do
    build_binary "$os"

    dir="$staging/$os"
    mkdir -p "$dir"
    cp "$BINARY_PATH" "$dir/$(basename "$BINARY_PATH")"
    cp -r ./assets "$dir/assets"

    butler push "$dir" "$ITCH_PROJECT:$os" --userversion "$VERSION"
done

butler status "$ITCH_PROJECT"
