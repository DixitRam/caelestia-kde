pragma ComponentBehavior: Bound

import QtQuick.Layouts
import Caelestia.Config
import qs.modules.nexus.common

PageBase {
    id: root

    title: qsTr("Keep Awake")
    isSubPage: true

    ColumnLayout {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        width: root.cappedWidth
        spacing: Tokens.spacing.extraSmall / 2

        SectionHeader {
            first: true
            text: qsTr("Configuration")
        }

        ToggleRow {
            first: true
            last: true
            text: qsTr("Show active indicator")
            subtext: qsTr("Show a chip with the activation time when Keep Awake is on")
            reset: ({ node: GlobalConfig.bar.keepawake, setting: "showActiveChip" })
            checked: Config.bar.keepawake.showActiveChip
            onToggled: {
                GlobalConfig.bar.keepawake.showActiveChip = checked;
                GlobalConfig.save();
            }
        }

        SectionHeader {
            text: qsTr("Widget")
        }

        ToggleRow {
            first: true
            last: true
            text: qsTr("Auto-hide when inactive")
            subtext: qsTr("Hide the widget when Keep Awake is off")
            reset: ({ node: GlobalConfig.bar.keepawake, setting: "autoHide" })
            checked: Config.bar.keepawake.autoHide
            onToggled: {
                GlobalConfig.bar.keepawake.autoHide = checked;
                GlobalConfig.save();
            }
        }
    }
}
