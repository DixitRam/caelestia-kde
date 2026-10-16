#include <QApplication>
#include <QCommandLineParser>
#include <QScreen>
#include <QProcess>
#include <Plasma/Corona>
#include <Plasma/Containment>
#include <Plasma/Applet>
#include <PlasmaQuick/ConfigView>
#include <KPackage/PackageLoader>
#include <KSharedConfig>
#include <KConfigGroup>
#include <QDebug>

#ifdef HAVE_KDBUSADDONS
#include <KDBusService>
#endif

class ConfigCorona : public Plasma::Corona {
public:
    explicit ConfigCorona(QObject *parent = nullptr) : Plasma::Corona(parent) {
        auto pkg = KPackage::PackageLoader::self()->loadPackage(QStringLiteral("Plasma/Shell"));
        pkg.setPath(QStringLiteral("org.kde.plasma.desktop"));
        setKPackage(pkg);
    }

    QRect screenGeometry(int id) const override {
        const auto scs = QGuiApplication::screens();
        if (id >= 0 && id < scs.size()) {
            return scs[id]->geometry();
        }
        if (!scs.isEmpty()) {
            return scs.first()->geometry();
        }
        return QRect(0, 0, 1920, 1080);
    }
};

int main(int argc, char **argv) {
    QApplication app(argc, argv);
    app.setApplicationName(QStringLiteral("caelestia-plasma-applet-config"));
    app.setOrganizationDomain(QStringLiteral("caelestia.org"));

    QCommandLineParser parser;
    parser.setApplicationDescription(QStringLiteral("Plasma Applet Configuration Window"));
    parser.addHelpOption();
    parser.addPositionalArgument(QStringLiteral("plugin"), QStringLiteral("Plugin ID of the applet to configure"));
    parser.process(app);

    const QStringList posArgs = parser.positionalArguments();
    const QString targetPlugin = posArgs.isEmpty() ? QStringLiteral("luisbocanegra.audio.visualizer") : posArgs.first();

#ifdef HAVE_KDBUSADDONS
    KDBusService service(KDBusService::Unique);
#endif

    ConfigCorona corona;
    const QString layoutFile = QStringLiteral("plasma-org.kde.plasma.desktop-appletsrc");
    corona.loadLayout(layoutFile);

    QList<Plasma::Applet *> matchingApplets;
    Plasma::Containment *desktopContainment = nullptr;

    for (auto c : corona.containments()) {
        if (!desktopContainment && (c->containmentType() == Plasma::Containment::Desktop ||
            c->pluginMetaData().pluginId() == QStringLiteral("org.kde.desktopcontainment"))) {
            desktopContainment = c;
        }
        for (auto a : c->applets()) {
            if (a->pluginMetaData().pluginId() == targetPlugin) {
                matchingApplets.append(a);
            }
        }
    }

    if (matchingApplets.isEmpty()) {
        if (!desktopContainment) {
            desktopContainment = corona.createContainment(QStringLiteral("org.kde.desktopcontainment"));
        }
        if (desktopContainment) {
            auto a = desktopContainment->createApplet(targetPlugin);
            if (a) {
                matchingApplets.append(a);
            }
        }
    }

    if (matchingApplets.isEmpty()) {
        qWarning() << "Failed to find or create applet for plugin:" << targetPlugin;
        return 1;
    }

    Plasma::Applet *primaryApplet = matchingApplets.first();
    auto view = new PlasmaQuick::ConfigView(primaryApplet);
    view->init();
    view->setTitle(primaryApplet->title().isEmpty()
        ? QStringLiteral("Configure Audio Visualizer")
        : QStringLiteral("Configure %1").arg(primaryApplet->title()));
    view->show();
    view->requestActivate();

#ifdef HAVE_KDBUSADDONS
    QObject::connect(&service, &KDBusService::activateRequested, [&view](const QStringList &, const QString &) {
        if (view) {
            view->show();
            view->raise();
            view->requestActivate();
        }
    });
#endif

    auto syncAndNotify = [&corona, &matchingApplets, &layoutFile, &targetPlugin]() {
        if (matchingApplets.size() > 1) {
            KConfigGroup srcCg = matchingApplets.first()->config();
            KConfigGroup srcGen(&srcCg, QStringLiteral("General"));
            for (int i = 1; i < matchingApplets.size(); ++i) {
                KConfigGroup dstCg = matchingApplets[i]->config();
                KConfigGroup dstGen(&dstCg, QStringLiteral("General"));
                srcGen.copyTo(&dstGen);
                dstGen.sync();
            }
        }
        corona.saveLayout(layoutFile);
        if (corona.config()) {
            corona.config()->sync();
        }

        const QString script = QStringLiteral(
            "for (var i = 0; i < desktops().length; i++) {"
            "    var ws = desktops()[i].widgets();"
            "    for (var j = 0; j < ws.length; j++) {"
            "        if (ws[j].type === '%1') {"
            "            ws[j].reloadConfig();"
            "        }"
            "    }"
            "}"
        ).arg(targetPlugin);

        QProcess::execute(QStringLiteral("qdbus6"), {
            QStringLiteral("org.kde.plasmashell"),
            QStringLiteral("/PlasmaShell"),
            QStringLiteral("org.kde.PlasmaShell.evaluateScript"),
            script
        });
    };

    QObject::connect(primaryApplet, &Plasma::Applet::configNeedsSaving, syncAndNotify);

    QObject::connect(view, &QWindow::visibleChanged, [&app, &syncAndNotify](bool visible) {
        if (!visible) {
            syncAndNotify();
            app.quit();
        }
    });

    return app.exec();
}
