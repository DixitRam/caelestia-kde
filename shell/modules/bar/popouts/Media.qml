pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Services.Mpris
import Caelestia.Config
import qs.components
import qs.components.controls
import qs.services
import qs.modules.nexus
import qs.modules.dashboard.dash as DashMedia

// Bar popout reusing the dashboard mini player, with a source line on top.
ColumnLayout {
    id: root

    required property var popouts

    property real scaleOffset: 1.0
    property real fontScale: 1.0
    property bool _isSidebarOpen: false

    readonly property MprisPlayer player: Players.sourcePlayer(Config.bar.media.excludedSources)
    readonly property string sourceName: Players.getIdentity(root.player)
    readonly property real miniWidth: 228

    // Same scroll volume as the bar icon: matching PipeWire stream app
    // volume, global sink when nothing is playing.
    readonly property var playerStream: Audio.appStreams ? Players.playerStream(root.player, Audio) : null
    readonly property real currentVolume: (root.playerStream && root.playerStream.audio) ? root.playerStream.audio.volume : (root.playerStream ? Audio.getAppVolume(root.playerStream) : Audio.volume)

    function setVolumeLevel(v: real): void {
        Players.setPlayerVolume(root.player, root.playerStream, Audio, v);
    }

    width: root.miniWidth + Tokens.padding.large * 2
    spacing: Tokens.spacing.small

    Item {
        id: sourceSelector

        Layout.fillWidth: true
        implicitHeight: sourceClickArea.implicitHeight
        visible: root.sourceName !== "" || Players.list.length > 0

        CustomMouseArea {
            id: sourceClickArea

            anchors.left: parent.left
            anchors.right: parent.right
            implicitHeight: Math.max(sourceLabel.implicitHeight, sourceExpandIcon.implicitHeight) + Tokens.padding.extraSmall
            cursorShape: Players.list.length > 1 ? Qt.PointingHandCursor : Qt.ArrowCursor
            hoverEnabled: true

            onClicked: {
                if (Players.list.length > 1)
                    sourceMenu.expanded = !sourceMenu.expanded;
            }

            RowLayout {
                anchors.fill: parent
                spacing: Tokens.spacing.extraSmall

                StyledText {
                    id: sourceLabel

                    Layout.fillWidth: true
                    text: root.sourceName || qsTr("No media")
                    font: Tokens.font.label.medium
                    color: sourceClickArea.containsMouse && Players.list.length > 1 ? Colours.palette.m3primary : Colours.palette.m3onSurfaceVariant
                    elide: Text.ElideRight
                }

                MaterialIcon {
                    id: sourceExpandIcon

                    visible: Players.list.length > 1
                    text: "expand_more"
                    fontStyle: Tokens.font.icon.small
                    color: sourceClickArea.containsMouse ? Colours.palette.m3primary : Colours.palette.m3onSurfaceVariant
                    rotation: sourceMenu.expanded ? 180 : 0

                    Behavior on rotation {
                        Anim {}
                    }
                }
            }
        }

        Menu {
            id: sourceMenu

            attachTo: sourceSelector
            attachSideY: Menu.Bottom
            thisSideY: Menu.Top
            attachSideX: Menu.Left
            thisSideX: Menu.Left
            marginY: Tokens.spacing.extraSmall

            items: playerList.instances
            onItemSelected: item => Players.manualActive = (item as PlayerItem).modelData
        }

        Variants {
            id: playerList

            model: Players.list

            PlayerItem {}
        }
    }

    Item {
        id: mediaContainer

        Layout.alignment: Qt.AlignHCenter
        Layout.preferredWidth: root.miniWidth
        Layout.preferredHeight: dashMedia.implicitHeight
        Layout.bottomMargin: Tokens.padding.small
        implicitHeight: Layout.preferredHeight

        DashMedia.Media {
            id: dashMedia

            width: root.miniWidth
            player: root.player
            extraButtons: true
            showGif: false
        }
    }

    WheelHandler {
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onWheel: event => {
            const step = GlobalConfig.services.audioIncrement || 0.05;
            if (event.angleDelta.y > 0)
                root.setVolumeLevel(root.currentVolume + step);
            else if (event.angleDelta.y < 0)
                root.setVolumeLevel(root.currentVolume - step);
        }
    }

    component PlayerItem: MenuItem {
        required property MprisPlayer modelData

        icon: modelData === Players.active ? "check" : ""
        text: Players.getIdentity(modelData)
        activeIcon: "animated_images"
    }
}
