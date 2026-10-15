import QtQuick
import Caelestia.Config

Item {
    id: root

    property date shownDate
    property real offset: Tokens.padding.extraLarge

    property bool _repositioning: false
    property real animTranslate
    property real animOpacity: 1

    property date _prevDate

    Component.onCompleted: root._prevDate = root.shownDate

    onShownDateChanged: {
        const parked = offset * (shownDate > _prevDate ? 1 : -1);
        _prevDate = shownDate;

        _repositioning = true;
        animTranslate = parked;
        animOpacity = 0;
        _repositioning = false;
        animTranslate = 0;
        animOpacity = 1;
    }

    Behavior on animTranslate {
        enabled: !root._repositioning

        Anim {
            type: Anim.DefaultSpatial
        }
    }

    Behavior on animOpacity {
        enabled: !root._repositioning

        Anim {
            type: Anim.FastEffects
        }
    }
}
