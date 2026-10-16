#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_DIR="$SCRIPT_DIR/plasma-config"
USER_CONFIG_DIR="${USER_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}}"

show_help() {
    cat << EOF
Usage: $(basename "$0") [TARGET [ACTION]]

Exports, configures, or removes Plasma configurations and widgets.
If no target is specified, all configurations are exported.

Targets:
  skwd, skwd-wall, wall       Export skwd-wall-v2 config
  visualizer [enable|disable|configure]
                              Manage plasma-audio-visualizer widgets
  all                         Export all configs (default)
  -h, --help                  Show this help message
EOF
}

apply_skwd_wallpaper() {
    if [[ -z "${CAELESTIA_TEST_MODE:-}" && "$USER_CONFIG_DIR" != "/tmp/"* ]] && command -v skwd-helm >/dev/null 2>&1; then
        local curr_wall
        curr_wall="$(skwd-helm current 2>/dev/null | cut -f2- || true)"
        if [[ -z "$curr_wall" ]]; then
            local cache_json="${XDG_CACHE_HOME:-$HOME/.cache}/skwd-wall-v2/last-applied.json"
            if [[ -f "$cache_json" ]]; then
                curr_wall="$(python3 -c "import json; print(json.load(open('$cache_json')).get('any', {}).get('path', ''))" 2>/dev/null || true)"
            fi
        fi
        if [[ -n "$curr_wall" && -f "$curr_wall" ]]; then
            skwd-helm apply "$curr_wall" >/dev/null 2>&1 || true
        fi
    fi
}

export_skwd() {
    echo "Exporting skwd-wall-v2 configuration..."
    mkdir -p "$USER_CONFIG_DIR/skwd-wall-v2"
    cp "$CONFIG_DIR/skwd-wall-v2/config.json" "$USER_CONFIG_DIR/skwd-wall-v2/config.json"
    echo "✓ skwd-wall-v2 config exported to $USER_CONFIG_DIR/skwd-wall-v2/config.json"
}

# Syncs N applet instances from template into appletsrc files.
# Emits NEEDS_SECOND_RECONFIGURE if newly allocated IDs need another cycle for geometry.
sync_kde_applets() {
    python3 - "$@" << 'PYEOF'
import glob, os, re, subprocess, sys

conf_path, config_dir, plugin_id, count_str, label = \
    sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5]
count = int(count_str)

with open(conf_path) as f:
    conf = f.read()

templates = {}
for block in re.split(r"(?m)(?=^\[Applet_\d+\]$)", conf):
    m = re.match(r"^\[Applet_(\d+)\]$", block.strip().splitlines()[0]) if block.strip() else None
    if not m:
        continue
    idx = int(m.group(1))
    geom_m = re.search(r"^geometry=(.*)$", block, re.M)
    geom = geom_m.group(1).strip() if geom_m else ""
    cleaned = re.sub(r"^geometry=.*\n?", "", block, flags=re.M).strip()
    templates[idx] = {"geom": geom, "block": cleaned}

candidate_files = []
shell_pkg = ""
try:
    shell_pkg = subprocess.check_output(
        ["qdbus6", "org.kde.plasmashell", "/PlasmaShell", "org.kde.PlasmaShell.shell"],
        text=True, stderr=subprocess.DEVNULL
    ).strip()
except Exception:
    pass

if shell_pkg:
    pkg_file = os.path.join(config_dir, f"plasma-{shell_pkg}-appletsrc")
    if os.path.exists(pkg_file):
        candidate_files.append(pkg_file)

for default_name in ["plasma-caelestia.desktop-appletsrc", "plasma-org.kde.plasma.desktop-appletsrc"]:
    p = os.path.join(config_dir, default_name)
    if os.path.exists(p) and p not in candidate_files:
        candidate_files.append(p)

for f in sorted(glob.glob(os.path.join(config_dir, "*desktop-appletsrc"))):
    if f not in candidate_files:
        candidate_files.append(f)

if not candidate_files:
    default_target = os.path.join(config_dir, "plasma-caelestia.desktop-appletsrc")
    with open(default_target, "w") as fh:
        fh.write("[Containments][1]\nplugin=org.kde.desktopcontainment\n")
    candidate_files = [default_target]

any_new_ids = False

