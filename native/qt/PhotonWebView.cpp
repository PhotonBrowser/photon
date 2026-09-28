#include "PhotonWebView.h"
#include "BrowserController.h"

#include <QDebug>
#include <QFocusEvent>
#include <QHoverEvent>
#include <QKeyEvent>
#include <QMetaObject>
#include <QMouseEvent>
#include <QQuickWindow>
#include <QSGClipNode>
#include <QSGSimpleTextureNode>
#include <QSGTexture>
#include <QStyleHints>
#include <QTimer>
#include <QWheelEvent>

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cmath>

namespace {

class PhotonPageNode final : public QSGClipNode {
public:
  PhotonPageNode() {
    m_texture_node = new QSGSimpleTextureNode;
    appendChildNode(m_texture_node);
  }

  QSGSimpleTextureNode *textureNode() const { return m_texture_node; }
  qreal clipRadius() const { return m_clip_radius; }
  void setClipRadius(qreal radius) { m_clip_radius = radius; }

  void setTexture(QSGTexture *texture, bool owned) {
    auto *previous = m_texture_node->texture();
    const bool previous_owned = m_texture_node->ownsTexture();
    m_texture_node->setOwnsTexture(false);
    m_texture_node->setTexture(texture);
    if (previous_owned && previous && previous != texture)
      delete previous;
    m_texture_node->setOwnsTexture(owned);
  }

private:
  QSGSimpleTextureNode *m_texture_node{nullptr};
  qreal m_clip_radius{-1};
};

} // namespace

PhotonWebView::PhotonWebView(QQuickItem *parent) : QQuickItem(parent) {
  setFlag(ItemHasContents, true);
  setClip(true);
  setAcceptedMouseButtons(Qt::AllButtons);
  setAcceptHoverEvents(true);
  setFlag(ItemIsFocusScope, true);
  setFocus(true);
  m_verbose = qEnvironmentVariableIsSet("PHOTON_VERBOSE");
  m_resize_timer = new QTimer(this);
  m_resize_timer->setSingleShot(true);
  m_resize_timer->setInterval(16);
  connect(m_resize_timer, &QTimer::timeout, this,
          [this] { resizeEngineView(); });

  connect(this, &QQuickItem::windowChanged, this, [this](QQuickWindow *window) {
    if (window)
      connect(window, &QQuickWindow::screenChanged, this,
              [this] { scheduleEngineResize(); });
    scheduleEngineResize();
  });
}

PhotonWebView::~PhotonWebView() {
  m_shutting_down.store(true, std::memory_order_release);
  if (m_resize_timer)
    m_resize_timer->stop();
  if (m_metrics_timer)
    m_metrics_timer->stop();
  // Stop callbacks before releasing the WebContent view, then release the
  // process/runtime owner after its view has been destroyed.
  if (m_view)
    m_view->shutdown();
  if (auto *browser = qobject_cast<BrowserController *>(m_browser.data()))
    browser->setExecutor({});
  m_view.reset();
  {
    QMutexLocker lock(&m_frame_mutex);
    m_pending_image = {};
    m_pending_frame_size = {};
  }
  m_runtime.reset();
}

