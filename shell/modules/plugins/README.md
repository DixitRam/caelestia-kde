# Bundled Plugins Directory

This directory is for plugins that are fetched from the community plugin store and bundled with the shell.
Plugins in this directory are installed via cmake to `~/.config/quickshell/caelestia/modules/plugins/`.

User-installed plugins (e.g. ones created manually or downloaded from the store UI) should go into `~/.config/caelestia/plugins/`.

## Registering bar widgets

A plugin can add widgets to the taskbar (Nexus → Taskbar → Toggle & rearrange)
by declaring them in its `metadata.json`:

```json
{
  "id": "my-plugin",
  "name": "My Plugin",
  "type": "quickshell",
  "ui": "main.qml",
  "barWidgets": [
    {
      "id": "myWidget",
      "name": "My widget",
      "icon": "star",
      "defaultZone": "right",
      "component": "BarWidget.qml"
    }
  ]
}
```

Fields per widget:

| Field | Required | Description |
| --- | --- | --- |
| `id` | yes | Unique widget id. Must not collide with a built-in id (built-ins win). |
| `component` | yes | QML file **relative to the plugin folder**. Absolute paths and `..` are rejected. |
| `popout` | no | QML file **relative to the plugin folder**, rendered in the shell's sliding popout container. Same rules as `component`. Defaults to none. |
| `name` | no | Label shown in the editor. Defaults to `id`. |
| `icon` | no | Material icon name shown in the editor. Defaults to `widgets`. |
| `defaultZone` | no | `left`, `middle` or `right`. Defaults to `right`. |
| `available` | no | Whether the widget is usable. Defaults to `true`. |
| `unavailableText` | no | Reason shown when `available` is `false`. |

Once registered, the widget:

- shows up in the Toggle & rearrange page (disable, move between zones);
- renders in the bar (the bar shows it only while the entry is enabled);
- can open a hover popout by exposing a
  `handleHover(pos, isHorizontal, popouts)` function on its root item;
- can react to scroll events sent through the bar by exposing a
  `handleWheel(pos, isHorizontal, angleDelta)` function on its root item;
- can render a popout in the shell's sliding container: declare `popout` and
  set `popouts.currentName` to the widget's `id` to reveal it.

A widget is handed the ambient values the built-ins get, for the ones its root
declares: `bar`, `screen`, `visibilities` and `popouts`. Anything left out is
not touched.

| Property | Description |
| --- | --- |
| `bar` | The bar the widget is rendered in. |
| `screen` | The screen that bar belongs to. |
| `visibilities` | Which drawers and panels are currently visible. |
| `popouts` | The popout state, so the widget can open a popout. |

Widgets are registered when the plugin loads and unregistered when it is
disabled or removed, so no shell restart is needed after toggling the plugin.

