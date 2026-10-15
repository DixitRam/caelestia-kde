#!/usr/bin/env bash

set -uo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/helpers.sh"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_SHELL="$REPO_ROOT/scripts/08-build-shell.sh"

# The guard decides from the checkout's own git state, so the test drives the
# real functions against throwaway repositories rather than stubbing git.
GUARD_SOURCE="$(extract_function "$BUILD_SHELL" shell_release_tag; extract_function "$BUILD_SHELL" checkout_may_use_prebuilt)"

if [[ -z "$GUARD_SOURCE" ]]; then
    fail "could not find shell_release_tag/checkout_may_use_prebuilt in scripts/08-build-shell.sh"
    run_tests
    exit 1
fi

# A checkout of main with origin/main pointing at the same commit, which is what
# install.sh leaves behind, plus a version.env naming the released version.
seed_checkout() {
    local dir="$1" revision="${2:-v1.0.0}"
    mkdir -p "$dir/.github"
    printf 'VERSION=%s\n' "$revision" > "$dir/.github/version.env"
    git init -q "$dir"
    git -C "$dir" symbolic-ref HEAD refs/heads/main
    git -C "$dir" config user.email test@example.com
    git -C "$dir" config user.name test
    git -C "$dir" add -A
    git -C "$dir" commit -qm "release $revision"
    git -C "$dir" update-ref refs/remotes/origin/main HEAD
}

allows_prebuilt() {
    local dir="$1" status
    BUNDLE_DIR="$dir" bash -c "$GUARD_SOURCE
checkout_may_use_prebuilt" >/dev/null 2>&1
    status=$?
    [[ $status -eq 0 ]]
}

test_main_sitting_on_its_remote_tip_may_use_the_prebuilt() {
    local repo
    repo="$(new_tmpdir)/repo"
    seed_checkout "$repo"

    allows_prebuilt "$repo" || fail "a clean main checkout should be allowed to use the prebuilt"
}

test_local_commits_on_main_build_locally() {
    local repo
    repo="$(new_tmpdir)/repo"
    seed_checkout "$repo"
    printf 'local\n' > "$repo/local.txt"
    git -C "$repo" add -A
    git -C "$repo" commit -qm "unreleased local work"

    if allows_prebuilt "$repo"; then
        fail "main with commits of its own must not be overwritten by the prebuilt"
    fi
}

test_main_behind_its_remote_builds_locally() {
    local repo
    repo="$(new_tmpdir)/repo"
    seed_checkout "$repo"
    # The remote moves on while the checkout stays behind, as a stale local main
    # does until something pulls it.
    git -C "$repo" commit -q --allow-empty -m "released later"
    git -C "$repo" update-ref refs/remotes/origin/main HEAD
    git -C "$repo" reset -q --hard HEAD~1

    if allows_prebuilt "$repo"; then
        fail "a stale main checkout must not be treated as the released revision"
    fi
}

test_another_branch_builds_locally() {
    local repo
    repo="$(new_tmpdir)/repo"
    seed_checkout "$repo"
    git -C "$repo" checkout -q -b dev

    if allows_prebuilt "$repo"; then
        fail "dev must never install the prebuilt shell"
    fi
}

test_the_released_tag_may_use_the_prebuilt() {
    local repo
    repo="$(new_tmpdir)/repo"
    seed_checkout "$repo"
    # An update pinned to a version: the checkout is the tag, not main's tip.
    git -C "$repo" commit -q --allow-empty -m "next release"
    git -C "$repo" tag v2.0.0
    printf 'VERSION=v2.0.0\n' > "$repo/.github/version.env"

    allows_prebuilt "$repo" || fail "a checkout sitting on the released tag should be allowed to use the prebuilt"
}

test_a_version_that_is_not_the_tag_builds_locally() {
    local repo
    repo="$(new_tmpdir)/repo"
    seed_checkout "$repo"
    git -C "$repo" commit -q --allow-empty -m "next release"
    git -C "$repo" tag v2.0.0
    # version.env still names the older release, so the prebuilt would be older
    # than the tree it replaces.
    printf 'VERSION=v1.0.0\n' > "$repo/.github/version.env"

    if allows_prebuilt "$repo"; then
        fail "a version.env that does not match the checked-out revision must build locally"
    fi
}

test_a_checkout_without_a_version_builds_locally() {
    local repo
    repo="$(new_tmpdir)/repo"
    seed_checkout "$repo"
    rm -f "$repo/.github/version.env"

    if allows_prebuilt "$repo"; then
        fail "without a version there is no release artifact to fetch"
    fi
}

