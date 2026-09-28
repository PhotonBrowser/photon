#include "BrowserController.h"

#include <QByteArray>
#include <QDebug>

extern "C" {
BrowserHandle *photon_browser_create();
void photon_browser_destroy(BrowserHandle *);
const char *photon_browser_string(const BrowserHandle *, unsigned);
bool photon_browser_flag(const BrowserHandle *, unsigned);
void photon_browser_update(BrowserHandle *, const char *, const char *, bool, bool, bool);
void photon_browser_load_failed(BrowserHandle *, const char *);
void photon_browser_navigation_started(BrowserHandle *, bool);
void photon_browser_frame_presented(BrowserHandle *);
void photon_browser_cancel_navigation(BrowserHandle *);
bool photon_browser_navigate(BrowserHandle *, const char *, char *, size_t);
const char *photon_browser_error(const BrowserHandle *);
bool photon_browser_command(const BrowserHandle *, unsigned);
}

BrowserController::BrowserController(QObject *parent) : QObject(parent), m_state(photon_browser_create()) {}
BrowserController::~BrowserController() { photon_browser_destroy(m_state); }
QString BrowserController::url() const { return QString::fromUtf8(photon_browser_string(m_state, 0)); }
QString BrowserController::title() const { return QString::fromUtf8(photon_browser_string(m_state, 1)); }
QString BrowserController::error() const { return QString::fromUtf8(photon_browser_string(m_state, 2)); }
bool BrowserController::loading() const { return photon_browser_flag(m_state, 0); }
bool BrowserController::canGoBack() const { return photon_browser_flag(m_state, 1); }
bool BrowserController::canGoForward() const { return photon_browser_flag(m_state, 2); }
void BrowserController::setExecutor(std::function<void(int, QString)> executor) { m_executor = std::move(executor); }
void BrowserController::engineStateChanged(QString url, QString title, bool loading, bool back, bool forward) {
  const auto urlBytes = url.toUtf8();
  const auto titleBytes = title.toUtf8();
  photon_browser_update(m_state, urlBytes.constData(), titleBytes.constData(), loading, back, forward);
  emit stateChanged();
}
void BrowserController::engineFramePresented() {
  photon_browser_frame_presented(m_state);
  emit stateChanged();
}
void BrowserController::engineLoadFailed(QString message) {
  const auto bytes = message.toUtf8();
  photon_browser_load_failed(m_state, bytes.constData());
  emit stateChanged();
  emit navigationError(message);
}
void BrowserController::navigate(QString text) {
  auto input = text.toUtf8();
  char output[8192]{};
  if (!photon_browser_navigate(m_state, input.constData(), output, sizeof(output))) {
    emit navigationError(QString::fromUtf8(photon_browser_error(m_state)));
    return;
  }
  if (!m_executor) {
    qWarning() << "Browser navigation submitted before the engine view was ready";
    emit navigationError(QStringLiteral("The browser view is not ready"));
    return;
  }
  if (qEnvironmentVariableIsSet("PHOTON_VERBOSE"))
    qInfo().noquote() << "Browser command: Navigate(" << output << ")";
  photon_browser_navigation_started(m_state, false);
  emit stateChanged();
  m_executor(0, QString::fromUtf8(output));
}
void BrowserController::command(int command) {
  if (photon_browser_command(m_state, static_cast<unsigned>(command)) && m_executor) {
    if (qEnvironmentVariableIsSet("PHOTON_VERBOSE"))
      qInfo() << "Browser command:" << command;
    photon_browser_navigation_started(m_state, command == 1);
    emit stateChanged();
    m_executor(command, {});
  }
}
void BrowserController::reload() { command(1); }
void BrowserController::cancelNavigation() {
  if (!loading())
    return;
  photon_browser_cancel_navigation(m_state);
  emit stateChanged();
  if (m_executor)
    m_executor(4, {});
}
void BrowserController::back() { command(2); }
void BrowserController::forward() { command(3); }
