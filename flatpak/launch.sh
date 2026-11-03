#!/bin/sh
# Flatpak entry: the home's own dirs, not ~/.var/app/…, so sessions, settings, logs and scripts
# are the same as a .deb install's (adapted from shagoff-commander's flatpak/launch.sh).
export XDG_CONFIG_HOME="$HOME/.config" XDG_DATA_HOME="$HOME/.local/share" \
    XDG_STATE_HOME="$HOME/.local/state" XDG_CACHE_HOME="$HOME/.cache"
# flatpak sets SHELL=/bin/sh: the user's login shell for the local shell tab.
host_shell=$(flatpak-spawn --host getent passwd "$(id -un)" | cut -d: -f7) && [ -n "$host_shell" ] && export SHELL="$host_shell"
exec /app/bin/shagshell "$@"
