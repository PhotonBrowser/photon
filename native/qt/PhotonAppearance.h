#pragma once

#include <QObject>
#include <QtQml/qqmlregistration.h>

#if defined(Q_OS_LINUX)
#include <QDBusVariant>
#endif

class PhotonAppearance : public QObject {
  Q_OBJECT
  QML_ELEMENT
  QML_SINGLETON
  Q_PROPERTY(bool dark READ isDark NOTIFY darkChanged)

public:
  explicit PhotonAppearance(QObject *parent = nullptr);

  bool isDark() const { return m_dark; }

signals:
  void darkChanged();

private:
  void updateColorScheme();

#if defined(Q_OS_LINUX)
private slots:
  void onPortalSettingChanged(QString const &nameSpace, QString const &key,
                              QDBusVariant const &value);
#endif

private:
  bool m_dark{false};
  int m_portalColorScheme{-1};
};
