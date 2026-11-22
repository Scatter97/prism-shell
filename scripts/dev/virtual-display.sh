#!/usr/bin/env bash
# virtual-display.sh — rootless, disposable X11 display for Prism Shell UI testing.
#
# Runs Xvfb (extracted from Arch packages into ~/.local/opt, no root, no system
# changes) and captures PNG screenshots with ffmpeg's x11grab input.
#
# Usage:
#   scripts/dev/virtual-display.sh install
#   scripts/dev/virtual-display.sh start [WIDTHxHEIGHT] [DISPLAY_NUMBER]
#   scripts/dev/virtual-display.sh status
#   scripts/dev/virtual-display.sh screenshot [OUT.png]
#   scripts/dev/virtual-display.sh run-app [OPTIONS] [-- APP_ARGS...]
#   scripts/dev/virtual-display.sh stop
#
# Examples:
#   scripts/dev/virtual-display.sh start 1920x1080
#   scripts/dev/virtual-display.sh screenshot
#   scripts/dev/virtual-display.sh run-app --out artifacts/screenshots/dock.png
#   scripts/dev/virtual-display.sh start 1280x800 :101
#   scripts/dev/virtual-display.sh stop
#
# Environment:
#   PRISM_XVFB_HOME      install dir for Xvfb/xauth (default ~/.local/opt/prism-xvfb)
#   PRISM_VD_DISPLAY     preferred display number (default 99, next free if busy)
#   PRISM_APP_BIN        app binary (default <repo>/target/debug/prism-preview)
#   PRISM_VD_STATE_DIR   runtime state dir (default $XDG_RUNTIME_DIR/prism-virtual-display)
#   ARCH_MIRROR          Arch mirror base URL used by `install`
#
# No sudo. Nothing outside the state dir, the install dir, and the output path
# is written. `stop` only terminates processes recorded in the state dir.

set -euo pipefail

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
XVFB_HOME=${PRISM_XVFB_HOME:-"$HOME/.local/opt/prism-xvfb"}
XVFB_BIN="$XVFB_HOME/bin/Xvfb"
XAUTH_BIN="$XVFB_HOME/bin/xauth"

STATE_DIR=${PRISM_VD_STATE_DIR:-"${XDG_RUNTIME_DIR:-/tmp}/prism-virtual-display"}
STATE_DISPLAY="$STATE_DIR/display"
STATE_SIZE="$STATE_DIR/size"
STATE_XVFB_PID="$STATE_DIR/xvfb.pid"
STATE_APP_PID="$STATE_DIR/app.pid"
STATE_APP_BIN="$STATE_DIR/app.bin"
STATE_AUTH="$STATE_DIR/Xauthority"
LOG_XVFB="$STATE_DIR/xvfb.log"
LOG_APP="$STATE_DIR/app.log"

DEFAULT_SIZE=1920x1080
DEFAULT_DISPLAY=99
SCREENSHOT_DIR="$REPO_ROOT/artifacts/screenshots"

log() { printf '[virtual-display] %s\n' "$*"; }
die() { printf '[virtual-display] error: %s\n' "$*" >&2; exit 1; }

