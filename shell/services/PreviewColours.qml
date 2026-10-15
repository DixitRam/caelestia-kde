import QtQuick
import Quickshell.Io
import qs.services

// The colour preview of the wallpaper picker. It owns the matugen process and decides when its
// result may cover the applied scheme:
//   show(source)  preview an image; a call made while a preview runs reruns it for the latest source.
//                 An empty source is a no-op, so a source that is not previewable yet leaves whatever
//                 is on screen alone until the caller can name one (see onVideoThumb)
//   hold()        cover the applied palette until the scheme that replaces it lands, so an apply
//                 does not fall back to the old palette while the engine runs; it holds nothing
//                 unless a preview palette is on screen
//   release()     the applied scheme has landed; the preview ends unless the picker wants it again
//   stop()        the preview is over; a result that arrives later is dropped
Item {
    id: root

    readonly property bool active: wanted !== ""

    property bool held
    property string wanted
    property string running

    function show(source: string): void {
        if (source === "")
            return;
        wanted = source;
        if (!proc.running)
            start();
    }

    function hold(): void {
        // A hold with no preview palette under it keeps nothing up, and an apply the engine does
        // not follow — a named scheme, a wallpaper with no thumbnail yet — would never end it.
        held = Colours.showPreview;
    }

    function release(): void {
        if (!held)
            return;
        held = false;
        settle();
    }

    function stop(): void {
        wanted = "";
        settle();
    }

    // The preview covers the applied scheme only while something still wants it: the picker's
    // highlight, or a chosen scheme that has not landed yet.
    function settle(): void {
        if (!active && !held)
            Colours.clearPreview();
    }

    function start(): void {
        if (!active)
            return;
        running = wanted;
        proc.running = true;
    }

    Process {
        id: proc

        command: ["caelestia", "wallpaper", "-p", root.running, ...Colours.smartArg]
        stdout: StdioCollector {
            onStreamFinished: {
                // Nothing wants a preview any more and no chosen scheme is waiting on one.
                if (!root.active && !root.held)
                    return;
                // The highlight moved while this ran, so the result belongs to an earlier one.
                if (root.active && root.wanted !== root.running) {
                    Qt.callLater(root.start);
                    return;
                }
                Colours.loadPreview(text);
            }
        }
    }

    Connections {
        // The applied scheme landing is the event a hold covers, so it is what ends one. An apply
        // the engine does not follow leaves the preview up until the next scheme lands — and that
        // palette comes from the wallpaper the user chose, which beats the one it was replacing.
        function onSchemeApplied(): void {
            root.release();
        }

        target: Colours
    }
}
