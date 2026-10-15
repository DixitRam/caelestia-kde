pragma ComponentBehavior: Bound

import org.kde.pipewire as Pipewire
import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Widgets
import Caelestia
import Caelestia.Config
import Caelestia.Images
import Caelestia.Services
import qs.components
import qs.components.images
import qs.services
import qs.utils

Item {
    id: root

    property string address: ""
    property string activeAddress: ""
    property string source: ""
    property string activeSource: ""
    property url fallbackIcon: ""
    property color fallbackColor: Colours.palette.m3primary
    property real sourceAspect: 16 / 9
    property real glowScaleX: 2.0
    property real glowScaleY: 2.0
    property int blurMax: 96
    property real glowOpacity: 0.45
    property real saturation: 1.0
    property real brightness: 0.25
    property real contrast: 0.2
    property real radius: Tokens.rounding.medium
    property bool deform: true
    property bool active: !Colours.light
    property bool useFallback: true
    property real bloomProgress: 0.0
    property bool _thumbExists: root.thumbPath ? IUtils.fileExists(root.thumbPath) : false

    readonly property string thumbPath: root.activeAddress ? `${Paths.runtimeDir}/caelestia/window-thumbs/${root.activeAddress.startsWith("0x") ? root.activeAddress.slice(2) : root.activeAddress}.png` : ""
    readonly property real fitted: root.sourceAspect > (root.width / Math.max(1, root.height)) ? root.width / root.sourceAspect : root.height
    readonly property bool hasLiveStream: stream.available

    function triggerSwitch(): void {
        const hasTarget = root.address !== "" || root.source !== "";
        if (!root.active || !hasTarget) {
            switchAnim.stop();
            bloomInAnim.stop();
            root.bloomProgress = 0.0;
            root.activeAddress = "";
            root.activeSource = "";
            return;
        }

        if (!Tokens.anim.enabled) {
            switchAnim.stop();
            bloomInAnim.stop();
            root.activeAddress = root.address;
            root.activeSource = root.source;
            root.bloomProgress = 1.0;
            return;
        }

        if (root.activeAddress === "" && root.activeSource === "") {
            root.activeAddress = root.address;
            root.activeSource = root.source;
            root.bloomProgress = 0.0;
            bloomInAnim.restart();
            return;
        }

        if (root.activeAddress !== root.address || root.activeSource !== root.source) {
            bloomInAnim.stop();
            switchAnim.restart();
        }
    }

    onAddressChanged: root.triggerSwitch()
    onSourceChanged: root.triggerSwitch()
    onActiveChanged: root.triggerSwitch()

    opacity: root.active ? 1 : 0
    visible: opacity > 0.01

    Component.onCompleted: {
        root.triggerSwitch();
    }

    Behavior on opacity {
        Anim {
            type: Anim.DefaultEffects
        }
    }

    SequentialAnimation {
        id: switchAnim

        NumberAnimation {
            target: root
            property: "bloomProgress"
            to: 0.0
            duration: Tokens.anim.duration(350)
            easing.type: Easing.OutQuad
        }

        ScriptAction {
            script: {
                root.activeAddress = root.address;
                root.activeSource = root.source;
            }
        }

        NumberAnimation {
            target: root
            property: "bloomProgress"
            to: 1.0
            duration: Tokens.anim.duration(1350)
            easing.type: Easing.OutCubic
        }
    }

    NumberAnimation {
        id: bloomInAnim

        target: root
        property: "bloomProgress"
        from: 0.0
        to: 1.0
        duration: Tokens.anim.duration(1350)
        easing.type: Easing.OutCubic
    }

    WindowStream {
        id: stream

        active: root.active && GlobalConfig.bar.livePreviews && !!root.activeAddress
        address: root.activeAddress
    }

    Item {
        id: glowContainer

        anchors.fill: parent
        opacity: root.glowOpacity * root.bloomProgress
        transform: Scale {
            origin.x: glowContainer.width / 2
            origin.y: glowContainer.height / 2
            xScale: 1.0 + (root.glowScaleX - 1.0) * root.bloomProgress
            yScale: 1.0 + (root.glowScaleY - 1.0) * root.bloomProgress
        }

        Item {
            id: liveGlowItem

            anchors.fill: parent
            visible: root.hasLiveStream
            layer.enabled: visible
            layer.effect: MultiEffect {
                blurEnabled: true
                blur: 1.0
                blurMax: root.blurMax
                saturation: root.saturation
                brightness: root.brightness
                contrast: root.contrast
            }

            Pipewire.PipeWireSourceItem {
                anchors.fill: root.deform ? parent : undefined
                anchors.centerIn: root.deform ? undefined : parent
                width: root.deform ? parent.width : root.fitted * root.sourceAspect
                height: root.deform ? parent.height : root.fitted

                Component.onCompleted: {
                    if ("objectSerial" in this)
                        this.objectSerial = Qt.binding(() => stream.objectSerial);
                    else if ("nodeId" in this)
                        this.nodeId = Qt.binding(() => stream.nodeId);
                }
            }
        }

        Item {
            id: thumbGlowItem

            anchors.fill: parent
            visible: !liveGlowItem.visible && root._thumbExists && thumbImage.status === Image.Ready
            layer.enabled: visible
            layer.effect: MultiEffect {
                blurEnabled: true
                blur: 1.0
                blurMax: root.blurMax
                saturation: root.saturation
                brightness: root.brightness
                contrast: root.contrast
            }

            CachingImage {
                id: thumbImage

                anchors.fill: root.deform ? parent : undefined
                anchors.centerIn: root.deform ? undefined : parent
                width: root.deform ? parent.width : root.fitted * root.sourceAspect
                height: root.deform ? parent.height : root.fitted
                path: root._thumbExists ? root.thumbPath : ""
                fillMode: root.deform ? Image.Stretch : Image.PreserveAspectFit
                asynchronous: true
            }
        }

        Item {
            id: sourceGlowItem

            anchors.fill: parent
            visible: !liveGlowItem.visible && !thumbGlowItem.visible && !!root.activeSource && sourceImage.status === Image.Ready
            layer.enabled: visible
            layer.effect: MultiEffect {
                blurEnabled: true
                blur: 1.0
                blurMax: root.blurMax
                saturation: root.saturation
                brightness: root.brightness
                contrast: root.contrast
            }

            Image {
                id: sourceImage

                anchors.fill: root.deform ? parent : undefined
                anchors.centerIn: root.deform ? undefined : parent
                width: root.deform ? parent.width : root.fitted * root.sourceAspect
                height: root.deform ? parent.height : root.fitted
                source: root.activeSource
                fillMode: root.deform ? Image.Stretch : Image.PreserveAspectFit
                asynchronous: true
            }
        }

        Item {
            id: fallbackGlowItem

            anchors.fill: parent
            visible: root.useFallback && !liveGlowItem.visible && !thumbGlowItem.visible && !sourceGlowItem.visible
            layer.enabled: visible
            layer.effect: MultiEffect {
                blurEnabled: true
                blur: 1.0
                blurMax: root.blurMax
            }

            Rectangle {
                anchors.fill: parent
                radius: root.radius
                color: root.fallbackColor
            }
        }
    }
}
