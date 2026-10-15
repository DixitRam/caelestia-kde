#!/usr/bin/env bash

set -uo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/helpers.sh"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
UNINSTALL_SCRIPT="$REPO_ROOT/uninstall.sh"
BASH_BIN="$(command -v bash || printf '/bin/bash')"

# The stubs that stand in for the two config tools are Python, so this file skips
# rather than fails where python3 is absent. Only bash is required to run the suite.
have_python() {
    if command -v python3 >/dev/null 2>&1; then
        return 0
    fi
    skip_test "python3 not installed"
    return 1
}

RULES_FILE="kwinrulesrc"
# The index a fresh file has to end up with: our three groups, in the order the
# installer writes them, and no other name.
FRESH_INDEX="caelestia-opacity,caelestia-dialogs,caelestia-pip"

SANDBOX=""
STUB_DIR=""
CALLS=""
FAKE=""
OUTPUT=""
STATUS=0

# The two config tools share one plain ini file: kwriteconfig6 rewrites it and
# kreadconfig6 answers from it, so the revert's own reads and writes are exercised
# for real instead of being answered from a fixture.
write_fake_config_tools() {
    cat > "$FAKE" <<'PY'
"""Minimal kwinrulesrc reader and writer, standing in for the two config tools."""

import os
import sys
from pathlib import Path


def arguments(argv):
    values = {"file": "kwinrulesrc", "group": "", "key": ""}
    positional = []
    index = 0
    while index < len(argv):
        argument = argv[index]
        if argument in ("--file", "--group", "--key") and index + 1 < len(argv):
            values[argument[2:]] = argv[index + 1]
            index += 2
        else:
            positional.append(argument)
            index += 1
    return values, positional


def read(path):
    sections = {}
    current = None
    if not path.is_file():
        return sections
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith(("#", ";")):
            continue
        if line.startswith("[") and line.endswith("]"):
            current = line[1:-1]
            sections.setdefault(current, {})
        elif current is not None and "=" in line:
            key, _, value = line.partition("=")
            sections[current][key.strip()] = value.strip()
    return sections


def write(path, sections):
    lines = []
    for name, entries in sections.items():
        # KConfig does not keep a group that has no keys left in it.
        if not entries:
            continue
        lines.append("[%s]" % name)
        for key, value in entries.items():
            lines.append("%s=%s" % (key, value))
        lines.append("")
    path.write_text("\n".join(lines))


def main():
    mode = sys.argv[1]
    values, positional = arguments(sys.argv[2:])
    path = Path(values["file"])
    if not path.is_absolute():
        path = Path(os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config"))) / path
    sections = read(path)

    if mode == "get":
        value = sections.get(values["group"], {}).get(values["key"])
        if value is None:
            return 1
        print(value)
        return 0

    if mode == "delete":
        entries = sections.get(values["group"])
        if entries is not None:
            entries.pop(values["key"], None)
            if not entries:
                del sections[values["group"]]
        write(path, sections)
        return 0

    sections.setdefault(values["group"], {})[values["key"]] = positional[0]
    write(path, sections)
    return 0


sys.exit(main())
PY
}

setup_sandbox() {
    SANDBOX="$(new_tmpdir)"
    STUB_DIR="$SANDBOX/bin"
    CALLS="$SANDBOX/calls.log"
    FAKE="$SANDBOX/kwinrules.py"
    mkdir -p "$STUB_DIR"
    : > "$CALLS"

    write_fake_config_tools

    stub_bin "$STUB_DIR" kreadconfig6 'exec python3 "$KW_RULES" get "$@"'
    stub_bin "$STUB_DIR" kwriteconfig6 "case \"\$*\" in
    *--delete*) exec python3 \"\$KW_RULES\" delete \"\$@\" ;;
esac
exec python3 \"\$KW_RULES\" set \"\$@\""
    recording_stub "$STUB_DIR" qdbus6 "$CALLS"
}

