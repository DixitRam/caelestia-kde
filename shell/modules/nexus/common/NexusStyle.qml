pragma Singleton

import QtQuick
import Quickshell
import Caelestia.Config

// Single definition of the Nexus secondary text size. Every secondary
// text across Nexus points here, so future size tweaks are one line.
Singleton {
    readonly property font secondaryFont: Tokens.font.label.builders.small.size(10).build()

    // How much a dragged row or tile grows while it is held. A lift rather than a
    // press, so it sits above 1 instead of at the 0.96 press scale.
    readonly property real dragScale: 1.05
}