void PhotonWebView::componentComplete() {
  QQuickItem::componentComplete();
  m_component_complete = true;

  std::string error;
  m_runtime = Photon::Runtime::create(PHOTON_HELPER_DIRECTORY, error);
  if (!m_runtime) {
    qCritical() << "Photon Engine startup failed:"
                << QString::fromStdString(error);
    return;
  }

  Photon::ViewCallbacks callbacks;
  callbacks.state_changed = [this](Photon::ViewState const &state) {
    if (m_shutting_down.load(std::memory_order_acquire))
      return;
    if (auto *browser = qobject_cast<BrowserController *>(m_browser.data()))
      browser->engineStateChanged(QString::fromStdString(state.url),
                                  QString::fromStdString(state.title),
                                  state.loading, state.can_go_back,
                                  state.can_go_forward);
    if (m_verbose)
      qInfo().noquote() << "Photon page:" << QString::fromStdString(state.url)
                        << "|" << QString::fromStdString(state.title)
                        << "| loading=" << state.loading
                        << " back=" << state.can_go_back
                        << " forward=" << state.can_go_forward;
    if (state.loading && m_navigation_timing_active)
      m_navigation_loading = true;
    if (!state.loading && m_navigation_loading) {
      auto elapsed =
          std::chrono::duration_cast<std::chrono::milliseconds>(
              std::chrono::steady_clock::now() - m_navigation_started)
              .count();
      if (m_verbose)
        qInfo() << "Photon navigation finished in" << elapsed << "ms";
      m_navigation_loading = false;
      m_navigation_timing_active = false;
    }
  };
  callbacks.failed = [this](std::string const &message) {
    if (m_shutting_down.load(std::memory_order_acquire))
      return;
    if (auto *browser = qobject_cast<BrowserController *>(m_browser.data()))
      browser->engineLoadFailed(QString::fromStdString(message));
    qWarning() << "Photon page error:" << QString::fromStdString(message);
  };
  callbacks.frame_ready =
      [this](std::shared_ptr<Photon::PresentedFrame const> frame) {
        if (m_shutting_down.load(std::memory_order_acquire))
          return;
        if (!frame || frame->width <= 0 || frame->height <= 0 ||
            frame->pixels.empty())
          return;
        if (m_navigation_timing_active &&
            m_first_frame_started != std::chrono::steady_clock::time_point{}) {
          auto elapsed =
              std::chrono::duration_cast<std::chrono::milliseconds>(
                  std::chrono::steady_clock::now() - m_first_frame_started)
                  .count();
          if (m_verbose)
            qInfo() << "Photon first frame after navigation:" << elapsed
                    << "ms";
          m_first_frame_started = {};
        }
        m_frames_received.fetch_add(1, std::memory_order_relaxed);
        if (m_has_pending_frame.exchange(true, std::memory_order_acq_rel))
          m_frames_coalesced.fetch_add(1, std::memory_order_relaxed);
        const auto engine_copy_us = frame->copy_time_microseconds;
        const auto frame_width = frame->width;
        const auto frame_height = frame->height;
        const auto frame_generation =
            m_frame_generation.fetch_add(1, std::memory_order_relaxed) + 1;
        const auto dpr =
            frame->device_pixel_ratio > 0 ? frame->device_pixel_ratio : 1.0;
        auto copy_started = std::chrono::steady_clock::now();
        QImage image(frame->pixels.data(), frame->width, frame->height,
                     static_cast<qsizetype>(frame->stride),
                     QImage::Format_ARGB32_Premultiplied);
        image = image.copy();
        const auto qt_copy_us = static_cast<std::uint64_t>(
            std::chrono::duration_cast<std::chrono::microseconds>(
                std::chrono::steady_clock::now() - copy_started)
                .count());
        {
          QMutexLocker lock(&m_frame_mutex);
          m_pending_image = std::move(image);
          m_pending_frame_size = QSizeF(frame_width / dpr, frame_height / dpr);
          m_pending_frame_generation = frame_generation;
        }
        m_engine_copy_microseconds.fetch_add(engine_copy_us,
                                             std::memory_order_relaxed);
        m_qt_copy_microseconds.fetch_add(qt_copy_us, std::memory_order_relaxed);
        m_last_frame_width.store(frame_width, std::memory_order_relaxed);
        m_last_frame_height.store(frame_height, std::memory_order_relaxed);
        update();
      };

  auto dpr = window() ? window()->devicePixelRatio() : 1.0;
  m_view = m_runtime->create_view(std::max(1, qRound(width())),
                                  std::max(1, qRound(height())), dpr,
                                  std::move(callbacks));
  if (!m_view) {
    qCritical() << "Photon Engine could not create a webpage view";
    return;
  }
  applyPreferredColorScheme();
  if (auto *browser = qobject_cast<BrowserController *>(m_browser.data()))
    browser->setExecutor([this](int command, QString url) {
      if (!m_view)
        return;
      switch (command) {
      case 0:
        m_required_frame_generation.store(
            m_frame_generation.load(std::memory_order_relaxed) + 1,
            std::memory_order_relaxed);
        m_navigation_started = std::chrono::steady_clock::now();
        m_first_frame_started = m_navigation_started;
        m_navigation_loading = false;
        m_navigation_timing_active = true;
        m_view->navigate(url.toStdString());
        break;
      case 1:
        m_required_frame_generation.store(
            m_frame_generation.load(std::memory_order_relaxed) + 1,
            std::memory_order_relaxed);
        m_navigation_started = std::chrono::steady_clock::now();
        m_first_frame_started = m_navigation_started;
        m_navigation_loading = false;
        m_navigation_timing_active = true;
        m_view->reload();
        break;
      case 2:
        m_required_frame_generation.store(
            m_frame_generation.load(std::memory_order_relaxed) + 1,
            std::memory_order_relaxed);
        m_navigation_started = std::chrono::steady_clock::now();
        m_first_frame_started = m_navigation_started;
        m_navigation_loading = false;
        m_navigation_timing_active = true;
        m_view->go_back();
        break;
      case 3:
        m_required_frame_generation.store(
            m_frame_generation.load(std::memory_order_relaxed) + 1,
            std::memory_order_relaxed);
        m_navigation_started = std::chrono::steady_clock::now();
        m_first_frame_started = m_navigation_started;
        m_navigation_loading = false;
        m_navigation_timing_active = true;
        m_view->go_forward();
        break;
      case 4:
        m_view->stop_loading();
        break;
      }
    });
  auto *pump_timer = new QTimer(this);
  pump_timer->setInterval(5);
  connect(pump_timer, &QTimer::timeout, this, [this] {
    if (m_runtime)
      m_runtime->pump();
  });
  pump_timer->start();
  if (m_verbose) {
    m_metrics_timer = new QTimer(this);
    m_metrics_timer->setInterval(5000);
    connect(m_metrics_timer, &QTimer::timeout, this, [this] {
      auto received = m_frames_received.exchange(0, std::memory_order_relaxed);
      auto presented =
          m_frames_presented.exchange(0, std::memory_order_relaxed);
      auto coalesced =
          m_frames_coalesced.exchange(0, std::memory_order_relaxed);
      if (!received && !presented && !coalesced)
        return;
      auto average = [](std::uint64_t total, std::uint64_t count) {
        return count ? static_cast<double>(total) / count : 0.0;
      };
      qInfo() << "Photon frames/5s received=" << received
              << "presented=" << presented << "coalesced=" << coalesced
              << "size=" << m_last_frame_width.load() << "x"
              << m_last_frame_height.load()
              << "DPR=" << (window() ? window()->devicePixelRatio() : 1.0)
              << "avg-us(engine-copy/Qt-copy/texture-upload)="
              << average(m_engine_copy_microseconds.exchange(0), received)
              << average(m_qt_copy_microseconds.exchange(0), received)
              << average(m_texture_upload_microseconds.exchange(0), presented);
    });
    m_metrics_timer->start();
  }
  resizeEngineView();
}