test_a_tree_that_is_not_a_checkout_builds_locally() {
    local dir
    dir="$(new_tmpdir)/tree"
    mkdir -p "$dir/.github"
    printf 'VERSION=v1.0.0\n' > "$dir/.github/version.env"

    if allows_prebuilt "$dir"; then
        fail "a source tree without git history must build locally"
    fi
}

fetch_function() {
    local name="$1" source
    source="$(awk -v fn="$name" '
        $0 ~ "^" fn "\\(\\) \\{" { capture = 1 }
        capture { print }
        capture && $0 == "}" { exit }
    ' "$REPO_ROOT/scripts/setup.sh")"
    if [[ -z "$source" ]]; then
        fail "could not find $name in scripts/setup.sh"
    fi
    printf '%s\n' "$source"
}

FETCH_SOURCE="$(fetch_function fetch_installer)"
MATCH_SOURCE="$(fetch_function installer_matches_checkout)"

fetch_with_status() {
    local status="$1" arch="${2:-x86_64}"
    BIN="$(mktemp -d)/caelestia-install"
    bash -c "
        BIN='$BIN'
        tui_version() { echo '9.9.9'; }
        release_tag() { echo 'v9.9.9'; }
        fetch_asset() { : > \"\$2\"; return 0; }
        verify_download() { return $status; }
        uname() { echo '$arch'; }
        $FETCH_SOURCE
        fetch_installer
    "
}

test_a_verified_download_succeeds_quietly() {
    local out err status

    fetch_with_status 0 >/dev/null 2>&1
    status=$?
    if [[ $status -ne 0 ]]; then
        fail "a verified download should succeed, got status $status"
    fi

    out="$(fetch_with_status 0 2>/dev/null)"
    if [[ -n "$out" ]]; then
        fail "a verified download must print nothing to stdout, got: $out"
    fi

    err="$(fetch_with_status 0 2>&1 >/dev/null)"
    if [[ -n "$err" ]]; then
        fail "a verified download must print nothing to stderr, got: $err"
    fi
}

test_a_checksum_mismatch_is_fatal() {
    local err status

    fetch_with_status 1 >/dev/null 2>&1
    status=$?
    if [[ $status -eq 0 ]]; then
        fail "a mismatched download must fail the install"
    fi

    err="$(fetch_with_status 1 2>&1 >/dev/null)"
    case "$err" in
        *"checksum"*) ;;
        *) fail "a mismatched download should say so on stderr; got: $err" ;;
    esac
}

test_a_release_without_a_checksum_is_fatal() {
    local err status

    fetch_with_status 2 >/dev/null 2>&1
    status=$?
    if [[ $status -eq 0 ]]; then
        fail "a release with no published checksum must not be trusted"
    fi

    err="$(fetch_with_status 2 2>&1 >/dev/null)"
    case "$err" in
        *"no checksum"*) ;;
        *) fail "the reason should name the missing checksum; got: $err" ;;
    esac
}

test_an_architecture_with_no_release_is_reported() {
    local err status

    fetch_with_status 0 riscv64 >/dev/null 2>&1
    status=$?
    if [[ $status -eq 0 ]]; then
        fail "an architecture with no published binary must not be attempted"
    fi

    err="$(fetch_with_status 0 riscv64 2>&1 >/dev/null)"
    case "$err" in
        *"no prebuilt installer"*) ;;
        *) fail "the unsupported architecture should be named; got: $err" ;;
    esac
}

test_a_binary_from_another_version_is_refused() {
    local dir binary

    dir="$(mktemp -d)"
    binary="$dir/caelestia-install"
    printf '#!/bin/sh\necho 1.2.3\n' > "$binary"
    chmod +x "$binary"

    if bash -c "
        BIN='$binary'
        tui_version() { echo '9.9.9'; }
        $MATCH_SOURCE
        installer_matches_checkout
    " >/dev/null 2>&1; then
        fail "a binary that disagrees with the checkout must be refused"
    fi

    printf '#!/bin/sh\necho 9.9.9\n' > "$binary"
    chmod +x "$binary"

    if ! bash -c "
        BIN='$binary'
        tui_version() { echo '9.9.9'; }
        $MATCH_SOURCE
        installer_matches_checkout
    " >/dev/null 2>&1; then
        fail "a binary that matches the checkout must be accepted"
    fi
}
run_tests
