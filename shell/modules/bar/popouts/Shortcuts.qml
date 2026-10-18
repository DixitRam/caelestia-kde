pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import Caelestia.Config
import qs.components
import "../../utilities/cards"

// Shortcuts popout: quick toggles only (custom shortcuts come later).
ColumnLayout {
    id: root

    required property var popouts
    required property DrawerVisibilities visibilities

    width: Math.max(360, togglesView.implicitWidth)
    spacing: Tokens.spacing.medium

    Toggles {
        id: togglesView

        Layout.fillWidth: true
        visibilities: root.visibilities
        popouts: root.popouts
    }
}