for appletsrc in candidate_files:
    with open(appletsrc, "r") as f:
        text = f.read()

    c_m = re.search(
        r"\[Containments\]\[(\d+)\](?:(?!\[Containments\]).)*?"
        r"(?:plugin=org\.kde\.desktopcontainment|plugin=org\.kde\.plasma\.folder)",
        text, re.S
    )
    c_id = c_m.group(1) if c_m else "1"

    all_ids = set(int(x) for x in re.findall(
        rf"\[Containments\]\[{c_id}\]\[Applets\]\[(\d+)\]", text
    ))

    existing_ids = []
    for a_id in sorted(all_ids):
        pat = (
            rf"\[Containments\]\[{c_id}\]\[Applets\]\[{a_id}\]\n"
            rf"(?:(?!^\[).)*?plugin={re.escape(plugin_id)}"
        )
        if re.search(pat, text, re.S | re.M):
            existing_ids.append(a_id)

    if len(existing_ids) >= count:
        keep_ids = existing_ids[:count]
        remove_ids = existing_ids[count:]
        new_ids = []
    else:
        keep_ids = list(existing_ids)
        remove_ids = []
        new_ids = []
        next_id = max(all_ids, default=0) + 1
        while len(keep_ids) < count:
            while next_id in all_ids:
                next_id += 1
            keep_ids.append(next_id)
            new_ids.append(next_id)
            all_ids.add(next_id)
            next_id += 1

    if new_ids:
        any_new_ids = True

    sections = re.split(r"(?m)(?=^\[)", text)

    # 1. Update ItemGeometries in desktop containment
    c_header = f"[Containments][{c_id}]"
    c_found = False
    for i, sec in enumerate(sections):
        lines = sec.splitlines()
        if not lines or lines[0].strip() != c_header:
            continue
        c_found = True
        c_lines = lines[:]
        for idx, a_id in enumerate(keep_ids, start=1):
            g = templates.get(idx, {}).get("geom", "")
            if not g:
                continue
            for gk in ("ItemGeometries-1920x1080", "ItemGeometriesHorizontal"):
                found_key = False
                for li, line in enumerate(c_lines):
                    if line.startswith(f"{gk}="):
                        found_key = True
                        if f"Applet-{a_id}:" in line:
                            c_lines[li] = re.sub(
                                rf"Applet-{a_id}:[^;\n]+", f"Applet-{a_id}:{g}", line
                            )
                        else:
                            sep = "" if line.endswith("=") else ";"
                            c_lines[li] = line.rstrip(";") + f"{sep}Applet-{a_id}:{g};"
                if not found_key:
                    c_lines.append(f"{gk}=Applet-{a_id}:{g};")
        for r_id in remove_ids:
            for li, line in enumerate(c_lines):
                if line.startswith("ItemGeometries"):
                    c_lines[li] = re.sub(rf"Applet-{r_id}:[^;\n]+;?", "", line)
        sections[i] = "\n".join(c_lines) + "\n\n"
        break

    if not c_found:
        c_lines = [c_header, "plugin=org.kde.desktopcontainment"]
        for idx, a_id in enumerate(keep_ids, start=1):
            g = templates.get(idx, {}).get("geom", "")
            if g:
                c_lines.append(f"ItemGeometries-1920x1080=Applet-{a_id}:{g};")
                c_lines.append(f"ItemGeometriesHorizontal=Applet-{a_id}:{g};")
        sections.insert(0, "\n".join(c_lines) + "\n\n")

    # 2. Filter out removed and updated applet sections
    to_remove_pfx = [f"[Containments][{c_id}][Applets][{r}]" for r in remove_ids]
    to_update_pfx = [f"[Containments][{c_id}][Applets][{k}]" for k in keep_ids]

    filtered = []
    for sec in sections:
        lines = sec.splitlines()
        if not lines or not lines[0].startswith("["):
            if sec.strip():
                filtered.append(sec)
            continue
        hdr = lines[0].strip()
        if any(hdr.startswith(p) for p in to_remove_pfx):
            continue
        if any(hdr.startswith(p) for p in to_update_pfx):
            continue
        filtered.append(sec)

    # 3. Append template applet sections
    for idx, a_id in enumerate(keep_ids, start=1):
        tmpl = templates.get(idx, {})
        if not tmpl:
            continue
        prefix = f"[Containments][{c_id}][Applets][{a_id}]"
        block = tmpl["block"].replace(f"[Applet_{idx}]", prefix)
        filtered.append(block)

    clean = [s.strip() for s in filtered if s.strip()]
    text = "\n\n".join(clean) + "\n"
    with open(appletsrc, "w") as fh:
        fh.write(text)
    print(
        f"✓ Synced {count} {label} widget(s) in {os.path.basename(appletsrc)}"
        f" (kept: {keep_ids}, removed extras: {remove_ids})"
    )

if any_new_ids:
    print("NEEDS_SECOND_RECONFIGURE")
PYEOF
}

plasma_reconfigure() {
    if [[ -z "${CAELESTIA_TEST_MODE:-}" && "$USER_CONFIG_DIR" != "/tmp/"* ]] && command -v qdbus6 >/dev/null 2>&1; then
        qdbus6 org.kde.KWin /KWin reconfigure 2>/dev/null || true
        qdbus6 org.kde.plasmashell /PlasmaShell refreshCurrentShell 2>/dev/null || true
    fi
}

remove_kde_applets_live() {
    local plugin_id="$1"
    if [[ -z "${CAELESTIA_TEST_MODE:-}" && "$USER_CONFIG_DIR" != "/tmp/"* ]] && command -v qdbus6 >/dev/null 2>&1; then
        qdbus6 org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell.evaluateScript "
for (var i = 0; i < desktops().length; i++) {
    var ws = desktops()[i].widgets();
    for (var j = 0; j < ws.length; j++) {
        if (ws[j].type === '${plugin_id}') {
            ws[j].remove();
        }
    }
}
" >/dev/null 2>&1 || true
    fi
}

