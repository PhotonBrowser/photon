#pragma once

#include <QObject>
#include <QString>
#include <QtQml/qqmlregistration.h>
#include <functional>

struct BrowserHandle;

class BrowserController : public QObject {
  Q_OBJECT
  QML_ELEMENT
  Q_PROPERTY(QString url READ url NOTIFY stateChanged)
  Q_PROPERTY(QString title READ title NOTIFY stateChanged)
  Q_PROPERTY(QString error READ error NOTIFY stateChanged)
  Q_PROPERTY(bool loading READ loading NOTIFY stateChanged)
  Q_PROPERTY(bool canGoBack READ canGoBack NOTIFY stateChanged)
  Q_PROPERTY(bool canGoForward READ canGoForward NOTIFY stateChanged)
public:
  explicit BrowserController(QObject *parent = nullptr);
  ~BrowserController() override;
  QString url() const;
  QString title() const;
  QString error() const;
  bool loading() const;
  bool canGoBack() const;
  bool canGoForward() const;
  void setExecutor(std::function<void(int, QString)> executor);
  void engineStateChanged(QString url, QString title, bool loading, bool back, bool forward);
  void engineLoadFailed(QString message);
  Q_INVOKABLE void navigate(QString text);
  Q_INVOKABLE void reload();
  Q_INVOKABLE void back();
  Q_INVOKABLE void forward();
signals:
  void stateChanged();
  void navigationError(QString message);
private:
  void command(int command);
  BrowserHandle *m_state;
  std::function<void(int, QString)> m_executor;
};
