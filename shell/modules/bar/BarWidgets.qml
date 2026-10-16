pragma Singleton

import QtQuick

QtObject {
    id: root

    readonly property list<var> widgets: [
        { id: "logo", zone: "left" },
        { id: "workspaces", zone: "left" },
        { id: "greeter", zone: "left" },
        { id: "dock", zone: "middle" },
        { id: "tray", zone: "right" },
        { id: "updateIndicator", zone: "right" },
        { id: "github", zone: "right" },
        { id: "media", zone: "right" },
        { id: "clock", zone: "right" },
        { id: "statusIcons", zone: "right" },
        { id: "kbLayoutIndicator", zone: "right" },
        { id: "notificationsIndicator", zone: "right" },
        { id: "perfCpu", zone: "right" },
        { id: "perfMemory", zone: "right" },
        { id: "perfStorage", zone: "right" },
        { id: "perfNetwork", zone: "right" },
        { id: "perfGpu", zone: "right" },
        { id: "perfBattery", zone: "right" },
        { id: "showDesktop", zone: "right" },
        { id: "power", zone: "right" },
        { id: "notes", zone: "right" },
        { id: "performance", zone: "right" },
        { id: "weather", zone: "right" },
        { id: "shortcuts", zone: "right" },
        { id: "screencapture", zone: "right" },
        { id: "keepawake", zone: "right" }
    ]

    readonly property var byId: {
        const widgets = {};
        root.widgets.forEach((widget, index) => widgets[widget.id] = Object.assign({ index: index }, widget));
        return widgets;
    }

    readonly property var ids: root.widgets.map(widget => widget.id)

    function widget(id: string): var {
        return root.byId[id] ?? null;
    }

    function isKnown(id: string): bool {
        return root.byId[id] !== undefined;
    }

    function entriesOf(node: var): var {
        return node.entries ?? [];
    }

    function zoneEntries(entries: var, zone: string): var {
        return (entries ?? []).filter(entry => entry.enabled
            && entry.id !== "spacer"
            && root.isKnown(entry.id)
            && (zone === "left" ? !entry.zone || entry.zone === "left" : entry.zone === zone));
    }

    function isEnabled(node: var, id: string): bool {
        for (const entry of root.entriesOf(node)) {
            if (entry.id === id)
                return entry.enabled === true;
        }
        return false;
    }

    function zoneFor(node: var, id: string): string {
        for (const entry of root.entriesOf(node)) {
            if (entry.id === id)
                return entry.zone ?? root.widget(id)?.zone ?? "left";
        }
        return root.widget(id)?.zone ?? "left";
    }

    function setEnabled(node: var, id: string, enabled: bool): void {
        const entries = root.entriesOf(node);
        const zone = root.widget(id)?.zone ?? "left";
        const next = [];
        let found = false;

        for (const entry of entries) {
            if (entry.id !== id || found) {
                next.push(entry);
                continue;
            }

            found = true;
            next.push({ id: id, enabled: enabled, zone: entry.zone ?? zone });
        }

        if (!found)
            next.splice(root.indexFor(entries, id), 0, { id: id, enabled: enabled, zone: zone });

        node.entries = next;
    }

    function replaceAll(node: var, entries: var): void {
        node.entries = entries.slice();
    }

    function resetToDefaults(node: var): void {
        node.resetOption("entries");
    }

    function indexFor(entries: var, id: string): int {
        const target = root.widget(id);
        if (target === null)
            return entries.length;

        for (let i = entries.length - 1; i >= 0; i--) {
            const other = root.widget(entries[i].id);
            if (other !== null && other.index < target.index)
                return i + 1;
        }
        return 0;
    }
}
