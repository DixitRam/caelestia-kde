pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import Quickshell
import Caelestia.Config
import qs.components
import "../../utilities/cards"
import qs.services

// Screen capture popout: the utilities card, reused as is.
ColumnLayout {
    id: root

    required property var popouts
    required property DrawerVisibilities visibilities

    width: Math.max(360, captureView.implicitWidth)
    spacing: Tokens.spacing.small

    CaptureCard {
        id: captureView

        Layout.fillWidth: true
        visibilities: root.visibilities
        props: cardProps
    }

    PersistentProperties {
        id: cardProps

        property string captureMode: "shot"
        property string recordingMode
    }
}
