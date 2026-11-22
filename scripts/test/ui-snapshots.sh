#!/usr/bin/env bash
# ui-snapshots.sh — reproducible UI-only PNG captures for the Linux preview.
#
# Requires the rootless Xvfb helper to be installed:
#   scripts/dev/virtual-display.sh install
#
# Run from the repository root. The virtual display and any preview process are
# stopped when the script exits. All output is under artifacts/screenshots/.

set -euo pipefail

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$REPO_ROOT"
export PATH="$HOME/.cargo/bin:$PATH"

OUT_DIR="$REPO_ROOT/artifacts/screenshots"
STATE_DIR=${PRISM_VD_STATE_DIR:-"${XDG_RUNTIME_DIR:-/tmp}/prism-virtual-display"}
DISPLAY_HELPER="$REPO_ROOT/scripts/dev/virtual-display.sh"

if [[ ! -x "$DISPLAY_HELPER" ]]; then
    echo "error: missing $DISPLAY_HELPER" >&2
    exit 1
fi
if [[ ! -x "$HOME/.local/opt/prism-xvfb/bin/Xvfb" ]]; then
    echo "error: Xvfb is not installed. Run: scripts/dev/virtual-display.sh install" >&2
    exit 1
fi

mkdir -p "$OUT_DIR"

cleanup() {
    "$DISPLAY_HELPER" stop >/dev/null 2>&1 || true
}
trap cleanup EXIT

"$DISPLAY_HELPER" stop >/dev/null 2>&1 || true
"$DISPLAY_HELPER" start 1920x1080

DISPLAY_NUMBER=$(cat "$STATE_DIR/display")
export DISPLAY=":$DISPLAY_NUMBER"
export XAUTHORITY="$STATE_DIR/Xauthority"

cargo build -p prism-app --bin prism-preview >/dev/null

APP="$REPO_ROOT/target/debug/prism-preview"

shoot() {
    local name=$1
    shift
    local path="$OUT_DIR/auto-$name.png"
    echo "capturing $name"
    "$APP" --scale 1 --screenshot "$path" "$@" >/dev/null
    if [[ ! -s "$path" ]]; then
        echo "error: screenshot missing: $path" >&2
        exit 1
    fi
}

# Baseline and every fixture at 1920x1080.
shoot "dock-1920x1080" --width 1920 --height 1080 --scenario laptop
shoot "cc-laptop-1920x1080" --width 1920 --height 1080 --scenario laptop --open-cc
shoot "cc-desktop-1920x1080" --width 1920 --height 1080 --scenario desktop --open-cc
shoot "cc-minimal-1920x1080" --width 1920 --height 1080 --scenario minimal --open-cc
shoot "cc-faults-1920x1080" --width 1920 --height 1080 --scenario faults --open-cc
shoot "many-1920x1080" --width 1920 --height 1080 --scenario many

# Interactive surfaces.
shoot "launcher-1920x1080" --width 1920 --height 1080 --open-launcher
shoot "preview-1920x1080" --width 1920 --height 1080 --preview browser
shoot "confirm-1920x1080" --width 1920 --height 1080 --open-cc --confirm

# Small logical canvas for overflow and scroll review.
shoot "many-1280x800" --width 1280 --height 800 --scenario many
shoot "cc-1280x800" --width 1280 --height 800 --scenario laptop --open-cc

# Scale checks: all produce 1920x1080 physical captures.
shoot "cc-125pct" --width 1536 --height 864 --scale 1.25 --scenario laptop --open-cc
shoot "cc-150pct" --width 1280 --height 720 --scale 1.5 --scenario laptop --open-cc
shoot "cc-200pct" --width 960 --height 540 --scale 2 --scenario laptop --open-cc

echo
echo "snapshots written to $OUT_DIR:"
ls -lh "$OUT_DIR"/auto-*.png
