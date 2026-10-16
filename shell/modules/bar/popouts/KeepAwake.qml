pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import Caelestia.Config
import qs.components
import "../../utilities/cards"
import qs.services

// Keep awake popout: the utilities card, reused as is.
ColumnLayout {
    id: root

    required property var popouts

    width: Math.max(360, keepAwakeView.implicitWidth)
    spacing: Tokens.spacing.small

    IdleInhibit {
        id: keepAwakeView

        Layout.fillWidth: true
    }
}