open_kde_applet_config() {
    local plugin_id="$1"
    if [[ -z "${CAELESTIA_TEST_MODE:-}" && "$USER_CONFIG_DIR" != "/tmp/"* ]] && command -v qdbus6 >/dev/null 2>&1; then
        qdbus6 org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell.evaluateScript "
var found = false;
for (var i = 0; i < desktops().length; i++) {
    var ws = desktops()[i].widgets();
    for (var j = 0; j < ws.length; j++) {
        if (ws[j].type === '${plugin_id}') {
            ws[j].showConfigurationInterface();
            found = true;
            break;
        }
    }
    if (found) break;
}
" >/dev/null 2>&1 || true
    fi
}

export_visualizer() {
    echo "Exporting and syncing plasma-audio-visualizer widgets..."
    local sync_output
    sync_output="$(sync_kde_applets \
        "$CONFIG_DIR/plasma-audio-visualizer.conf" \
        "$USER_CONFIG_DIR" \
        "luisbocanegra.audio.visualizer" \
        "2" \
        "plasma-audio-visualizer")"

    echo "$sync_output" | grep -v "^NEEDS_SECOND_RECONFIGURE$"

    plasma_reconfigure

    # Freshly added applets need a second reload for Plasma to apply geometry.
    if echo "$sync_output" | grep -q "^NEEDS_SECOND_RECONFIGURE$"; then
        sleep 2
        plasma_reconfigure
    fi
}

disable_visualizer() {
    echo "Disabling and removing plasma-audio-visualizer widgets..."
    remove_kde_applets_live "luisbocanegra.audio.visualizer"
    sync_kde_applets \
        "$CONFIG_DIR/plasma-audio-visualizer.conf" \
        "$USER_CONFIG_DIR" \
        "luisbocanegra.audio.visualizer" \
        "0" \
        "plasma-audio-visualizer" | grep -v "^NEEDS_SECOND_RECONFIGURE$"
    plasma_reconfigure
}

configure_visualizer() {
    echo "Opening configuration for plasma-audio-visualizer..."
    # Ensure visualizer is present first
    local count="0"
    if command -v qdbus6 >/dev/null 2>&1; then
        count="$(qdbus6 org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell.evaluateScript "
var c = 0;
for (var i = 0; i < desktops().length; i++) {
    var ws = desktops()[i].widgets();
    for (var j = 0; j < ws.length; j++) {
        if (ws[j].type === 'luisbocanegra.audio.visualizer') c++;
    }
}
print(c);
" 2>/dev/null || echo "0")"
    fi

    if [[ "$count" -eq 0 ]]; then
        export_visualizer
        sleep 1
    fi
    open_kde_applet_config "luisbocanegra.audio.visualizer"
}

DO_SKWD=false
VISUALIZER_ACTION=""

if [[ $# -eq 0 ]]; then
    DO_SKWD=true
    VISUALIZER_ACTION="enable"
else
    while [[ $# -gt 0 ]]; do
        case "$1" in
            skwd|skwd-wall|wall|wallpaper|--skwd|--skwd-wall)
                DO_SKWD=true
                shift
                ;;
            visualizer|audio|audio-visualizer|plasma-audio-visualizer|--visualizer|--audio)
                shift
                if [[ $# -gt 0 ]]; then
                    case "$1" in
                        enable|on|sync)
                            VISUALIZER_ACTION="enable"
                            shift
                            ;;
                        disable|off|remove)
                            VISUALIZER_ACTION="disable"
                            shift
                            ;;
                        configure|config|menu)
                            VISUALIZER_ACTION="configure"
                            shift
                            ;;
                        *)
                            VISUALIZER_ACTION="enable"
                            ;;
                    esac
                else
                    VISUALIZER_ACTION="enable"
                fi
                ;;
            enable-visualizer|--enable-visualizer)
                VISUALIZER_ACTION="enable"
                shift
                ;;
            disable-visualizer|--disable-visualizer)
                VISUALIZER_ACTION="disable"
                shift
                ;;
            configure-visualizer|--configure-visualizer)
                VISUALIZER_ACTION="configure"
                shift
                ;;
            all|--all)
                DO_SKWD=true
                VISUALIZER_ACTION="enable"
                shift
                ;;
            -h|--help|help)
                show_help
                exit 0
                ;;
            *)
                echo "Unknown argument: $1" >&2
                show_help >&2
                exit 1
                ;;
        esac
    done
fi

if [[ "$DO_SKWD" == true ]]; then
    export_skwd
fi

case "$VISUALIZER_ACTION" in
    enable)
        export_visualizer
        ;;
    disable)
        disable_visualizer
        ;;
    configure)
        configure_visualizer
        ;;
esac

if [[ "$DO_SKWD" == true ]]; then
    sleep 2
    apply_skwd_wallpaper
fi
