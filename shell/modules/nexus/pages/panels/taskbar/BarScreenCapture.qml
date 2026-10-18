pragma ComponentBehavior: Bound

import QtQuick.Layouts
import Caelestia.Config
import qs.modules.nexus.common

PageBase {
    id: root

    title: qsTr("Screen Capture")
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
            text: qsTr("Show screenshot buttons")
            subtext: qsTr("Show region/window/screen capture buttons")
            reset: ({ node: GlobalConfig.bar.screencapture, setting: "showShots" })
            checked: Config.bar.screencapture.showShots
            onToggled: {
                GlobalConfig.bar.screencapture.showShots = checked;
                GlobalConfig.save();
            }
        }

        ToggleRow {
            text: qsTr("Show recording buttons")
            subtext: qsTr("Show region/screen recording buttons")
            reset: ({ node: GlobalConfig.bar.screencapture, setting: "showRecord" })
            checked: Config.bar.screencapture.showRecord
            onToggled: {
                GlobalConfig.bar.screencapture.showRecord = checked;
                GlobalConfig.save();
            }
        }

        ToggleRow {
            last: true
            text: qsTr("Show folder shortcuts")
            subtext: qsTr("Show buttons to open screenshots/recordings folders")
            reset: ({ node: GlobalConfig.bar.screencapture, setting: "showFolders" })
            checked: Config.bar.screencapture.showFolders
            onToggled: {
                GlobalConfig.bar.screencapture.showFolders = checked;
                GlobalConfig.save();
            }
        }
    }
}