void PhotonWebView::setBrowser(QObject *browser) { m_browser = browser; }

void PhotonWebView::setDarkMode(bool dark) {
  if (m_dark_mode == dark)
    return;
  m_dark_mode = dark;
  emit darkModeChanged();
  applyPreferredColorScheme();
}

void PhotonWebView::applyPreferredColorScheme() {
  if (!m_view)
    return;
  m_view->set_preferred_color_scheme(m_dark_mode
                                         ? Photon::PreferredColorScheme::Dark
                                         : Photon::PreferredColorScheme::Light);
}

void PhotonWebView::geometryChange(const QRectF &newGeometry,
                                   const QRectF &oldGeometry) {
  QQuickItem::geometryChange(newGeometry, oldGeometry);
  scheduleEngineResize();
}

void PhotonWebView::scheduleEngineResize() {
  if (m_component_complete && m_resize_timer)
    m_resize_timer->start();
}

void PhotonWebView::resizeEngineView() {
  if (!m_component_complete || !m_view)
    return;
  auto dpr = window() ? window()->devicePixelRatio() : 1.0;
  m_view->resize(std::max(1, qRound(width())), std::max(1, qRound(height())),
                 dpr);
}

void PhotonWebView::setCornerRadius(qreal radius) {
  radius = std::max<qreal>(0, radius);
  if (qFuzzyCompare(m_corner_radius, radius))
    return;
  m_corner_radius = radius;
  emit cornerRadiusChanged();
  update();
}

