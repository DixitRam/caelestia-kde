pragma Singleton

import Quickshell
import Quickshell.Io
import Caelestia
import Caelestia.Models
import Caelestia.Config
import qs.utils

Searcher {
    id: root

    property alias alphaApps: appDb.alphaApps
    property list<string> settingsIds: []

    function launch(entry: DesktopEntry): void {
        appDb.incrementFrequency(entry.id);
        Launch.launchEntry(entry);
    }

    /// Every visible desktop entry in AppDb order (favourites, then frequency, then name).
    function allApps(): list<var> {
        const res = [];
        const apps = list;
        for (let i = 0; i < apps.length; i++)
            res.push(apps[i].entry);
        return res;
    }

    function search(search: string): list<var> {
        const prefix = GlobalConfig.launcher.specialPrefix;

        if (search.startsWith(`${prefix}i `)) {
            keys = ["id", "name"];
            weights = [0.9, 0.1];
        } else if (search.startsWith(`${prefix}c `)) {
            keys = ["categories", "name"];
            weights = [0.9, 0.1];
        } else if (search.startsWith(`${prefix}d `)) {
            keys = ["comment", "name"];
            weights = [0.9, 0.1];
        } else if (search.startsWith(`${prefix}e `)) {
            keys = ["execString", "name"];
            weights = [0.9, 0.1];
        } else if (search.startsWith(`${prefix}w `)) {
            keys = ["startupClass", "name"];
            weights = [0.9, 0.1];
        } else if (search.startsWith(`${prefix}g `)) {
            keys = ["genericName", "name"];
            weights = [0.9, 0.1];
        } else if (search.startsWith(`${prefix}k `)) {
            keys = ["keywords", "name"];
            weights = [0.9, 0.1];
        } else {
            keys = ["name"];
            weights = [1];

            if (!search.startsWith(`${prefix}t `))
                return withSettingsPages(search, query(search).map(e => e.entry));
        }

        const results = query(search.slice(prefix.length + 2)).map(e => e.entry);
        if (search.startsWith(`${prefix}t `))
            return results.filter(a => a.runInTerminal);
        return results;
    }

    // System Settings pages, like KRunner. They are NoDisplay, so DesktopEntries.applications
    // skips them; byId still resolves them. Name matches go above apps, comment-only matches below.
    function withSettingsPages(search: string, apps: list<var>): list<var> {
        const q = search.trim().toLowerCase();
        if (!GlobalConfig.launcher.searchSettingsPages || q.length < 3)
            return apps;

        // Every word has to appear; a trailing "s" is dropped so "themes" still finds "theme"
        const words = q.split(/\s+/).map(w => w.length > 3 ? w.replace(/s$/, "") : w);
        const matches = text => words.every(w => text.toLowerCase().includes(w));
        const pages = settingsIds.map(id => DesktopEntries.byId(id)).filter(Boolean);
        const byName = pages.filter(e => matches(e.name));
        const byComment = pages.filter(e => !byName.includes(e) && matches(`${e.name} ${e.comment} ${e.genericName}`));
        return byName.concat(apps, byComment);
    }

    function selector(item: var): string {
        return keys.map(k => item[k]).join(" ");
    }

    list: appDb.apps.filter(a => !Strings.testRegexList(GlobalConfig.launcher.hiddenApps, a.id))
    useFuzzy: GlobalConfig.launcher.useFuzzy.apps

    Process {
        running: GlobalConfig.launcher.searchSettingsPages && root.settingsIds.length === 0
        command: ["sh", "-c", "for f in /usr/share/applications/kcm_*.desktop; do [ -e \"$f\" ] && basename \"$f\" .desktop; done"]
        stdout: StdioCollector {
            onStreamFinished: root.settingsIds = text.trim().split("\n").filter(Boolean)
        }
    }

    AppDb {
        id: appDb

        path: `${Paths.state}/apps.sqlite`
        favouriteApps: GlobalConfig.launcher.favouriteApps
        entries: DesktopEntries.applications.values
    }
}
