#!/usr/bin/env bash
# Exports skwd-wall and/or plasma-audio-visualizer configs to user locations.
# Usage: export-plasma-config.sh [skwd|visualizer|all]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCEd:-${BASH_SOURCE[0]}}")" && pwd)"
CONFIG_DIR="$SCRIPT_DIR/plasma-config"
USER_CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}"

show_help() {
    cat << EOF
Usage: $(basename "$0") [TARGET...]

Exports and overwrites Plasma configurations to user locations.
If no target is specified, all configurations are exported.

Targets:
  skwd, skwd-wall, wall       Export skwd-wall-v2 config
  visualizer, audio           Export and sync plasma-audio-visualizer widgets
  all                         Export all configs (default)
  -h, --help                  Show this help message
EOF
}

export_skwd() {
    echo "Exporting skwd-wall-v2 configuration..."
    mkdir -p "$USER_CONFIG_DIR/skwd-wall-v2"
    cp "$CONFIG_DIR/skwd-wall-v2/config.json" "$USER_CONFIG_DIR/skwd-wall-v2/config.json"
    echo "✓ skwd-wall-v2 config exported to $USER_CONFIG_DIR/skwd-wall-v2/config.json"

    # Re-apply current wallpaper so skwd-paper-plasma never goes dark
    if command -v skwd-helm >/dev/null 2>&1; then
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

export_visualizer() {
    echo "Exporting and syncing plasma-audio-visualizer widgets..."
    python3 - "$CONFIG_DIR/plasma-audio-visualizer.conf" "$USER_CONFIG_DIR" << 'EOF'
import glob, os, re, sys

conf_path, config_dir = sys.argv[1], sys.argv[2]
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

candidate_files = glob.glob(os.path.join(config_dir, "*desktop-appletsrc"))
if not candidate_files:
    default_target = os.path.join(config_dir, "plasma-caelestia.desktop-appletsrc")
    with open(default_target, "w") as f:
        f.write("[Containments][1]\nplugin=org.kde.desktopcontainment\n")
    candidate_files = [default_target]

for appletsrc in candidate_files:
    with open(appletsrc, "r") as f:
        text = f.read()

    # Find desktop containment
    c_m = re.search(r"\[Containments\]\[(\d+)\](?:(?!\[Containments\]).)*?(?:plugin=org\.kde\.desktopcontainment|plugin=org\.kde\.plasma\.folder)", text, re.S)
    c_id = c_m.group(1) if c_m else "1"

    all_ids = set(int(x) for x in re.findall(rf"\[Containments\]\[{c_id}\]\[Applets\]\[(\d+)\]", text))

    viz_ids = []
    for a_id in sorted(all_ids):
        pattern = rf"\[Containments\]\[{c_id}\]\[Applets\]\[{a_id}\]\n(?:(?!^\[).)*?plugin=luisbocanegra\.audio\.visualizer"
        if re.search(pattern, text, re.S | re.M):
            viz_ids.append(a_id)

    # Ensure exactly 2 visualizer applet IDs: keep 2, allocate if missing, remove if extra
    if len(viz_ids) >= 2:
        keep_ids = viz_ids[:2]
        remove_ids = viz_ids[2:]
    else:
        keep_ids = list(viz_ids)
        remove_ids = []
        next_id = max(all_ids, default=0) + 1
        while len(keep_ids) < 2:
            while next_id in all_ids:
                next_id += 1
            keep_ids.append(next_id)
            all_ids.add(next_id)

    # 1. Remove extra visualizers
    for r_id in remove_ids:
        text = re.sub(rf"(?ms)^\[Containments\]\[{c_id}\]\[Applets\]\[{r_id}\].*?(?=(?:^\[|\Z))", "", text)
        text = re.sub(rf"(ItemGeometries[^\n]*?)Applet-{r_id}:[^;\n]+;", r"\1", text)

    # 2. Update/add the 2 visualizer applets
    for idx, a_id in enumerate(keep_ids, start=1):
        tmpl = templates[idx]
        g = tmpl["geom"]
        prefix = f"[Containments][{c_id}][Applets][{a_id}]"

        if g:
            def update_geom(m):
                line = m.group(0)
                if f"Applet-{a_id}:" in line:
                    return re.sub(rf"Applet-{a_id}:[^;\n]+", f"Applet-{a_id}:{g}", line)
                else:
                    return line.rstrip(";") + f";Applet-{a_id}:{g};"
            if re.search(r"ItemGeometries[^\n]*=", text):
                text = re.sub(r"ItemGeometries[^\n]*=[^\n]*", update_geom, text)
            else:
                c_head = f"[Containments][{c_id}]"
                g_lines = f"\nItemGeometries-1920x1080=Applet-{a_id}:{g};\nItemGeometriesHorizontal=Applet-{a_id}:{g};"
                text = text.replace(c_head, c_head + g_lines, 1)

        pattern = rf"(?ms)^\[Containments\]\[{c_id}\]\[Applets\]\[{a_id}\].*?(?=(?:^\[Containments\]\[{c_id}\]\[Applets\]\[(?!{a_id}\b)\d+\]|^\[Containments\]\[(?!{c_id}\b)\d+\]|\Z))"
        text = re.sub(pattern, "", text)

        replacement = tmpl["block"].replace(f"[Applet_{idx}]", prefix)
        text = text.rstrip() + f"\n\n{replacement}\n"

    with open(appletsrc, "w") as f:
        f.write(text)
    print(f"✓ Synced 2 visualizer widgets in {os.path.basename(appletsrc)} (kept: {keep_ids}, removed extras: {remove_ids})")
EOF

    if command -v qdbus6 >/dev/null 2>&1; then
        qdbus6 org.kde.KWin /KWin reconfigure 2>/dev/null || true
        qdbus6 org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell.evaluateScript "
            var ds = desktops();
            for (var i = 0; i < ds.length; i++) {
                var d = ds[i];
                var ws = d.widgets();
                var viz = [];
                for (var j = 0; j < ws.length; j++) {
                    if (ws[j].type === 'luisbocanegra.audio.visualizer') {
                        viz.push(ws[j]);
                    }
                }
                while (viz.length > 2) {
                    viz.pop().remove();
                }
                while (viz.length < 2) {
                    var w = d.addWidget('luisbocanegra.audio.visualizer');
                    if (w) viz.push(w);
                    else break;
                }
                for (var k = 0; k < viz.length; k++) {
                    viz[k].reloadConfig();
                }
            }
        " 2>/dev/null || true
    fi
}

# Parse targets
DO_SKWD=false
DO_VISUALIZER=false

if [[ $# -eq 0 ]]; then
    DO_SKWD=true
    DO_VISUALIZER=true
else
    for arg in "$@"; do
        case "$arg" in
            skwd|skwd-wall|wall|wallpaper|--skwd|--skwd-wall)
                DO_SKWD=true
                ;;
            visualizer|audio|audio-visualizer|plasma-audio-visualizer|--visualizer|--audio)
                DO_VISUALIZER=true
                ;;
            all|--all)
                DO_SKWD=true
                DO_VISUALIZER=true
                ;;
            -h|--help|help)
                show_help
                exit 0
                ;;
            *)
                echo "Unknown target: $arg" >&2
                show_help >&2
                exit 1
                ;;
        esac
    done
fi

if [[ "$DO_SKWD" == true ]]; then
    export_skwd
fi

if [[ "$DO_VISUALIZER" == true ]]; then
    export_visualizer
fi
