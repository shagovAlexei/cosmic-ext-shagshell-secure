#!/usr/bin/env bash
# Adapted from shagoff-commander (.claude/skills/shagoff-feature/headless.sh).
# Run ShagShell Secure in an invisible sway session and drive it, without touching the user's
# desktop, focus, config or state. Needs: sway, wtype, grim.
#
#   headless.sh start [WxH]        start headless sway (default 1200x800) + the app (debug build)
#   headless.sh key ARGS...        wtype ARGS (e.g. -k F7, -M shift -k F2 -m shift)
#   headless.sh click X Y [right|middle|double|none|up|down]  mouse at (X, Y), click or wheel (built from ./vpointer once)
#   headless.sh drag X1 Y1 X2 Y2   press at (X1, Y1), move to (X2, Y2), release
#   headless.sh vkey [ctrl] [alt] [shift] [ru] KEY  a real key (a-z or evdev code) on a us,ru keymap:
#                                  Ctrl+letters and Alt+letters arrive as from a keyboard
#   headless.sh shot NAME          screenshot to $DIR/NAME.png
#   headless.sh sshd start|stop    a user-space sshd on 127.0.0.1:2222 for this user, its own host key
#                                  and client key $DIR/sshd/id_test (nothing system-wide touched)
#   headless.sh session ID         write a session file ID (path) for that sshd into the app's config
#   headless.sh stop
#
# Limits: wtype uploads its own keymap, so letter shortcuts bound to physical keys (Ctrl+M,
# Ctrl+Shift+S, Alt+L…) do not arrive through `key` — send those with `vkey`; named keys (F-keys,
# Enter, Tab, arrows, Alt+F7…) and typed text go through `key`. Keys need
# the 300 ms pauses below or the first one is lost to the keymap switch.
set -euo pipefail
DIR=${SHAGSHELL_HEADLESS:-/tmp/shagshell-headless}
ROOT=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
mkdir -p "$DIR"/{cfg,state,cache,data}
sock() { cat "$DIR/wayland"; }
# pid files, not `pkill -f`: a pattern also matches the shell that runs this script.
kill_pid() { [ -f "$DIR/$1.pid" ] && { kill "$(cat "$DIR/$1.pid")" 2>/dev/null || true; }; rm -f "$DIR/$1.pid"; }
app() {
    kill_pid app
    cd "$ROOT"
    WAYLAND_DISPLAY="$(sock)" XDG_CONFIG_HOME="$DIR/cfg" XDG_STATE_HOME="$DIR/state" XDG_DATA_HOME="$DIR/data" \
        XDG_CACHE_HOME="$DIR/cache" ICED_BACKEND="${ICED_BACKEND:-}" setsid "$ROOT/target/debug/shagshell" >"$DIR/app.log" 2>&1 &
    echo $! >"$DIR/app.pid"
    sleep 3
}
case ${1:-} in
start)
    kill_pid app; kill_pid sway  # a second `start` must not leak the first sway
    before=$(ls "$XDG_RUNTIME_DIR" | grep -E '^wayland-[0-9]+$' || true)
    printf 'output HEADLESS-1 resolution %s\ndefault_border none\n' "${2:-1200x800}" >"$DIR/sway.cfg"
    echo "${2:-1200x800}" >"$DIR/size"
    env -u WAYLAND_DISPLAY -u DISPLAY WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=${WLR_RENDERER:-pixman} \
        setsid sway -c "$DIR/sway.cfg" >"$DIR/sway.log" 2>&1 &
    echo $! >"$DIR/sway.pid"
    sleep 2
    comm -13 <(echo "$before" | sort) <(ls "$XDG_RUNTIME_DIR" | grep -E '^wayland-[0-9]+$' | sort) | head -1 >"$DIR/wayland"
    [ -s "$DIR/wayland" ] || { echo "sway did not start, see $DIR/sway.log"; exit 1; }
    (cd "$ROOT" && cargo build -q -p shagshell)
    app ;;
key) shift; WAYLAND_DISPLAY="$(sock)" wtype -s 300 "$@" -s 300 ;;
click)
    vp="$ROOT/.claude/skills/shagshell-feature/vpointer"
    (cd "$vp" && cargo build -q --release --offline)
    size=$(cat "$DIR/size" 2>/dev/null || echo 1200x800)  # the output's, for absolute motion
    WAYLAND_DISPLAY="$(sock)" "$vp/target/release/vpointer" "$2" "$3" "${4:-left}" "${size%x*}" "${size#*x}"; sleep 0.5 ;;
drag)
    vp="$ROOT/.claude/skills/shagshell-feature/vpointer"
    (cd "$vp" && cargo build -q --release --offline)
    size=$(cat "$DIR/size" 2>/dev/null || echo 1200x800)
    WAYLAND_DISPLAY="$(sock)" "$vp/target/release/vpointer" drag "$2" "$3" "$4" "$5" "${size%x*}" "${size#*x}"; sleep 0.5 ;;
vkey)
    shift; vp="$ROOT/.claude/skills/shagshell-feature/vpointer"
    (cd "$vp" && cargo build -q --release --offline)
    WAYLAND_DISPLAY="$(sock)" "$vp/target/release/vkey" "$@"; sleep 0.3 ;;
shot) WAYLAND_DISPLAY="$(sock)" grim "$DIR/$2.png"; echo "$DIR/$2.png" ;;
sshd)
    S="$DIR/sshd"
    case ${2:-start} in
    start)
        mkdir -p "$S"
        [ -f "$S/host_key" ] || ssh-keygen -q -t ed25519 -N '' -f "$S/host_key"
        [ -f "$S/id_test" ] || ssh-keygen -q -t ed25519 -N '' -f "$S/id_test"
        cp "$S/id_test.pub" "$S/authorized_keys"
        printf '%s\n' "Port 2222" "ListenAddress 127.0.0.1" "HostKey $S/host_key" \
            "AuthorizedKeysFile $S/authorized_keys" "PidFile $S/sshd.pid" "UsePAM no" \
            "StrictModes no" "PasswordAuthentication no" "KbdInteractiveAuthentication no" >"$S/sshd_config"
        kill_pid sshd
        setsid /usr/sbin/sshd -D -f "$S/sshd_config" -E "$S/sshd.log" >/dev/null 2>&1 &
        echo $! >"$DIR/sshd.pid"
        sleep 1
        ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile="$S/known_hosts" \
            -i "$S/id_test" -p 2222 127.0.0.1 true && echo "sshd ok on 127.0.0.1:2222" ;;
    stop) kill_pid sshd ;;
    esac ;;
session)
    f="$DIR/cfg/shagshell/sessions/$2.toml"; mkdir -p "$(dirname "$f")"
    printf '%s\n' 'host = "127.0.0.1"' 'port = 2222' "identity = \"$DIR/sshd/id_test\"" \
        "extra = \"-o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=$DIR/sshd/known_hosts\"" >"$f"
    echo "$f" ;;
stop) kill_pid app; kill_pid sway; kill_pid sshd ;;
*) sed -n '2,18p' "$0"; exit 1 ;;
esac
