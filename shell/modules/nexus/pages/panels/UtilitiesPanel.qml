pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import Quickshell
import Caelestia.Config
import qs.components
import qs.components.controls
import qs.utils
import qs.modules.nexus.common

PageBase {
    id: root

    title: qsTr("Quick toggle")
    isSubPage: true

    ColumnLayout {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        width: root.cappedWidth
        spacing: Tokens.spacing.extraSmall / 2

        SectionHeader {
            first: true
            text: qsTr("General")
        }

        ToggleRow {
            first: true
            text: qsTr("Enabled")
            reset: ({ node: GlobalConfig.utilities, setting: "enabled" })
            checked: Config.utilities.enabled
            onToggled: GlobalConfig.utilities.enabled = checked
        }

        ToggleRow {
            Layout.fillWidth: true
            text: qsTr("Show on hover")
            subtext: qsTr("Reveal when the cursor reaches the screen edge")
            reset: ({ node: GlobalConfig.utilities, setting: "showOnHover" })
            checked: Config.utilities.showOnHover
            onToggled: GlobalConfig.utilities.showOnHover = checked
        }

        StepperRow {
            Layout.topMargin: Tokens.spacing.extraSmall / 2 - parent.spacing
            Layout.fillWidth: true
            label: qsTr("Hover trigger depth")
            subtext: qsTr("Distance in from the screen edge that opens the quick toggles")
            reset: ({ node: GlobalConfig.utilities, setting: "hoverThickness" })
            value: Config.utilities.hoverThickness
            from: 1
            to: 100
            stepSize: 1
            onMoved: v => GlobalConfig.utilities.hoverThickness = v
        }

        StepperRow {
            Layout.topMargin: Tokens.spacing.extraSmall / 2 - parent.spacing
            Layout.fillWidth: true
            label: qsTr("Hover trigger width")
            subtext: qsTr("How much of that edge opens the quick toggles, as a percentage of their width")
            reset: ({ node: GlobalConfig.utilities, setting: "hoverWidth" })
            value: Config.utilities.hoverWidth
            from: 10
            to: 100
            stepSize: 5
            onMoved: v => GlobalConfig.utilities.hoverWidth = v
        }

        StepperRow {
            Layout.topMargin: Tokens.spacing.extraSmall / 2 - parent.spacing
            Layout.fillWidth: true
            last: true
            label: qsTr("Drag threshold")
            subtext: qsTr("Pixels dragged before the quick toggle opens")
            reset: ({ node: GlobalConfig.utilities, setting: "dragThreshold" })
            value: Config.utilities.dragThreshold
            from: 0
            to: 200
            stepSize: 5
            onMoved: v => GlobalConfig.utilities.dragThreshold = v
        }
    }
}
