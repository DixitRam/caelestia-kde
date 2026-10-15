pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.components
import qs.services
import qs.utils

LazyLoader {
    id: loader

    property list<string> cwd: ["Home"]
    property string filterLabel: qsTr("All files")
    property list<string> filters: ["*"]
    property bool selectFolder: false
    property string title: selectFolder ? qsTr("Select a folder") : qsTr("Select a file")
    property bool countedOpen: false

    signal accepted(path: string)
    signal rejected

    function open(): void {
        activeAsync = true;
    }

    function close(): void {
        rejected();
    }

    onAccepted: activeAsync = false
    onRejected: activeAsync = false

    onActiveAsyncChanged: {
        if (activeAsync !== countedOpen) {
            Visibilities.openDialogs += activeAsync ? 1 : -1;
            countedOpen = activeAsync;
        }
    }
    Component.onDestruction: {
        if (countedOpen)
            Visibilities.openDialogs -= 1;
    }

    FloatingWindow {
        id: root

        property bool selectFolder: loader.selectFolder
        property list<string> cwd: loader.cwd
        property string filterLabel: loader.filterLabel
        property list<string> filters: loader.filters

        readonly property bool selectionValid: {
            if (loader.selectFolder) return true;
            const file = folderContents.currentItem?.modelData;
            return (file && !file.isDir && (filters.includes("*") || filters.some(f => f.toLowerCase() === file.suffix.toLowerCase()))) ?? false;
        }

        function accepted(path: string): void {
            loader.accepted(path);
        }

        function rejected(): void {
            loader.rejected();
        }

        // Resolve the cursor and hand it up: the folder under it in folder mode, the
        // folder being browsed when it is on a file, and the file otherwise. The view
        // asks for this instead of resolving the selection itself.
        function acceptSelection(): void {
            const entry = folderContents.currentItem?.modelData;
            if (!selectFolder) {
                if (entry)
                    accepted(entry.path);
                return;
            }

            if (entry?.isDir) {
                accepted(entry.path);
                return;
            }

            const here = cwd[0] === "Home" ? Paths.home + (cwd.length > 1 ? `/${cwd.slice(1).join("/")}` : "") : cwd.join("/");
            accepted(here);
        }

        implicitWidth: 1000
        implicitHeight: 600
        minimumSize.width: 400
        minimumSize.height: 300
        color: Colours.tPalette.m3surface
        surfaceFormat.opaque: false
        title: loader.title

        onVisibleChanged: {
            if (!visible)
                rejected();
        }

        RowLayout {
            anchors.fill: parent

            spacing: 0

            Sidebar {
                Layout.fillHeight: true
                dialog: root
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true

                spacing: 0

                HeaderBar {
                    Layout.fillWidth: true
                    dialog: root
                }

                FolderContents {
                    id: folderContents

                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    dialog: root
                }

                DialogButtons {
                    Layout.fillWidth: true
                    dialog: root
                }
            }
        }

        Behavior on color {
            CAnim {}
        }
    }
}
