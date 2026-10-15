import QtQuick
import Caelestia.Config
import qs.services

ColorAnimation {
    // A palette landing is suppressed: every colour in the shell animates through here, and
    // a hundred of them starting in the same frame is what smears a theme switch. Landing
    // each colour instead keeps the switch a switch.
    duration: Colours.switchingTheme ? 0 : Tokens.anim.durations.expressiveSlowEffects
    easing: Tokens.anim.expressiveSlowEffects
}
