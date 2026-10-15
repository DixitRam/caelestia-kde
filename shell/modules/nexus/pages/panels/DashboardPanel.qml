pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import Quickshell
import M3Shapes
import Caelestia.Config
import qs.components
import qs.components.controls
import qs.services
import qs.utils
import qs.modules.nexus.common

PageBase {
    id: root

    readonly property list<MenuItem> dashboardShapeItems: [
        MenuItem {
            property int value: MaterialShape.Circle

            text: qsTr("Circle")
        },
        MenuItem {
            property int value: MaterialShape.Square

            text: qsTr("Square")
        },
        MenuItem {
            property int value: MaterialShape.Pill

            text: qsTr("Pill")
        },
        MenuItem {
            property int value: MaterialShape.Diamond

            text: qsTr("Diamond")
        },
        MenuItem {
            property int value: MaterialShape.ClamShell

            text: qsTr("Clam Shell")
        },
        MenuItem {
            property int value: MaterialShape.Pentagon

            text: qsTr("Pentagon")
        },
        MenuItem {
            property int value: MaterialShape.Gem

            text: qsTr("Gem")
        },
        MenuItem {
            property int value: MaterialShape.Cookie4Sided

            text: qsTr("Cookie 4-Sided")
        },
        MenuItem {
            property int value: MaterialShape.Cookie6Sided

            text: qsTr("Cookie 6-Sided")
        },
        MenuItem {
            property int value: MaterialShape.Cookie7Sided

            text: qsTr("Cookie 7-Sided")
        },
        MenuItem {
            property int value: MaterialShape.Cookie9Sided

            text: qsTr("Cookie 9-Sided")
        },
        MenuItem {
            property int value: MaterialShape.Cookie12Sided

            text: qsTr("Cookie 12-Sided")
        }
    ]

    title: qsTr("Dashboard")
    isSubPage: true

    ColumnLayout {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        width: root.cappedWidth
        spacing: Tokens.spacing.extraSmall / 2

        SectionHeader {
            first: true
            text: qsTr("General")
        }

        ToggleRow {
            first: true
            text: qsTr("Enabled")
            reset: ({ node: GlobalConfig.dashboard, setting: "enabled" })
            checked: Config.dashboard.enabled
            onToggled: GlobalConfig.dashboard.enabled = checked
        }

        ToggleRow {
            Layout.fillWidth: true
            text: qsTr("Show on hover")
            subtext: qsTr("Reveal when the cursor reaches the screen edge")
            reset: ({ node: GlobalConfig.dashboard, setting: "showOnHover" })
            checked: Config.dashboard.showOnHover
            onToggled: GlobalConfig.dashboard.showOnHover = checked
        }

        SelectRow {
            Layout.fillWidth: true
            label: qsTr("Dashboard profile picture shape")
            subtext: qsTr("Choose the shape of the profile picture on the dashboard")
            fallbackIcon: "person"
            fallbackText: qsTr("Pill")
            active: {
                for (let i = 0; i < dashboardShapeItems.length; i++) {
                    if (dashboardShapeItems[i].value === GlobalConfig.dashboard.profilePicShape)
                        return dashboardShapeItems[i];
                }
                return dashboardShapeItems[0];
            }
            menuItems: dashboardShapeItems
            onSelected: item => {
                GlobalConfig.dashboard.profilePicShape = item.value
            }
        }
        ToggleRow {
            Layout.fillWidth: true
            last: true
            text: qsTr("Show clock seconds")
            subtext: qsTr("Display seconds below the clock in the dashboard")
            reset: ({ node: GlobalConfig.dashboard, setting: "showClockSeconds" })
            checked: Config.dashboard.showClockSeconds
            onToggled: GlobalConfig.dashboard.showClockSeconds = checked
        }
        SectionHeader {
            text: qsTr("Tabs")
        }

        ToggleRow {
            first: true
            text: qsTr("Dashboard")
            reset: ({ node: GlobalConfig.dashboard, setting: "showDashboard" })
            checked: Config.dashboard.showDashboard
            onToggled: GlobalConfig.dashboard.showDashboard = checked
        }

        ToggleRow {
            text: qsTr("Media")
            reset: ({ node: GlobalConfig.dashboard, setting: "showMedia" })
            checked: Config.dashboard.showMedia
            onToggled: GlobalConfig.dashboard.showMedia = checked
        }

        ToggleRow {
            text: qsTr("Performance")
            reset: ({ node: GlobalConfig.dashboard, setting: "showPerformance" })
            checked: Config.dashboard.showPerformance
            onToggled: GlobalConfig.dashboard.showPerformance = checked
        }

        ToggleRow {
            Layout.fillWidth: true
            text: qsTr("Weather")
            reset: ({ node: GlobalConfig.dashboard, setting: "showWeather" })
            checked: Config.dashboard.showWeather
            onToggled: GlobalConfig.dashboard.showWeather = checked
        }

        ToggleRow {
            Layout.fillWidth: true
            text: qsTr("Notes")
            reset: ({ node: GlobalConfig.dashboard, setting: "showNotes" })
            checked: Config.dashboard.showNotes
            onToggled: GlobalConfig.dashboard.showNotes = checked
        }

        ToggleRow {
            Layout.fillWidth: true
            text: qsTr("Terminal")
            reset: ({ node: GlobalConfig.dashboard, setting: "showTerminal" })
            checked: Config.dashboard.showTerminal
            onToggled: GlobalConfig.dashboard.showTerminal = checked
        }

        ToggleRow {
            Layout.fillWidth: true
            text: qsTr("Recolor media GIF")
            subtext: qsTr("Apply system theme colors to the media GIF")
            reset: ({ node: GlobalConfig.dashboard, setting: "colorizeMediaGif" })
            checked: Config.dashboard.colorizeMediaGif
            onToggled: GlobalConfig.dashboard.colorizeMediaGif = checked
        }

        ToggleRow {
            Layout.fillWidth: true
            text: qsTr("Use material shapes")
            subtext: qsTr("Replace the media GIF with audio-reactive material shapes")
            reset: ({ node: GlobalConfig.dashboard, setting: "useMediaShapes" })
            checked: Config.dashboard.useMediaShapes
            onToggled: GlobalConfig.dashboard.useMediaShapes = checked
        }

        ToggleRow {
            Layout.fillWidth: true
            text: qsTr("Ambient color mode")
            subtext: Colours.light ? qsTr("Ambient glow is unavailable in light mode") : qsTr("Ambient light glow in media tab")
            reset: ({ node: GlobalConfig.dashboard, setting: "ambientColor" })
            checked: Config.dashboard.ambientColor
            enabled: !Colours.light
            onToggled: GlobalConfig.dashboard.ambientColor = checked
        }

        StepperRow {
            last: true
            label: qsTr("Ambient glow opacity")
            reset: ({ node: GlobalConfig.dashboard, setting: "ambientOpacity" })
            value: Config.dashboard.ambientOpacity
            from: 0
            to: 1
            stepSize: 0.05
            enabled: Config.dashboard.ambientColor && !Colours.light
            opacity: enabled ? 1 : 0.5
            onMoved: v => GlobalConfig.dashboard.ambientOpacity = v
        }

        ToggleRow {
            Layout.fillWidth: true
            last: true
            text: qsTr("Welcome splash")
            visible: false
            subtext: qsTr("Show a welcome message on the dashboard")
            reset: ({ node: GlobalConfig.dashboard, setting: "showHyprlandSplash" })
            checked: Config.dashboard.showHyprlandSplash
            onToggled: GlobalConfig.dashboard.showHyprlandSplash = checked
        }

        SectionHeader {
            text: qsTr("Performance widgets")
        }

        ToggleRow {
            first: true
            text: qsTr("Battery")
            reset: ({ node: GlobalConfig.dashboard.performance, setting: "showBattery" })
            checked: Config.dashboard.performance.showBattery
            onToggled: GlobalConfig.dashboard.performance.showBattery = checked
        }

        ToggleRow {
            text: qsTr("GPU")
            reset: ({ node: GlobalConfig.dashboard.performance, setting: "showGpu" })
            checked: Config.dashboard.performance.showGpu
            onToggled: GlobalConfig.dashboard.performance.showGpu = checked
        }

        ToggleRow {
            text: qsTr("CPU")
            reset: ({ node: GlobalConfig.dashboard.performance, setting: "showCpu" })
            checked: Config.dashboard.performance.showCpu
            onToggled: GlobalConfig.dashboard.performance.showCpu = checked
        }

        ToggleRow {
            text: qsTr("Memory")
            reset: ({ node: GlobalConfig.dashboard.performance, setting: "showMemory" })
            checked: Config.dashboard.performance.showMemory
            onToggled: GlobalConfig.dashboard.performance.showMemory = checked
        }

        ToggleRow {
            text: qsTr("Storage")
            reset: ({ node: GlobalConfig.dashboard.performance, setting: "showStorage" })
            checked: Config.dashboard.performance.showStorage
            onToggled: GlobalConfig.dashboard.performance.showStorage = checked
        }

        ToggleRow {
            last: true
            text: qsTr("Network")
            reset: ({ node: GlobalConfig.dashboard.performance, setting: "showNetwork" })
            checked: Config.dashboard.performance.showNetwork
            onToggled: GlobalConfig.dashboard.performance.showNetwork = checked
        }

        SectionHeader {
            text: qsTr("Behavior")
        }

        StepperRow {
            first: true
            label: qsTr("Hover trigger depth")
            subtext: qsTr("Distance in from the screen edge that opens the dashboard")
            reset: ({ node: GlobalConfig.dashboard, setting: "hoverThickness" })
            value: Config.dashboard.hoverThickness
            from: 1
            to: 100
            stepSize: 1
            onMoved: v => GlobalConfig.dashboard.hoverThickness = v
        }

        StepperRow {
            label: qsTr("Hover trigger width")
            subtext: qsTr("How much of the top edge opens the dashboard, as a percentage of its width")
            reset: ({ node: GlobalConfig.dashboard, setting: "hoverWidth" })
            value: Config.dashboard.hoverWidth
            from: 10
            to: 100
            stepSize: 5
            onMoved: v => GlobalConfig.dashboard.hoverWidth = v
        }

        StepperRow {
            last: true
            label: qsTr("Drag threshold")
            subtext: qsTr("Pixels dragged before the dashboard opens")
            reset: ({ node: GlobalConfig.dashboard, setting: "dragThreshold" })
            value: Config.dashboard.dragThreshold
            from: 0
            to: 200
            stepSize: 5
            onMoved: v => GlobalConfig.dashboard.dragThreshold = v
        }
    }
}