static Photon::PointerButton photon_button(Qt::MouseButton button) {
  switch (button) {
  case Qt::LeftButton:
    return Photon::PointerButton::Primary;
  case Qt::RightButton:
    return Photon::PointerButton::Secondary;
  case Qt::MiddleButton:
    return Photon::PointerButton::Middle;
  case Qt::BackButton:
    return Photon::PointerButton::Back;
  case Qt::ForwardButton:
    return Photon::PointerButton::Forward;
  default:
    return Photon::PointerButton::None;
  }
}

static std::uint8_t photon_buttons(Qt::MouseButtons buttons) {
  std::uint8_t result = 0;
  for (auto button : {Qt::LeftButton, Qt::RightButton, Qt::MiddleButton,
                      Qt::BackButton, Qt::ForwardButton}) {
    if (buttons.testFlag(button))
      result |= static_cast<std::uint8_t>(photon_button(button));
  }
  return result;
}

static Photon::PointerEvent
pointer_event(Photon::PointerType type, QPointF position,
              QPointF global_position, Qt::MouseButton button,
              Qt::MouseButtons buttons, Qt::KeyboardModifiers modifiers) {
  Photon::PointerEvent event;
  event.type = type;
  event.x = position.x();
  event.y = position.y();
  event.screen_x = global_position.x();
  event.screen_y = global_position.y();
  event.button = photon_button(button);
  event.buttons = photon_buttons(buttons);
  event.shift = modifiers.testFlag(Qt::ShiftModifier);
  event.control = modifiers.testFlag(Qt::ControlModifier);
  event.alt = modifiers.testFlag(Qt::AltModifier);
  event.meta = modifiers.testFlag(Qt::MetaModifier);
  return event;
}

void PhotonWebView::mousePressEvent(QMouseEvent *event) {
  if (!m_view)
    return;
  setFocus(true);
  auto translated = pointer_event(Photon::PointerType::Press, event->position(),
                                  event->globalPosition(), event->button(),
                                  event->buttons(), event->modifiers());
  translated.click_count = 1;
  m_view->send_pointer_event(translated);
  event->accept();
}

void PhotonWebView::mouseReleaseEvent(QMouseEvent *event) {
  if (!m_view)
    return;
  m_view->send_pointer_event(pointer_event(
      Photon::PointerType::Release, event->position(), event->globalPosition(),
      event->button(), event->buttons(), event->modifiers()));
  event->accept();
}

void PhotonWebView::mouseMoveEvent(QMouseEvent *event) {
  if (!m_view)
    return;
  m_view->send_pointer_event(pointer_event(
      Photon::PointerType::Move, event->position(), event->globalPosition(),
      Qt::NoButton, event->buttons(), event->modifiers()));
  event->accept();
}

void PhotonWebView::hoverMoveEvent(QHoverEvent *event) {
  if (!m_view)
    return;
  m_view->send_pointer_event(pointer_event(
      Photon::PointerType::Move, event->position(), event->position(),
      Qt::NoButton, Qt::NoButton, Qt::NoModifier));
  event->accept();
}

