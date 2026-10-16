pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Services.UPower
import Caelestia
import Caelestia.Config
import Caelestia.Services

Scope {
    id: root

    readonly property list<var> warnLevels: [...GlobalConfig.general.battery.warnLevels.values].sort((a, b) => b.level - a.level)

    // UPower reports its initial state as a change shortly after startup, which is not a plug event.
    property bool settled: false

    Timer {
        interval: 5000
        running: true
        onTriggered: root.settled = true
    }

    Connections {
        function onOnBatteryChanged(): void {
            const notify = root.settled && GlobalConfig.utilities.toasts.chargingChanged;
            if (UPower.onBattery) {
                if (notify)
                    Toaster.toast(qsTr("Charger unplugged"), qsTr("Battery is discharging"), "power_off");
            } else {
                if (notify)
                    Toaster.toast(qsTr("Charger plugged in"), qsTr("Battery is charging"), "power");
                for (const level of root.warnLevels)
                    level.warned = false;
            }
        }

        target: UPower
    }

    Connections {
        function onPercentageChanged(): void {
            if (!UPower.onBattery)
                return;

            const p = UPower.displayDevice.percentage * 100;
            for (const level of root.warnLevels) {
                if (p <= level.level && !level.warned) {
                    level.warned = true;
                    Toaster.toast(level.title ?? qsTr("Battery warning"), level.message ?? qsTr("Battery level is low"), level.icon ?? "battery_android_alert", level.critical ? Toast.Error : Toast.Warning);
                }
            }
        }

        target: UPower.displayDevice
    }
}
