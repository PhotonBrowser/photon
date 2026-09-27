#include "PhotonAppearance.h"

#include <QGuiApplication>
#include <QPalette>
#include <QStyleHints>

#if defined(Q_OS_LINUX)
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDBusVariant>
#endif

namespace {
#if defined(Q_OS_LINUX)
constexpr auto portalService = "org.freedesktop.portal.Desktop";
constexpr auto portalPath = "/org/freedesktop/portal/desktop";
constexpr auto portalInterface = "org.freedesktop.portal.Settings";

int colorSchemeValue(QDBusVariant const &value) {
  QVariant unwrapped = value.variant();
  while (unwrapped.metaType() == QMetaType::fromType<QDBusVariant>())
    unwrapped = unwrapped.value<QDBusVariant>().variant();

  bool ok = false;
  int scheme = unwrapped.toInt(&ok);
  return ok && scheme >= 0 && scheme <= 2 ? scheme : -1;
}
#endif
} // namespace

PhotonAppearance::PhotonAppearance(QObject *parent) : QObject(parent) {
  auto *hints = QGuiApplication::styleHints();
  connect(hints, &QStyleHints::colorSchemeChanged, this,
          [this] { updateColorScheme(); });

#if defined(Q_OS_LINUX)
  QDBusConnection::sessionBus().connect(
      QString::fromLatin1(portalService), QString::fromLatin1(portalPath),
      QString::fromLatin1(portalInterface), QStringLiteral("SettingChanged"),
      this, SLOT(onPortalSettingChanged(QString, QString, QDBusVariant)));

  QDBusMessage request = QDBusMessage::createMethodCall(
      QString::fromLatin1(portalService), QString::fromLatin1(portalPath),
      QString::fromLatin1(portalInterface), QStringLiteral("Read"));
  request << QStringLiteral("org.freedesktop.appearance")
          << QStringLiteral("color-scheme");
  auto *watcher = new QDBusPendingCallWatcher(
      QDBusConnection::sessionBus().asyncCall(request), this);
  connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, watcher] {
    QDBusPendingReply<QDBusVariant> reply = *watcher;
    if (reply.isValid())
      onPortalSettingChanged(QStringLiteral("org.freedesktop.appearance"),
                             QStringLiteral("color-scheme"), reply.value());
    watcher->deleteLater();
  });
#endif

  updateColorScheme();
}

#if defined(Q_OS_LINUX)
void PhotonAppearance::onPortalSettingChanged(QString const &nameSpace,
                                              QString const &key,
                                              QDBusVariant const &value) {
  if (nameSpace != QLatin1String("org.freedesktop.appearance") ||
      key != QLatin1String("color-scheme"))
    return;
  m_portalColorScheme = colorSchemeValue(value);
  updateColorScheme();
}
#endif

void PhotonAppearance::updateColorScheme() {
  bool dark = false;
#if defined(Q_OS_LINUX)
  if (m_portalColorScheme == 1) {
    dark = true;
  } else if (m_portalColorScheme == 2) {
    dark = false;
  } else
#endif
  {
    auto scheme = QGuiApplication::styleHints()->colorScheme();
    dark =
        scheme == Qt::ColorScheme::Dark ||
        (scheme == Qt::ColorScheme::Unknown &&
         QGuiApplication::palette().color(QPalette::Window).lightness() < 128);
  }

  if (m_dark == dark)
    return;
  m_dark = dark;
  emit darkChanged();
}
