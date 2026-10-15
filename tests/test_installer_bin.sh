#!/usr/bin/env bash

set -uo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/helpers.sh"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RESOLVER="$REPO_ROOT/scripts/lib/installer-bin.sh"

seed_bundle() {
    local dir="$1"
    mkdir -p "$dir/installer/data" "$dir/.github" "$dir/scripts/lib"
    printf '0.1.0\n' > "$dir/installer/data/tui.version"
    printf 'VERSION=v2.5.1\n' > "$dir/.github/version.env"
}

# The stub answers both requests the resolver makes: the artifact, then the
# checksum published beside it. It records what it handed out so the sidecar
# describes those very bytes, and it can be told to refuse or to lie.
stub_curl() {
    local dir="$1" behaviour="$2"
    mkdir -p "$dir"
    cat > "$dir/curl" <<EOF
#!/bin/sh
record="$dir/record"
dest=""
url=""
while [ "\$#" -gt 0 ]; do
    case "\$1" in
        -o) shift; dest="\$1" ;;
        http*) url="\$1" ;;
    esac
    shift
done
if [ "$behaviour" = "fail" ]; then
    exit 22
fi
case "\$url" in
    *.sha256)
        if [ "$behaviour" = "mismatch" ]; then
            printf '%064d\n' 0 > "\$dest"
        else
            sha256sum "\$(cat "\$record")" | cut -d' ' -f1 > "\$dest"
        fi
        ;;
    *)
        printf '#!/bin/sh\nexit 0\n' > "\$dest"
        printf '%s' "\$dest" > "\$record"
        ;;
esac
exit 0
EOF
    chmod +x "$dir/curl"
}

resolver() {
    local bundle="$1" home="$2" path="$3"
    shift 3
    HOME="$home" XDG_CACHE_HOME="$home/.cache" BUNDLE_DIR="$bundle" PATH="$path" \
        bash -c "source '$RESOLVER'; $*" 2>"$home/stderr"
}

cache_path() {
    printf '%s\n' "$1/.cache/caelestia/caelestia-install"
}

test_a_binary_in_the_bundle_wins() {
    local dir bundle fake out
    dir="$(new_tmpdir)"
    bundle="$dir/bundle"
    seed_bundle "$bundle"
    mkdir -p "$dir/home"
    fake="$bundle/caelestia-install"
    printf '#!/bin/sh\nexit 0\n' > "$fake"
    chmod +x "$fake"

    out="$(resolver "$bundle" "$dir/home" "$PATH" caelestia_installer_bin)"
    assert_eq "$fake" "$out" "the checkout's own binary is used"
    assert_file_missing "$(cache_path "$dir/home")"
}

test_the_published_url_names_the_release_and_the_arch() {
    local dir bundle url
    dir="$(new_tmpdir)"
    bundle="$dir/bundle"
    seed_bundle "$bundle"
    mkdir -p "$dir/home"

    url="$(resolver "$bundle" "$dir/home" "$PATH" caelestia_installer_url)"
    assert_contains "$url" "/releases/download/v2.5.1/" "the release tag comes from version.env"
    assert_contains "$url" "caelestia-install-$(uname -m)-v0.1.0" "the asset name carries the arch and the installer version"
}

test_the_binary_is_fetched_into_the_cache_when_none_is_local() {
    local dir bundle out cached
    dir="$(new_tmpdir)"
    bundle="$dir/bundle"
    seed_bundle "$bundle"
    mkdir -p "$dir/home"
    stub_curl "$dir/bin" ok

    out="$(resolver "$bundle" "$dir/home" "$dir/bin:$PATH" caelestia_installer_bin)"
    cached="$(cache_path "$dir/home")"
    assert_eq "$cached" "$out" "the cached copy is returned"
    assert_file_exists "$cached"
}

test_a_checksum_mismatch_is_refused() {
    local dir bundle out cached
    dir="$(new_tmpdir)"
    bundle="$dir/bundle"
    seed_bundle "$bundle"
    mkdir -p "$dir/home"
    stub_curl "$dir/bin" mismatch

    out="$(resolver "$bundle" "$dir/home" "$dir/bin:$PATH" caelestia_installer_bin)"
    cached="$(cache_path "$dir/home")"
    assert_eq "" "$out" "a mismatch yields no binary"
    assert_file_missing "$cached"
    assert_file_contains "$dir/home/stderr" "failed its checksum" "the refusal is explained"
}

test_an_unreachable_release_is_not_fatal() {
    local dir bundle out
    dir="$(new_tmpdir)"
    bundle="$dir/bundle"
    seed_bundle "$bundle"
    mkdir -p "$dir/home"
    stub_curl "$dir/bin" fail

    out="$(resolver "$bundle" "$dir/home" "$dir/bin:$PATH" caelestia_installer_bin)"
    assert_eq "" "$out" "no binary is reported"
}

test_fetching_can_be_switched_off() {
    local dir bundle out
    dir="$(new_tmpdir)"
    bundle="$dir/bundle"
    seed_bundle "$bundle"
    mkdir -p "$dir/home"
    stub_curl "$dir/bin" ok

    out="$(CAELESTIA_NO_INSTALLER_FETCH=1 resolver "$bundle" "$dir/home" "$dir/bin:$PATH" caelestia_installer_bin)"
    assert_eq "" "$out" "the fetch is skipped"
    assert_file_missing "$(cache_path "$dir/home")"
}

test_a_step_script_runs_when_no_binary_can_be_found() {
    local dir bundle out
    dir="$(new_tmpdir)"
    bundle="$dir/bundle"
    seed_bundle "$bundle"
    mkdir -p "$dir/home"
    printf '#!/usr/bin/env bash\nprintf "ran %%s\\n" "$1"\n' > "$bundle/scripts/02a-submodules.sh"
    chmod +x "$bundle/scripts/02a-submodules.sh"

    out="$(CAELESTIA_NO_INSTALLER_FETCH=1 resolver "$bundle" "$dir/home" "$PATH" 'caelestia_run_step 02a-submodules')"
    assert_contains "$out" "ran " "the shipped script is the fallback"
}

test_a_step_with_neither_binary_nor_script_says_so() {
    local dir bundle status
    dir="$(new_tmpdir)"
    bundle="$dir/bundle"
    seed_bundle "$bundle"
    mkdir -p "$dir/home"

    CAELESTIA_NO_INSTALLER_FETCH=1 resolver "$bundle" "$dir/home" "$PATH" 'caelestia_run_step 09-system-tweaks' > "$dir/out" 2>&1
    status=$?
    assert_status 127 "$status" "a missing step is reported as unrunnable"
    assert_file_contains "$dir/home/stderr" "cannot run the 09-system-tweaks step" "the message names the step"
}

run_tests
