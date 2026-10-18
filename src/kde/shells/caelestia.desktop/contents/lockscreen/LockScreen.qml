
import QtQuick
// Carried in the package and imported by directory: a module import is looked up in the engine's
// own import path, and the greeter is a process of the session that started before any of this was
// installed, so anything named as a module is looked for where it is not.
import "Caelestia/Services"
import "M3Shapes"

Item {
    id: root

    property bool locked: true
    property bool viewVisible: false
    property string notification

    signal clearPassword()
    signal notificationRepeated()

    onViewVisibleChanged: {
        if (!viewVisible) {
            root.clearPassword();
            return;
        }
        if (viewVisible && lockScreenUi && typeof lockScreenUi.ensureAuthenticating === "function") {
            lockScreenUi.ensureAuthenticating();
            if (lockScreenUi.activePasswordPill)
                lockScreenUi.activePasswordPill.forceActiveFocus();
        }
    }

    LayoutMirroring.enabled: Application.layoutDirection === Qt.RightToLeft
    LayoutMirroring.childrenInherit: true
    implicitWidth: 800
    implicitHeight: 600

    LockScreenUi {
        id: lockScreenUi

        anchors.fill: parent
    }
}
