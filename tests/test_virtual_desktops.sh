#!/usr/bin/env bash
# suite: fast

set -uo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/helpers.sh"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# What a fresh install does to the desktop count is the native 06-services step's own
# behaviour, and the crate tests it there (installer/tests/steps.rs). What is left here is
# the wiring no single file can see: which steps apply the rule, and what marks a run as an
# install rather than an update.

test_the_helper_reaches_both_steps_that_apply_it() {
    local native script
    native="$(cat "$REPO_ROOT/installer/src/steps/services.rs")"
    assert_contains "$native" "ensure_five_desktops" \
        "the native services step should apply the helper"

    script="$(cat "$REPO_ROOT/scripts/09-system-tweaks.sh")"
    assert_contains "$script" "lib/virtual-desktops.sh" "09-system-tweaks.sh should source the helper"
    assert_contains "$script" "ensure_five_virtual_desktops" "09-system-tweaks.sh should apply the helper"
}

test_the_install_entries_mark_the_run_and_the_update_does_not() {
    assert_contains "$(cat "$REPO_ROOT/installer/src/app.rs")" \
        '("CAELESTIA_FRESH_INSTALL", "1"' \
        "the TUI installer should mark the run it runs the steps for"
    assert_contains "$(cat "$REPO_ROOT/src/bin/caelestia")" \
        'CAELESTIA_FRESH_INSTALL="${CAELESTIA_FRESH_INSTALL:-1}"' \
        "a packaged install should mark the run"
    assert_contains "$(cat "$REPO_ROOT/src/bin/caelestia-update")" \
        'CAELESTIA_FRESH_INSTALL=0' \
        "the updater re-applying a packaged install must not mark it"
}

run_tests