usage() {
    sed -n '7,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

# ---------------------------------------------------------------- dependencies

require_ffmpeg() {
    command -v ffmpeg >/dev/null 2>&1 || die "ffmpeg is required (pacman -S ffmpeg)"
    ffmpeg -hide_banner -devices 2>/dev/null | grep -q x11grab \
        || die "this ffmpeg build has no x11grab input device"
}

require_xvfb() {
    [[ -x "$XVFB_BIN" ]] || die "Xvfb not installed at $XVFB_BIN — run: $0 install"
    [[ -x "$XAUTH_BIN" ]] || die "xauth not installed at $XAUTH_BIN — run: $0 install"
}

require_xdpyinfo() {
    command -v xdpyinfo >/dev/null 2>&1 \
        || die "xdpyinfo is required to probe displays (pacman -S xorg-xdpyinfo)"
}

# ---------------------------------------------------------------- state helpers

have_state() { [[ -f "$STATE_DISPLAY" && -f "$STATE_XVFB_PID" ]]; }

state_display() { cat "$STATE_DISPLAY" 2>/dev/null || true; }
state_size() { cat "$STATE_SIZE" 2>/dev/null || true; }
state_xvfb_pid() { cat "$STATE_XVFB_PID" 2>/dev/null || true; }
state_app_pid() { cat "$STATE_APP_PID" 2>/dev/null || true; }

pid_alive() { [[ -n "${1:-}" ]] && kill -0 "$1" 2>/dev/null; }

cmdline_of() { tr '\0' ' ' <"/proc/$1/cmdline" 2>/dev/null || true; }

display_in_use() {
    local n=$1
    [[ -e "/tmp/.X${n}-lock" || -e "/tmp/.X11-unix/X${n}" ]]
}

clear_state() {
    rm -f "$STATE_DISPLAY" "$STATE_SIZE" "$STATE_XVFB_PID" "$STATE_APP_PID" \
        "$STATE_APP_BIN" "$STATE_AUTH"
}

# ---------------------------------------------------------------- install

package_version() {
    # Ask pacman for the current repo version; fall back to pinned versions.
    local name=$1 fallback=$2
    if command -v pacman >/dev/null 2>&1; then
        local v
        v=$(pacman -Si "$name" 2>/dev/null | awk -F': ' '/^Version[[:space:]]*:/{print $2; exit}')
        [[ -n "$v" ]] && { printf '%s' "$v"; return; }
    fi
    printf '%s' "$fallback"
}

fetch_package() {
    local pkg=$1 ver=$2 dest=$3 mirror=${ARCH_MIRROR:-https://geo.mirror.pkgbuild.com}
    local file="${pkg}-${ver}-x86_64.pkg.tar.zst"
    local url="$mirror/extra/os/x86_64/$file"
    log "downloading $file" >&2
    curl -fsSL --retry 3 --connect-timeout 20 -o "$dest/$file" "$url" \
        || die "download failed: $url (set ARCH_MIRROR or update the pinned version)"
    printf '%s' "$dest/$file"
}

cmd_install() {
    command -v curl >/dev/null 2>&1 || die "curl is required"
    command -v bsdtar >/dev/null 2>&1 || die "bsdtar (libarchive) is required"
    mkdir -p "$XVFB_HOME/bin"

    local tmp
    tmp=$(mktemp -d "${TMPDIR:-/tmp}/prism-xvfb.XXXXXX")
    # shellcheck disable=SC2064
    trap "rm -rf '$tmp'" EXIT

    local xvfb_ver xauth_ver
    xvfb_ver=$(package_version xorg-server-xvfb 21.1.26-1)
    xauth_ver=$(package_version xorg-xauth 1.1.5-1)

    local xvfb_pkg xauth_pkg
    xvfb_pkg=$(fetch_package xorg-server-xvfb "$xvfb_ver" "$tmp")
    xauth_pkg=$(fetch_package xorg-xauth "$xauth_ver" "$tmp")

    log "extracting binaries into $XVFB_HOME/bin"
    bsdtar -xf "$xvfb_pkg" -C "$tmp" usr/bin/Xvfb
    bsdtar -xf "$xauth_pkg" -C "$tmp" usr/bin/xauth
    install -m 755 "$tmp/usr/bin/Xvfb" "$XVFB_BIN"
    install -m 755 "$tmp/usr/bin/xauth" "$XAUTH_BIN"

    local missing
    missing=$(ldd "$XVFB_BIN" 2>/dev/null | awk '/not found/{print $1}')
    if [[ -n "$missing" ]]; then
        log "warning: Xvfb has unresolved shared libraries:"
        printf '  %s\n' $missing >&2
        log "install the corresponding Arch packages (e.g. libxfont2, pixman, libgl, nettle)"
    fi
    command -v xkbcomp >/dev/null 2>&1 \
        || log "warning: xkbcomp not found; keyboard handling may be limited (pacman -S xorg-xkbcomp)"

    log "installed: $XVFB_BIN"
    log "done"
}

# ---------------------------------------------------------------- start/stop/status

cmd_start() {
    local size=${1:-$DEFAULT_SIZE}
    local wanted=${2:-${PRISM_VD_DISPLAY:-$DEFAULT_DISPLAY}}
    wanted=${wanted#:}
    [[ "$size" =~ ^[0-9]{3,5}x[0-9]{3,5}$ ]] || die "size must look like 1920x1080 (got '$size')"
    [[ "$wanted" =~ ^[0-9]{1,3}$ ]] || die "display number must be numeric, e.g. 99 or :99 (got '$wanted')"
    require_xvfb
    require_xdpyinfo

    if have_state; then
        local old_pid old_disp
        old_pid=$(state_xvfb_pid)
        old_disp=$(state_display)
        if pid_alive "$old_pid" && [[ "$(cmdline_of "$old_pid")" == *"Xvfb :$old_disp"* ]]; then
            die "already running on :$old_disp ($(state_size)); run '$0 stop' first"
        fi
        log "removing stale state from :$old_disp"
        clear_state
    fi

    mkdir -p "$STATE_DIR"
    chmod 700 "$STATE_DIR"

    # Pick the requested display, or the next free one if it is occupied.
    local disp=""
    local n
    for ((n = wanted; n < wanted + 50; n++)); do
        if ! display_in_use "$n"; then disp=$n; break; fi
    done
    [[ -n "$disp" ]] || die "no free X display in :$wanted..$((wanted + 49))"
    if [[ "$disp" != "$wanted" ]]; then
        log "display :$wanted is busy; using :$disp"
    fi

    # Cookie-based access control. The server is unix-socket only (-nolisten tcp).
    umask 077
    local cookie
    cookie=$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n')
    rm -f "$STATE_AUTH"
    : >"$STATE_AUTH"
    "$XAUTH_BIN" -f "$STATE_AUTH" add ":$disp" . "$cookie" >/dev/null

    log "starting Xvfb on :$disp at ${size}x24 (dpi 96)"
    "$XVFB_BIN" ":$disp" \
        -screen 0 "${size}x24" \
        -dpi 96 \
        -nolisten tcp \
        -noreset \
        -auth "$STATE_AUTH" \
        >"$LOG_XVFB" 2>&1 &
    local pid=$!

    local ready=0
    for _ in $(seq 1 100); do
        if XAUTHORITY="$STATE_AUTH" xdpyinfo -display ":$disp" >/dev/null 2>&1; then
            ready=1
            break
        fi
        pid_alive "$pid" || break
        sleep 0.1
    done

    if (( ! ready )); then
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
        clear_state
        log "Xvfb failed to start; log follows:" >&2
        tail -20 "$LOG_XVFB" >&2 || true
        exit 1
    fi

    printf '%s\n' "$disp" >"$STATE_DISPLAY"
    printf '%s\n' "$size" >"$STATE_SIZE"
    printf '%s\n' "$pid" >"$STATE_XVFB_PID"

    log "virtual display ready: :$disp ${size} (pid $pid)"
    log "state: $STATE_DIR"
}

cmd_stop() {
    if ! have_state; then
        log "no virtual display is recorded as running"
        rm -rf "$STATE_DIR"
        return 0
    fi

    local disp xpid apid app_bin
    disp=$(state_display)
    xpid=$(state_xvfb_pid)
    apid=$(state_app_pid)
    app_bin=$(cat "$STATE_APP_BIN" 2>/dev/null || true)

    # Stop only processes we recorded and can still identify.
    if pid_alive "$apid"; then
        local app_name
        app_name=$(basename "${app_bin:-prism-preview}")
        if [[ "$(cmdline_of "$apid")" == *"$app_name"* ]]; then
            log "stopping app pid $apid"
            kill "$apid" 2>/dev/null || true
        fi
    fi

    if pid_alive "$xpid" && [[ "$(cmdline_of "$xpid")" == *"Xvfb :$disp"* ]]; then
        log "stopping Xvfb on :$disp (pid $xpid)"
        kill "$xpid" 2>/dev/null || true
        for _ in $(seq 1 50); do
            pid_alive "$xpid" || break
            sleep 0.1
        done
        pid_alive "$xpid" && kill -9 "$xpid" 2>/dev/null || true
    fi

    rm -rf "$STATE_DIR"
    log "stopped"
}

cmd_status() {
    if ! have_state || ! pid_alive "$(state_xvfb_pid)"; then
        log "not running"
        return 1
    fi
    local disp size xpid apid
    disp=$(state_display)
    size=$(state_size)
    xpid=$(state_xvfb_pid)
    apid=$(state_app_pid)
    log "display :$disp ${size} — Xvfb pid $xpid"
    if pid_alive "$apid"; then
        log "app pid $apid running"
    else
        log "no app running"
    fi
    log "log: $LOG_XVFB"
}

# ---------------------------------------------------------------- screenshot

capture() {
    local out=$1
    require_xvfb
    require_ffmpeg
    have_state || die "no virtual display running — run '$0 start' first"

    local disp size xpid
    disp=$(state_display)
    size=$(state_size)
    xpid=$(state_xvfb_pid)
    pid_alive "$xpid" || die "Xvfb pid $xpid is gone — run '$0 start'"
    [[ -f "$STATE_AUTH" ]] || die "missing Xauthority file; restart the display"

    mkdir -p "$(dirname "$out")"
    XAUTHORITY="$STATE_AUTH" ffmpeg -hide_banner -loglevel error -y \
        -f x11grab -draw_mouse 0 -video_size "$size" \
        -i ":$disp" -frames:v 1 -update 1 "$out"
    [[ -s "$out" ]] || die "screenshot was not written: $out"
    log "screenshot: $out"
}

cmd_screenshot() {
    local out=${1:-}
    if [[ -z "$out" ]]; then
        local size
        size=$(state_size)
        [[ -n "$size" ]] || die "no virtual display running — run '$0 start' first"
        out="$SCREENSHOT_DIR/prism-${size}-$(date +%Y%m%d-%H%M%S).png"
    fi
    capture "$out"
}

# ---------------------------------------------------------------- run app

cmd_run_app() {
    local binary=${PRISM_APP_BIN:-"$REPO_ROOT/target/debug/prism-preview"}
    local wait_secs=6
    local out=""
    local keep=0
    local app_args=()

    while (($#)); do
        case $1 in
            -b|--binary) [[ $# -ge 2 ]] || die "--binary needs a path"; binary=$2; shift 2 ;;
            -w|--wait) [[ $# -ge 2 ]] || die "--wait needs seconds"; wait_secs=$2; shift 2 ;;
            -o|--out) [[ $# -ge 2 ]] || die "--out needs a path"; out=$2; shift 2 ;;
            -k|--keep) keep=1; shift ;;
            --) shift; app_args=("$@"); break ;;
            -h|--help) usage 0 ;;
            *) die "unknown run-app option: $1 (use '--' before app arguments)" ;;
        esac
    done

    [[ "$wait_secs" =~ ^[0-9]+([.][0-9]+)?$ ]] || die "--wait must be a number of seconds"
    [[ -x "$binary" ]] || die "app binary not found or not executable: $binary"
    require_ffmpeg
    have_state || die "no virtual display running — run '$0 start' first"

    local disp xpid old_apid old_bin
    disp=$(state_display)
    xpid=$(state_xvfb_pid)
    pid_alive "$xpid" || die "Xvfb pid $xpid is gone — run '$0 start'"

    old_apid=$(state_app_pid)
    old_bin=$(cat "$STATE_APP_BIN" 2>/dev/null || true)
    if pid_alive "$old_apid"; then
        if [[ -n "$old_bin" && "$(cmdline_of "$old_apid")" == *"$(basename "$old_bin")"* ]]; then
            log "replacing previous app pid $old_apid"
            kill "$old_apid" 2>/dev/null || true
            sleep 0.3
        else
            log "ignoring stale app pid $old_apid (command line no longer matches)"
        fi
    fi

    log "launching $(basename "$binary") on :$disp"
    env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET \
        DISPLAY=":$disp" XAUTHORITY="$STATE_AUTH" \
        "$binary" ${app_args[@]+"${app_args[@]}"} >"$LOG_APP" 2>&1 &
    local apid=$!
    printf '%s\n' "$apid" >"$STATE_APP_PID"
    printf '%s\n' "$binary" >"$STATE_APP_BIN"

    # Wait in small steps so an early crash is reported quickly.
    local waited=0
    while :; do
        if ! pid_alive "$apid"; then
            log "app exited before capture; log ($LOG_APP):" >&2
            tail -30 "$LOG_APP" >&2 || true
            rm -f "$STATE_APP_PID" "$STATE_APP_BIN"
            exit 1
        fi
        awk -v w="$waited" -v t="$wait_secs" 'BEGIN{exit !(w>=t)}' && break
        sleep 0.25
        waited=$(awk -v w="$waited" 'BEGIN{printf "%.2f", w+0.25}')
    done

    cmd_screenshot "$out"

    if (( keep )); then
        log "app left running (pid $apid); log: $LOG_APP"
    else
        kill "$apid" 2>/dev/null || true
        rm -f "$STATE_APP_PID" "$STATE_APP_BIN"
    fi
}

# ---------------------------------------------------------------- main

main() {
    local cmd=${1:-help}
    shift || true
    case $cmd in
        install) cmd_install ;;
        start) cmd_start "$@" ;;
        stop) cmd_stop ;;
        status) cmd_status ;;
        screenshot|shot) cmd_screenshot "$@" ;;
        run-app|run) cmd_run_app "$@" ;;
        help|-h|--help) usage 0 ;;
        *) printf 'unknown command: %s\n\n' "$cmd" >&2; usage 1 ;;
    esac
}

main "$@"
