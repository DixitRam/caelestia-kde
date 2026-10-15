#!/usr/bin/env bash
if [[ -z "${CAELESTIA_INSTALLER_BIN_SOURCED:-}" ]]; then
CAELESTIA_INSTALLER_BIN_SOURCED=1

caelestia_installer_bin() {
    if [[ -n "${CAELESTIA_INSTALL_BIN:-}" && -x "${CAELESTIA_INSTALL_BIN}" ]]; then
        printf '%s\n' "$CAELESTIA_INSTALL_BIN"
        return 0
    fi

    local bundle="${BUNDLE_DIR:-}"
    if [[ -z "$bundle" ]]; then
        bundle="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." 2>/dev/null && pwd)" || bundle=""
    fi

    local candidate
    if [[ -n "$bundle" ]]; then
        for candidate in \
            "$bundle/caelestia-install" \
            "$bundle/installer/target/release/caelestia-install" \
            "$bundle/installer/target/debug/caelestia-install"; do
            if [[ -x "$candidate" ]]; then
                printf '%s\n' "$candidate"
                return 0
            fi
        done
    fi

    candidate="$(command -v caelestia-install 2>/dev/null || true)"
    if [[ -n "$candidate" && -x "$candidate" ]]; then
        printf '%s\n' "$candidate"
        return 0
    fi

    for candidate in \
        "${HOME:-}/.local/bin/caelestia-install" \
        /usr/bin/caelestia-install \
        /usr/lib/caelestia/caelestia-install; do
        if [[ -n "$candidate" && -x "$candidate" ]]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done

    return 1
}

caelestia_step_script() {
    local id="$1" bundle="${2:-${BUNDLE_DIR:-}}"
    [[ -n "$bundle" ]] || return 1
    printf '%s\n' "$bundle/scripts/$id.sh"
}

caelestia_run_step() {
    local id="$1"
    shift
    local bundle="${BUNDLE_DIR:-}"
    local bin script

    if bin="$(caelestia_installer_bin)"; then
        if [[ -n "$bundle" ]]; then
            "$bin" step "$id" --bundle "$bundle" "$@"
        else
            "$bin" step "$id" "$@"
        fi
        return $?
    fi

    script="$(caelestia_step_script "$id" "$bundle")" || true
    if [[ -n "$script" && -f "$script" ]]; then
        BUNDLE_DIR="$bundle" bash "$script" "$@"
        return $?
    fi

    printf '[ERR]   cannot run the %s step: no installer binary and no %s\n' "$id" "${script:-script}" >&2
    return 127
}

fi
