#!/usr/bin/env bash
export PATH="$HOME/.local/bin:$PATH"

set -euo pipefail

BUNDLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export BUNDLE_DIR

source "$BUNDLE_DIR/scripts/lib/log.sh"
source "$BUNDLE_DIR/scripts/lib/installer-bin.sh"

# The update itself belongs to the installer, so this entry point only has to find the binary and
# hand over. Keeping the published `bash update.sh` working is the whole job; the hand-off is
# shared with uninstall.sh so the two cannot drift apart the way they already did once.
caelestia_run_preset update "${1:-}"