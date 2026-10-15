pragma ComponentBehavior: Bound

import QtQuick
import Caelestia.Config
import qs.services

Text {
    id: root

    property bool animate: false
    // A swap grows in from this scale. 0.25 for a glyph, whose swap the growth makes
    // legible; a run of text stays at 1, where a shrink would just look like a wobble.
    property real swapFromScale: 1

    renderType: Text.NativeRendering
    textFormat: Text.PlainText
    color: Colours.palette.m3onSurface
    font: Tokens.font.body.small

    Behavior on color {
        CAnim {}
    }

    Behavior on text {
        enabled: root.animate

        SequentialAnimation {
            Anim {
                target: root
                property: "opacity"
                to: 0
                duration: Tokens.anim.durations.expressiveFastEffects
                easing: Tokens.anim.standard
            }
            PropertyAction {}
            PropertyAction {
                target: root
                property: "scale"
                value: root.swapFromScale
            }
            ParallelAnimation {
                Anim {
                    target: root
                    property: "opacity"
                    to: 1
                    duration: Tokens.anim.durations.expressiveDefaultEffects
                    easing: Tokens.anim.standard
                }
                Anim {
                    target: root
                    property: "scale"
                    to: 1
                    duration: Tokens.anim.durations.expressiveDefaultEffects
                    easing: Tokens.anim.standard
                }
            }
        }
    }
}