void PhotonWebView::wheelEvent(QWheelEvent *event) {
  if (!m_view)
    return;
  auto translated = pointer_event(Photon::PointerType::Wheel, event->position(),
                                  event->globalPosition(), Qt::NoButton,
                                  event->buttons(), event->modifiers());
  auto pixel_delta = event->pixelDelta();
  if (!pixel_delta.isNull() && event->phase() != Qt::NoScrollPhase) {
    translated.wheel_x = -pixel_delta.x();
    translated.wheel_y = -pixel_delta.y();
    translated.precise_wheel = true;
  } else {
    constexpr double deltas_per_step = QWheelEvent::DefaultDeltasPerStep;
    auto step_size =
        static_cast<double>(QGuiApplication::styleHints()->wheelScrollLines()) *
        40.0;
    translated.wheel_x = -event->angleDelta().x() / deltas_per_step * step_size;
    translated.wheel_y = -event->angleDelta().y() / deltas_per_step * step_size;
  }
  switch (event->phase()) {
  case Qt::ScrollBegin:
  case Qt::ScrollUpdate:
    translated.scroll_phase = Photon::ScrollPhase::Ongoing;
    break;
  case Qt::ScrollMomentum:
    translated.scroll_phase = Photon::ScrollPhase::Momentum;
    break;
  case Qt::ScrollEnd:
    translated.scroll_phase = Photon::ScrollPhase::Ended;
    break;
  case Qt::NoScrollPhase:
    translated.scroll_phase = Photon::ScrollPhase::None;
    break;
  }
  m_view->send_pointer_event(translated);
  event->accept();
}

static Photon::Key photon_key(int key) {
  if ((key >= Qt::Key_A && key <= Qt::Key_Z) ||
      (key >= Qt::Key_0 && key <= Qt::Key_9))
    return static_cast<Photon::Key>(key);
  if (key >= Qt::Key_F1 && key <= Qt::Key_F12)
    return static_cast<Photon::Key>(0x70 + key - Qt::Key_F1);
  switch (key) {
  case Qt::Key_Backspace:
    return Photon::Key::Backspace;
  case Qt::Key_Tab:
  case Qt::Key_Backtab:
    return Photon::Key::Tab;
  case Qt::Key_Return:
  case Qt::Key_Enter:
    return Photon::Key::Enter;
  case Qt::Key_Escape:
    return Photon::Key::Escape;
  case Qt::Key_Space:
    return Photon::Key::Space;
  case Qt::Key_PageUp:
    return Photon::Key::PageUp;
  case Qt::Key_PageDown:
    return Photon::Key::PageDown;
  case Qt::Key_End:
    return Photon::Key::End;
  case Qt::Key_Home:
    return Photon::Key::Home;
  case Qt::Key_Left:
    return Photon::Key::Left;
  case Qt::Key_Up:
    return Photon::Key::Up;
  case Qt::Key_Right:
    return Photon::Key::Right;
  case Qt::Key_Down:
    return Photon::Key::Down;
  case Qt::Key_Delete:
    return Photon::Key::Delete;
  default:
    return Photon::Key::Unknown;
  }
}

static std::uint32_t event_code_point(QString const &text) {
  auto points = text.toUcs4();
  return points.isEmpty() ? 0 : points.first();
}

void PhotonWebView::keyPressEvent(QKeyEvent *event) {
  if (!m_view)
    return;
  auto modifiers = event->modifiers();
  m_view->send_key_event(
      photon_key(event->key()), true, event_code_point(event->text()),
      modifiers.testFlag(Qt::ShiftModifier),
      modifiers.testFlag(Qt::ControlModifier),
      modifiers.testFlag(Qt::AltModifier), modifiers.testFlag(Qt::MetaModifier),
      event->isAutoRepeat(), !event->text().isEmpty());
  event->accept();
}

void PhotonWebView::keyReleaseEvent(QKeyEvent *event) {
  if (!m_view)
    return;
  auto modifiers = event->modifiers();
  m_view->send_key_event(
      photon_key(event->key()), false, event_code_point(event->text()),
      modifiers.testFlag(Qt::ShiftModifier),
      modifiers.testFlag(Qt::ControlModifier),
      modifiers.testFlag(Qt::AltModifier), modifiers.testFlag(Qt::MetaModifier),
      event->isAutoRepeat(), false);
  event->accept();
}

void PhotonWebView::focusInEvent(QFocusEvent *event) {
  QQuickItem::focusInEvent(event);
  if (m_view)
    m_view->set_focus(true);
}

void PhotonWebView::focusOutEvent(QFocusEvent *event) {
  QQuickItem::focusOutEvent(event);
  if (m_view)
    m_view->set_focus(false);
}

