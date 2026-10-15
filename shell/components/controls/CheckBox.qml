pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import Quickshell
import Caelestia.Config
import qs.components
import qs.components.controls
import qs.services

Item {
    id: root

    property bool checked: false

    signal toggled()

    implicitWidth: 24
    implicitHeight: 24

    StyledRect {
        anchors.fill: parent
        radius: Tokens.rounding.medium
        color: "transparent"
        border.width: 2
        border.color: !root.enabled ? Colours.disabledContainer : root.checked ? Colours.palette.m3primary : Colours.palette.m3onSurfaceVariant

        Behavior on border.color {
            CAnim {}
        }

        MaterialIcon {
            anchors.centerIn: parent
            text: "check"
            fontStyle: Tokens.font.icon.builders.small.weight(Font.DemiBold).build()
            color: !root.enabled ? Colours.disabledContent : Colours.palette.m3primary
            scale: root.checked ? 1 : 0

            Behavior on scale {
                Anim {
                    type: Anim.EmphasizedSmall
                }
            }
        }
    }

    CustomMouseArea {
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: {
            root.checked = !root.checked;
            root.toggled();
        }
    }
}
