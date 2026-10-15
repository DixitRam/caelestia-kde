#!/usr/bin/env bash
if [[ -z "${CAELESTIA_INSTALLER_BIN_SOURCED:-}" ]]; then
CAELESTIA_INSTALLER_BIN_SOURCED=1

caelestia_installer_url() {
    local arch bundle tag version
    arch="$(uname -m)"
    case "$arch" in
        x86_64 | aarch64) ;;
        *) return 1 ;;
    esac

    bundle="${BUNDLE_DIR:-}"
    if [[ -z "$bundle" ]]; then
        bundle="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." 2>/dev/null && pwd)" || return 1
    fi

    version="$(tr -d '[:space:]' < "$bundle/installer/data/tui.version" 2>/dev/null || true)"
    tag="$(sed -nE 's/^[[:space:]]*VERSION=//p' "$bundle/.github/version.env" 2>/dev/null | tr -d '[:space:]')"
    [[ -n "$version" && -n "$tag" ]] || return 1

    printf 'https://github.com/ladybug-me/caelestia-kde/releases/download/%s/caelestia-install-%s-v%s\n' \
        "$tag" "$arch" "$version"
}

caelestia_cached_installer() {
    printf '%s\n' "${XDG_CACHE_HOME:-${HOME:-}/.cache}/caelestia/caelestia-install"
}

caelestia_fetch_installer() {
    local bin url sidecar expected actual
    bin="$(caelestia_cached_installer)"
    if [[ -x "$bin" ]]; then
        printf '%s\n' "$bin"
        return 0
    fi

    url="$(caelestia_installer_url)" || return 1
    command -v curl >/dev/null 2>&1 || return 1
    command -v sha256sum >/dev/null 2>&1 || return 1

    mkdir -p "$(dirname -- "$bin")" || return 1
    rm -f "$bin.part"
    if ! curl -fsSL --connect-timeout 10 --max-time 60 "$url" -o "$bin.part" 2>/dev/null; then
        rm -f "$bin.part"
        return 1
    fi

    sidecar="$(mktemp)" || { rm -f "$bin.part"; return 1; }
    if ! curl -fsSL --connect-timeout 10 "$url.sha256" -o "$sidecar" 2>/dev/null; then
        rm -f "$sidecar" "$bin.part"
        return 1
    fi
    expected="$(cut -d' ' -f1 < "$sidecar" | tr -d '[:space:]')"
    rm -f "$sidecar"

    actual="$(sha256sum "$bin.part" | cut -d' ' -f1)"
    if [[ -z "$expected" || "$expected" != "$actual" ]]; then
        printf '[ERR]   the downloaded installer failed its checksum; keeping the step scripts\n' >&2
        rm -f "$bin.part"
        return 1
    fi

    chmod +x "$bin.part" || { rm -f "$bin.part"; return 1; }
    mv -f "$bin.part" "$bin" || { rm -f "$bin.part"; return 1; }
    printf '%s\n' "$bin"
}

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

    # A packaged install ships the step scripts but not the installer, so the
    # published binary is fetched once and cached. CAELESTIA_NO_INSTALLER_FETCH
    # keeps a machine offline: the step scripts then run as they always have.
    if [[ "${CAELESTIA_NO_INSTALLER_FETCH:-0}" == "1" ]]; then
        return 1
    fi
    caelestia_fetch_installer
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
