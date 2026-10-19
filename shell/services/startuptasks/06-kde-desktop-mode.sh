#!/usr/bin/env bash
# Enable KDE desktop mode and sync KDE wallpaper layout to desktop mode.
set -euo pipefail

SYNC_SCRIPT=""
for candidate in \
    "${SCRIPTS_DIR:-}/sync-kde-wallpaper-layout.sh" \
    "$HOME/.config/quickshell/caelestia/scripts/sync-kde-wallpaper-layout.sh" \
    "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." 2>/dev/null && pwd)/scripts/sync-kde-wallpaper-layout.sh"; do
    if [[ -f "$candidate" ]]; then
        SYNC_SCRIPT="$candidate"
        break
    fi
done

if [[ -n "$SYNC_SCRIPT" ]]; then
    bash "$SYNC_SCRIPT" desktop
    echo "StartupTasks: Synced KDE wallpaper layout to desktop mode"
fi

exit 0