# The stubs write one plain ini file, so a value is read back straight from it
# rather than through the config tools: the assertions are about the file.
ini_value() {
    local file="$1" group="$2" key="$3"
    awk -v group="$group" -v key="$key" '
        $0 == "[" group "]" { inside = 1; next }
        /^\[/ { inside = 0 }
        inside && index($0, key "=") == 1 { print substr($0, length(key) + 2); exit }
    ' "$file"
}

# kwinrulesrc as the installer leaves it, with the index the installer writes. No
# Order key, which is what KWin 6.7 writes: only the list and its count.
write_installed_fixture() {
    local index="$1" count="$2"
    cat > "$SANDBOX/$RULES_FILE" <<EOF
[General]
rules=$index
count=$count

[user-rule]
Description=Keep this window put
wmclass=keepme
wmclassmatch=1
placement=2
placementrule=2

[caelestia-opacity]
Description=Caelestia: window opacity
types=33
opacityinactive=95
opacityinactiverule=2

[caelestia-dialogs]
Description=Caelestia: center dialogs
types=32
placement=5
placementrule=2

[caelestia-pip]
Description=Caelestia: pin picture-in-picture
title=Picture(-| )in(-| )[Pp]icture
titlematch=3
above=true
aboverule=2
EOF
}

# uninstall.sh runs from top to bottom and cannot be sourced, so its revert of the window
# rules is cut out by its own first and last lines - the first delete through the line that
# reports it - and run against the same stubs the installer step is tested with.
extract_uninstall_revert() {
    awk '
        /^kwriteconfig6 .*--group "caelestia-opacity"/ { inside = 1 }
        inside { print }
        inside && /^ok "Removed the Caelestia window rules/ { exit }
    ' "$UNINSTALL_SCRIPT"
}

run_uninstall_revert() {
    local revert="$SANDBOX/revert.sh"
    {
        printf 'set -uo pipefail\n'
        printf 'ok() { printf "  [OK]    %%s\\n" "$*"; }\n'
        extract_uninstall_revert
    } > "$revert"
    : > "$CALLS"
    OUTPUT="$(cd "$SANDBOX" && env PATH="$STUB_DIR:$PATH" XDG_CONFIG_HOME="$SANDBOX" KW_CALLS="$CALLS" KW_RULES="$FAKE" "$BASH_BIN" "$revert" 2>&1)"
    STATUS=$?
}

# uninstall.sh strips our names out of the index and leaves the user's rules alone:
# the list is theirs, and deleting a name that is not ours would silently disable a
# rule they wrote.
test_the_revert_keeps_the_users_names_and_strips_ours() {
    have_python || return 0
    setup_sandbox
    write_installed_fixture "user-rule,$FRESH_INDEX" 4

    run_uninstall_revert

    local file="$SANDBOX/$RULES_FILE"
    assert_status 0 "$STATUS" "the revert should succeed"
    assert_contains "$OUTPUT" "Removed the Caelestia window rules from kwinrulesrc" \
        "the revert should report what it removed"
    assert_eq "user-rule" "$(ini_value "$file" General rules)" "only our names should leave the list"
    assert_eq "1" "$(ini_value "$file" General count)" "and count should follow the names that are left"
    assert_eq "keepme" "$(ini_value "$file" user-rule wmclass)" "the user's own rule should survive"
    assert_not_contains "$(cat "$file")" "caelestia-" "every group of ours should be gone"
}

# With no name left the file has to look like it did before the install: an index
# that names nothing, and a count of nothing, are both absent rather than empty.
test_the_revert_removes_the_index_when_no_names_are_left() {
    have_python || return 0
    setup_sandbox
    write_installed_fixture "$FRESH_INDEX" 3

    run_uninstall_revert

    local file="$SANDBOX/$RULES_FILE"
    assert_status 0 "$STATUS" "the revert should succeed"
    assert_eq "" "$(ini_value "$file" General rules)" "the index should be deleted"
    assert_eq "" "$(ini_value "$file" General count)" "and the count with it"
    assert_not_contains "$(cat "$file")" "[General]" "the General group should be gone once it is empty"
    assert_not_contains "$(cat "$file")" "caelestia-" "with none of our groups left"
}

run_tests
