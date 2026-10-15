pragma ComponentBehavior: Bound

import QtQuick
import Caelestia.Config

QtObject {
    id: root

    property int baseDuration: GlobalConfig.overview.baseDuration
    property real blobScaleSpeed: GlobalConfig.overview.blobScaleSpeed
    property real wallpaperFadeSpeed: GlobalConfig.overview.wallpaperFadeSpeed
    property real gridFadeSpeed: GlobalConfig.overview.gridFadeSpeed
    property int easingType: GlobalConfig.overview.easingType
    // Through Tokens.anim.duration so the appearance page's animations switch reaches
    // these too: the overview and the drawers it morphs are the shell's biggest motion,
    // and a switch that only stops the small ones is not a switch.
    readonly property int blobDuration: Tokens.anim.duration(Math.round(baseDuration / blobScaleSpeed))
    readonly property int wallpaperDuration: Tokens.anim.duration(Math.round(baseDuration / wallpaperFadeSpeed))
    readonly property int gridDuration: Tokens.anim.duration(Math.round(baseDuration / gridFadeSpeed))
}