QSGNode *PhotonWebView::updatePaintNode(QSGNode *oldNode,
                                        UpdatePaintNodeData *) {
  QImage image;
  QSizeF frame_size;
  std::uint64_t frame_generation = 0;
  {
    QMutexLocker lock(&m_frame_mutex);
    image = std::move(m_pending_image);
    if (!image.isNull()) {
      frame_size = m_pending_frame_size;
      m_pending_frame_size = {};
      frame_generation = m_pending_frame_generation;
    }
  }
  auto *clip = static_cast<PhotonPageNode *>(oldNode);
  if (!clip && image.isNull())
    return nullptr;
  if (!clip)
    clip = new PhotonPageNode;

  if (!image.isNull() && window()) {
    auto upload_started = std::chrono::steady_clock::now();
    auto *texture = window()->createTextureFromImage(image);
    m_texture_upload_microseconds.fetch_add(
        std::chrono::duration_cast<std::chrono::microseconds>(
            std::chrono::steady_clock::now() - upload_started)
            .count(),
        std::memory_order_relaxed);
    if (!texture) {
      qWarning() << "Qt Quick failed to upload a Photon page frame";
      return clip;
    }
    clip->setTexture(texture, true);
    clip->textureNode()->setFiltering(QSGTexture::Linear);
    m_displayed_frame_size = frame_size;
    m_has_pending_frame.store(false, std::memory_order_release);
    m_frames_presented.fetch_add(1, std::memory_order_relaxed);
    auto browser = m_browser;
    if (browser && frame_generation >= m_required_frame_generation.load(
                                           std::memory_order_relaxed)) {
      QMetaObject::invokeMethod(
          browser,
          [browser] {
            if (auto *controller =
                    qobject_cast<BrowserController *>(browser.data()))
              controller->engineFramePresented();
          },
          Qt::QueuedConnection);
    }
  }
  if (m_displayed_frame_size.isValid() && !m_displayed_frame_size.isEmpty())
    clip->textureNode()->setRect(QRectF(QPointF(0, 0), m_displayed_frame_size));

  auto bounds = boundingRect();
  auto radius = std::min<qreal>(m_corner_radius,
                                std::min(bounds.width(), bounds.height()) / 2);
  if (clip->geometry() && clip->clipRect() == bounds &&
      clip->clipRadius() == radius)
    return clip;
  QVector<QPointF> vertices;
  constexpr int segments_per_corner = 8;
  vertices.reserve(1 + 4 * (segments_per_corner + 1) + 1);
  vertices.append(bounds.center());
  const QPointF centers[] = {
      {bounds.right() - radius, bounds.top() + radius},
      {bounds.right() - radius, bounds.bottom() - radius},
      {bounds.left() + radius, bounds.bottom() - radius},
      {bounds.left() + radius, bounds.top() + radius},
  };
  const qreal starts[] = {-90, 0, 90, 180};
  constexpr qreal pi = 3.14159265358979323846;
  for (int corner = 0; corner < 4; ++corner) {
    for (int segment = 0; segment <= segments_per_corner; ++segment) {
      auto angle =
          (starts[corner] + 90.0 * segment / segments_per_corner) * pi / 180.0;
      vertices.append({centers[corner].x() + radius * std::cos(angle),
                       centers[corner].y() + radius * std::sin(angle)});
    }
  }
  vertices.append(vertices[1]);

  auto *geometry = new QSGGeometry(QSGGeometry::defaultAttributes_Point2D(),
                                   vertices.size());
  geometry->setDrawingMode(QSGGeometry::DrawTriangleFan);
  auto *points = geometry->vertexDataAsPoint2D();
  for (qsizetype index = 0; index < vertices.size(); ++index)
    points[index].set(vertices[index].x(), vertices[index].y());
  auto *old_geometry = clip->geometry();
  clip->setFlag(QSGNode::OwnsGeometry, false);
  delete old_geometry;
  clip->setGeometry(geometry);
  clip->setFlag(QSGNode::OwnsGeometry, true);
  clip->setClipRect(bounds);
  clip->setClipRadius(radius);
  clip->setIsRectangular(false);
  return clip;
}
