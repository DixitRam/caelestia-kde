import QtQuick
import QtQuick.Layouts
import Caelestia.Config
import qs.components
import qs.components.controls
import qs.services

StyledRect {
    id: root

    required property var dialog

    implicitHeight: inner.implicitHeight + Tokens.padding.medium * 2

    color: Colours.tPalette.m3surfaceContainer

    RowLayout {
        id: inner

        anchors.fill: parent
        anchors.margins: Tokens.padding.medium

        spacing: Tokens.spacing.small

        StyledText {
            text: qsTr("Filter:")
        }

        StyledRect {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.rightMargin: Tokens.spacing.medium

            color: Colours.tPalette.m3surfaceContainerHigh
            radius: Tokens.rounding.medium

            StyledText {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                anchors.margins: Tokens.padding.medium

                elide: Text.ElideRight
                text: root.dialog.selectFolder ? qsTr("Folders") : `${root.dialog.filterLabel} (${root.dialog.filters.map(f => `*.${f}`).join(", ")})`
            }
        }

        // The shared button, not a private pill: the shell's density, press feedback
        // and disabled colours belong to dialog actions too.
        TextButton {
            text: root.dialog.selectFolder ? qsTr("Select Folder") : qsTr("Select")
            type: TextButton.Tonal
            inactiveColour: Colours.tPalette.m3surfaceContainerHigh
            inactiveOnColour: Colours.palette.m3onSurface
            disabled: !root.dialog.selectionValid
            onClicked: root.dialog.acceptSelection()
        }

        TextButton {
            text: qsTr("Cancel")
            type: TextButton.Tonal
            inactiveColour: Colours.tPalette.m3surfaceContainerHigh
            inactiveOnColour: Colours.palette.m3onSurface
            onClicked: root.dialog.rejected()
        }
    }
}
