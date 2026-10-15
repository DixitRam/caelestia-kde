import QtQuick
import QtQuick.Layouts
import Caelestia.Config
import Caelestia.Services.QuickShare
import qs.components
import qs.services
import qs.modules.nexus.common

PageBase {
    id: root

    title: qsTr("Quick Share")
    isSubPage: true

    ColumnLayout {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        width: root.cappedWidth
        spacing: Tokens.spacing.extraSmall / 2

        SectionHeader {
            first: true
            text: qsTr("Status")
        }

        ToggleRow {
            first: true
            text: qsTr("Enable Quick Share")
            subtext: QuickShareService.isEnabled ? qsTr("Listening for nearby devices")
                : QuickShare.enabled ? qsTr("Checking this machine can receive")
                : qsTr("Disabled")
            checked: QuickShare.enabled
            onToggled: QuickShare.setEnabled(checked)
        }

        ToggleRow {
            text: qsTr("Discoverable")
            subtext: qsTr("Advertise this machine so nearby devices can send to it")
            enabled: QuickShareService.isEnabled
            checked: QuickShareService.isVisible
            onToggled: QuickShareService.isVisible = checked
        }

        InfoRow {
            last: true
            icon: "wifi"
            label: qsTr("Same network")
            subtext: qsTr("Nearby devices have to be on the same network, which for a phone means the same Wi-Fi")
        }

        SectionHeader {
            text: qsTr("Setup")
        }

        RowButton {
            first: true
            last: true
            icon: "security"
            text: qsTr("Quick Share setup")
            subtext: QuickShareSetup.status
            trailingIcon: "chevron_right"
            disabled: QuickShareSetup.busy
            onClicked: QuickShareSetup.run()
        }

        SectionHeader {
            text: qsTr("Startup")
        }

        ToggleRow {
            first: true
            last: true
            text: qsTr("Start automatically")
            subtext: qsTr("Turn Quick Share on when the shell starts")
            reset: ({ node: GlobalConfig.services, setting: "quickShareAutoStart" })
            checked: Config.services.quickShareAutoStart
            onToggled: GlobalConfig.services.quickShareAutoStart = checked
        }

        SectionHeader {
            text: qsTr("Transfers")
        }

        InfoRow {
            first: true
            icon: "download"
            label: qsTr("Received files")
            subtext: qsTr("Saved to your Downloads folder")
        }

        RowButton {
            last: true
            icon: "delete_sweep"
            text: qsTr("Clear transfer history")
            subtext: qsTr("%n transfer(s) recorded", "", QuickShareService.transferHistory.length)
            trailingIcon: "chevron_right"
            onClicked: QuickShareService.clearHistory()
        }
    }
}
