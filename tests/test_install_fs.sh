#!/usr/bin/env bash
# suite: isolated

set -uo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/helpers.sh"
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/scripts/lib/install-fs.sh"


test_wait_for_nonempty_file_returns_immediately_when_it_is_there() {
    local tmp status
    tmp="$(new_tmpdir)"
    printf 'scheme\n' > "$tmp/scheme.json"

    wait_for_nonempty_file "$tmp/scheme.json" 0
    status=$?

    assert_status 0 "$status" "an existing file should not be waited on"
}

test_wait_for_nonempty_file_times_out_on_a_missing_file() {
    local tmp status
    tmp="$(new_tmpdir)"

    wait_for_nonempty_file "$tmp/absent.json" 0
    status=$?

    assert_status 1 "$status" "a file that never arrives should report a timeout"
}

test_wait_for_nonempty_file_treats_an_empty_file_as_absent() {
    local tmp status
    tmp="$(new_tmpdir)"
    : > "$tmp/scheme.json"

    wait_for_nonempty_file "$tmp/scheme.json" 0
    status=$?

    assert_status 1 "$status" "a truncated file is not yet usable"
}

test_wait_for_nonempty_file_returns_once_the_file_lands() {
    local tmp status
    tmp="$(new_tmpdir)"

    ( sleep 1; printf 'scheme\n' > "$tmp/scheme.json" ) &
    wait_for_nonempty_file "$tmp/scheme.json" 5
    status=$?
    wait

    assert_status 0 "$status" "the wait should end as soon as the file is written"
}

test_validate_install_manifest_accepts_a_complete_tree() {
    local tmp status
    tmp="$(new_tmpdir)"
    mkdir -p "$tmp/root/usr/bin"
    : > "$tmp/root/usr/bin/app"
    printf '/usr/bin/app\n' > "$tmp/manifest.txt"

    validate_install_manifest "$tmp/manifest.txt" "$tmp/root"
    status=$?

    assert_status 0 "$status" "a manifest whose paths all exist should pass"
}

test_validate_install_manifest_rejects_a_missing_path() {
    local tmp status
    tmp="$(new_tmpdir)"
    mkdir -p "$tmp/root/usr/bin"
    : > "$tmp/root/usr/bin/app"
    printf '/usr/bin/app\n/usr/bin/absent\n' > "$tmp/manifest.txt"

    validate_install_manifest "$tmp/manifest.txt" "$tmp/root" 2>/dev/null
    status=$?

    assert_status 1 "$status" "a manifest referencing a missing path should fail"
}

test_validate_install_manifest_rejects_vcs_metadata() {
    local tmp status
    tmp="$(new_tmpdir)"
    mkdir -p "$tmp/root/usr/share/.git"
    : > "$tmp/root/usr/share/.git/config"
    printf '/usr/share/.git/config' > "$tmp/manifest.txt"

    validate_install_manifest "$tmp/manifest.txt" "$tmp/root" 2>/dev/null
    status=$?

    assert_status 1 "$status" "VCS metadata must not be accepted in the package manifest"
}

test_validate_install_manifest_rejects_a_missing_manifest() {
    local tmp status
    tmp="$(new_tmpdir)"
    mkdir -p "$tmp/root"

    validate_install_manifest "$tmp/absent.txt" "$tmp/root" 2>/dev/null
    status=$?

    assert_status 1 "$status" "a missing manifest should fail"
}

test_validate_install_manifest_checks_the_unterminated_last_entry() {
    local tmp status
    tmp="$(new_tmpdir)"
    mkdir -p "$tmp/root/usr/bin"
    : > "$tmp/root/usr/bin/app"
    printf '/usr/bin/app\n/usr/bin/absent' > "$tmp/manifest.txt"

    validate_install_manifest "$tmp/manifest.txt" "$tmp/root" 2>/dev/null
    status=$?

    assert_status 1 "$status" "the final entry must be checked even without a trailing newline"
}

test_validate_install_manifest_accepts_a_cmake_shaped_manifest() {
    local tmp status
    tmp="$(new_tmpdir)"
    mkdir -p "$tmp/root/usr/share/caelestia/src"
    : > "$tmp/root/usr/share/caelestia/src/hyprland.conf"
    : > "$tmp/root/usr/share/caelestia/src/starship.toml"
    printf '/usr/share/caelestia/src/hyprland.conf\n/usr/share/caelestia/src/starship.toml' \
        > "$tmp/manifest.txt"

    validate_install_manifest "$tmp/manifest.txt" "$tmp/root"
    status=$?

    assert_status 0 "$status" "a CMake-shaped manifest of existing files should pass"
}

run_tests