import QtQuick
import qs.components

// The open/close motion every Nexus popup dialog shares. An exit is shorter and
// smaller than its entrance, so dismissing never feels heavier than opening.
Transition {
    property bool exiting

    Anim {
        property: "opacity"
        from: exiting ? 1 : 0
        to: exiting ? 0 : 1
        type: exiting ? Anim.FastEffects : Anim.DefaultEffects
    }
    Anim {
        property: "scale"
        from: exiting ? 1 : 0.9
        to: exiting ? 0.9 : 1
        type: exiting ? Anim.FastEffects : Anim.FastSpatial
    }
}
