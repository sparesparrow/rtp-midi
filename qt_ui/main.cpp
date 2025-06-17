#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include "rust_service_bridge.h"

int main(int argc, char *argv[])
{
    QGuiApplication app(argc, argv);

    QQmlApplicationEngine engine;

    // Registrace našeho C++ bridge, aby byl dostupný z QML
    qmlRegisterType<RustServiceBridge>("tech.sparrow-ai.rtpmidi", 1, 0, "RustService");

    const QUrl url(QStringLiteral("qrc:/main.qml"));
    QObject::connect(&engine, &QQmlApplicationEngine::objectCreated,
                     &app, [url](QObject *obj, const QUrl &objUrl) {
        if (!obj && url == objUrl)
            QCoreApplication::exit(-1);
    }, Qt::QueuedConnection);
    engine.load(url);

    return app.exec();
}
