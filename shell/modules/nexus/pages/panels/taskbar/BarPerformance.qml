pragma ComponentBehavior: Bound

import QtQuick.Layouts
import Quickshell
import Caelestia.Config
import qs.components.controls
import qs.modules.nexus.common

PageBase {
    id: root

    readonly property list<MenuItem> pillItems: [MenuItem {
        text: qsTr("CPU")
    }, MenuItem {
        text: qsTr("GPU")
    }, MenuItem {
        text: qsTr("Memory")
    }, MenuItem {
        text: qsTr("Storage")
    }, MenuItem {
        text: qsTr("Network")
    }, MenuItem {
        text: qsTr("Battery")
    }]
    readonly property list<string> pillValues: ["cpu", "gpu", "memory", "storage", "network", "battery"]
    readonly property string pill: String(Config.bar.performance?.pill ?? "").trim().toLowerCase()
    readonly property int pillIndex: Math.max(0, root.pillValues.indexOf(root.pill))
    readonly property list<MenuItem> styleItems: [MenuItem {
        text: qsTr("Pill")
    }, MenuItem {
        text: qsTr("Rectangle")
    }]
    readonly property list<string> styleValues: ["classic", "compact"]
    readonly property string pillStyle: String(Config.bar.performance?.pillStyle ?? "classic").trim().toLowerCase()
    readonly property int styleIndex: Math.max(0, root.styleValues.indexOf(root.pillStyle))
    readonly property list<MenuItem> widgetStyleItems: [MenuItem {
        text: qsTr("Icon")
    }, MenuItem {
        text: qsTr("Pill")
    }, MenuItem {
        text: qsTr("Rectangle")
    }]
    readonly property list<string> widgetStyleValues: ["icon", "classic", "compact"]
    readonly property string widgetStyle: String(Config.bar.performance?.widgetStyle ?? "classic").trim().toLowerCase()
    readonly property int widgetStyleIndex: Math.max(0, root.widgetStyleValues.indexOf(root.widgetStyle))

    title: qsTr("Performance")
    isSubPage: true

    ColumnLayout {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        width: root.cappedWidth
        spacing: Tokens.spacing.extraSmall / 2

        SectionHeader {
            first: true
            text: qsTr("Specific widgets")
        }

        SelectRow {
            first: true
            label: qsTr("Pill style")
            subtext: qsTr("Choose the shape of the specific pills")
            menuItems: root.styleItems
            active: root.styleItems[root.styleIndex]
            onSelected: item => {
                GlobalConfig.bar.performance.pillStyle = root.styleValues[root.styleItems.indexOf(item)];
                GlobalConfig.save();
            }
        }

        ToggleRow {
            last: true
            text: qsTr("Show values")
            subtext: qsTr("Show the value next to the icon in each pill")
            reset: ({ node: GlobalConfig.bar.performance, setting: "showText" })
            checked: Config.bar.performance.showText
            onToggled: {
                GlobalConfig.bar.performance.showText = checked;
                GlobalConfig.save();
            }
        }

        SectionHeader {
            text: qsTr("Panel widget")
        }

        SelectRow {
            first: true
            label: qsTr("Style")
            subtext: qsTr("Speed icon or a stat pill in the panel")
            menuItems: root.widgetStyleItems
            active: root.widgetStyleItems[root.widgetStyleIndex]
            onSelected: item => {
                GlobalConfig.bar.performance.widgetStyle = root.widgetStyleValues[root.widgetStyleItems.indexOf(item)];
                GlobalConfig.save();
            }
        }

        SelectRow {
            label: qsTr("Metric")
            subtext: qsTr("Stat shown in the bar, the full data is in the popout")
            enabled: root.widgetStyle !== "icon"
            opacity: root.widgetStyle !== "icon" ? 1 : 0.5
            menuItems: root.pillItems
            active: root.pillItems[root.pillIndex]
            onSelected: item => {
                GlobalConfig.bar.performance.pill = root.pillValues[root.pillItems.indexOf(item)];
                GlobalConfig.save();
            }
        }

        ToggleRow {
            last: true
            text: qsTr("Show values")
            subtext: qsTr("Show the value next to the icon in the panel widget")
            enabled: root.widgetStyle !== "icon"
            opacity: root.widgetStyle !== "icon" ? 1 : 0.5
            reset: ({ node: GlobalConfig.bar.performance, setting: "widgetShowText" })
            checked: Config.bar.performance.widgetShowText
            onToggled: {
                GlobalConfig.bar.performance.widgetShowText = checked;
                GlobalConfig.save();
            }
        }
    }
}
