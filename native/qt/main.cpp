#include <QDebug>
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlError>

int main(int argc, char **argv) {
  QGuiApplication app(argc, argv);
  app.setApplicationName("Photon");

  QQmlApplicationEngine engine;
  QObject::connect(&engine, &QQmlApplicationEngine::warnings, &app,
                   [](QList<QQmlError> const &warnings) {
                     for (auto const &warning : warnings)
                       qWarning().noquote() << warning.toString();
                   });
  engine.loadFromModule("Photon", "Main");
  if (engine.rootObjects().isEmpty())
    return 1;
  return app.exec();
}
