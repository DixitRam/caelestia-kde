pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import Quickshell.Services.Mpris
import Quickshell.Services.Pipewire
import Caelestia.Config
import Caelestia.Services
import qs.components
import qs.services

// Compact now-playing entry: truncated title plus the cava visualiser.
StyledRect {
    id: root

    required property var popouts

    readonly property bool isHorizontal: Config.bar.position === "top" || Config.bar.position === "bottom"
    readonly property MprisPlayer player: Players.sourcePlayer(Config.bar.media.excludedSources)

    readonly property int maxLen: Config.bar.media.maxTitleLength
    readonly property string rawTitle: player?.trackTitle || qsTr("Nothing playing")
    readonly property string trackTitle: rawTitle.length > maxLen ? rawTitle.substring(0, maxLen) + "…" : rawTitle
    readonly property bool isPlaying: player?.isPlaying ?? false
    readonly property bool available: player !== null
    readonly property bool showTitle: Config.bar.media.showTitle
    readonly property bool showVisualiser: Config.bar.media.showVisualiser
    // Text mode only while actually playing; paused, stopped or missing
    // players fall back to the icon.
    readonly property bool showText: root.showTitle && root.isPlaying
    readonly property bool showBars: root.showVisualiser && root.isPlaying
    readonly property bool hasContent: root.showText || root.showBars
    readonly property real contentWidth: root.hasContent ? contentLayout.implicitWidth : modeIcon.implicitWidth
    readonly property real contentHeight: root.hasContent ? contentLayout.implicitHeight : modeIcon.implicitHeight

    // Stream-aware volume: matching PipeWire stream app volume first,
    // then the global sink. Driven by mouse scroll.
    // Shared with the popout through Players so both stay in sync.
    readonly property PwNode playerStream: Audio.appStreams ? Players.playerStream(root.player, Audio) : null
    readonly property real currentVolume: (root.playerStream && root.playerStream.audio) ? root.playerStream.audio.volume : (root.playerStream ? Audio.getAppVolume(root.playerStream) : Audio.volume)

    function setVolumeLevel(v: real): void {
        Players.setPlayerVolume(root.player, root.playerStream, Audio, v);
    }

    implicitWidth: isHorizontal ? (root.hasContent && root.showText ? contentWidth + Tokens.padding.medium * 2 : Math.max(Tokens.sizes.bar.innerWidth, contentWidth + Tokens.padding.medium * 2)) : Tokens.sizes.bar.innerWidth
    implicitHeight: isHorizontal ? Tokens.sizes.bar.innerWidth : (root.hasContent && root.showText ? contentHeight + Tokens.padding.medium * 2 : Math.max(Tokens.sizes.bar.innerWidth, contentHeight + Tokens.padding.medium * 2))

    color: Config.bar.media.background ? Colours.tPalette.m3surfaceContainer : Qt.alpha(Colours.tPalette.m3surfaceContainer, 0)
    radius: Tokens.rounding.full

    // A bar entry with auto-hide collapses instead of leaving an empty pill.
    visible: enabled && (!Config.bar.media.autoHide || available)

    ServiceRef {
        service: Config.bar.media.showVisualiser ? Audio.cava : null
    }

    GridLayout {
        id: contentLayout

        anchors.centerIn: parent
        columns: root.isHorizontal ? (root.showText && root.showBars ? 2 : 1) : 1
        rows: root.isHorizontal ? 1 : (root.showText && root.showBars ? 2 : 1)
        rowSpacing: (root.showText && root.showBars) ? Tokens.spacing.small : 0
        columnSpacing: (root.showText && root.showBars) ? Tokens.spacing.small : 0

        Item {
            id: textContainer

            visible: root.showText

            Layout.alignment: Qt.AlignCenter
            Layout.row: root.isHorizontal ? 0 : (Config.bar.media.inverted ? 1 : 0)
            Layout.column: 0

            implicitWidth: root.showText ? (root.isHorizontal ? titleText.implicitWidth : titleText.implicitHeight) : 0
            implicitHeight: root.showText ? (root.isHorizontal ? titleText.implicitHeight : titleText.implicitWidth) : 0

            StyledText {
                id: titleText

                anchors.centerIn: parent
                text: root.trackTitle
                font: Tokens.font.body.medium
                color: Colours.palette.m3onSurface
                elide: Text.ElideRight
                animate: true

                rotation: root.isHorizontal ? 0 : (Config.bar.media.inverted ? 270 : 90)
            }
        }

        Item {
            id: equalizerContainer

            Layout.alignment: Qt.AlignCenter
            Layout.row: root.isHorizontal ? 0 : (root.showText && Config.bar.media.inverted ? 0 : 1)
            Layout.column: (root.isHorizontal && root.showText) ? 1 : 0

            visible: root.showBars
            implicitWidth: root.showBars ? (root.isHorizontal ? 23 : 20) : 0
            implicitHeight: root.showBars ? (root.isHorizontal ? 20 : 23) : 0

            Item {
                anchors.centerIn: parent
                width: 23
                height: 20

                rotation: root.isHorizontal ? 0 : (Config.bar.media.inverted ? 270 : 90)

                Repeater {
                    model: 5

                    Rectangle {
                        id: barItem

                        required property int index

                        readonly property real cavaVal: (Audio.cava && Audio.cava.values && Audio.cava.values.length > index * 2) ? Audio.cava.values[index * 2] : 0
                        // Parked flat while animations are off: the bars are decoration, and
                        // motion is their only channel.
                        readonly property real rawHeight: root.isPlaying && Tokens.anim.enabled ? Math.max(3, Math.min(18, 3 + cavaVal * 15)) : 3

                        x: index * 5
                        width: 3
                        height: Math.round(rawHeight)
                        y: Math.round((20 - height) / 2)
                        radius: 1.5
                        color: Colours.palette.m3primary

                        Behavior on height {
                            NumberAnimation {
                                // Audio tracking: the bars follow the level, they are not
                                // animating a state change, so this stays far shorter than
                                // any transition token.
                                duration: Tokens.anim.duration(50)
                                easing.type: Easing.OutQuad
                            }
                        }
                    }
                }
            }
        }
    }

    MaterialIcon {
        id: modeIcon

        anchors.centerIn: parent
        visible: !root.hasContent
        text: "graphic_eq"
        color: Colours.palette.m3onSurface
        fontStyle: Tokens.font.icon.builders.medium.build()
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.RightButton
        cursorShape: Qt.PointingHandCursor
        onClicked: mouse => {
            if (!root.popouts)
                return;
            root.popouts.currentName = "mediacontext";
            root.popouts.currentCenter = root.isHorizontal ? root.mapToItem(null, root.implicitWidth / 2, 0).x : (root.mapToItem(null, 0, root.implicitHeight / 2).y ?? 0);
            root.popouts.hasCurrent = true;
            mouse.accepted = true;
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
}
