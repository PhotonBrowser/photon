#pragma once
#include <LibPhotonEmbedder/PhotonEmbedder.h>
#include <QImage>
#include <QMutex>
#include <QPointer>
#include <QQuickItem>
#include <QSizeF>
#include <QTimer>
#include <atomic>
#include <chrono>
#include <memory>

class PhotonWebView : public QQuickItem {
  Q_OBJECT
  QML_ELEMENT
  Q_PROPERTY(qreal cornerRadius READ cornerRadius WRITE setCornerRadius NOTIFY
                 cornerRadiusChanged)
  Q_PROPERTY(QObject *browser READ browser WRITE setBrowser)
public:
  explicit PhotonWebView(QQuickItem *parent = nullptr);
  ~PhotonWebView() override;
  qreal cornerRadius() const { return m_corner_radius; }
  void setCornerRadius(qreal radius);
  QObject *browser() const { return m_browser.data(); }
  void setBrowser(QObject *browser);

signals:
  void cornerRadiusChanged();

protected:
  void componentComplete() override;
  QSGNode *updatePaintNode(QSGNode *, UpdatePaintNodeData *) override;
  void geometryChange(const QRectF &, const QRectF &) override;
  void mousePressEvent(QMouseEvent *) override;
  void mouseReleaseEvent(QMouseEvent *) override;
  void mouseMoveEvent(QMouseEvent *) override;
  void hoverMoveEvent(QHoverEvent *) override;
  void wheelEvent(QWheelEvent *) override;
  void keyPressEvent(QKeyEvent *) override;
  void keyReleaseEvent(QKeyEvent *) override;
  void focusInEvent(QFocusEvent *) override;
  void focusOutEvent(QFocusEvent *) override;

private:
  void resizeEngineView();
  void scheduleEngineResize();
  std::unique_ptr<Photon::Runtime> m_runtime;
  std::unique_ptr<Photon::View> m_view;
  QTimer *m_resize_timer{nullptr};
  QTimer *m_metrics_timer{nullptr};
  QMutex m_frame_mutex;
  QImage m_pending_image;
  QSizeF m_pending_frame_size;
  QSizeF m_displayed_frame_size;
  std::atomic_bool m_has_pending_frame{false};
  std::atomic_bool m_shutting_down{false};
  std::atomic_uint64_t m_frames_received{0};
  std::atomic_uint64_t m_frames_presented{0};
  std::atomic_uint64_t m_frames_coalesced{0};
  std::atomic_uint64_t m_engine_copy_microseconds{0};
  std::atomic_uint64_t m_qt_copy_microseconds{0};
  std::atomic_uint64_t m_texture_upload_microseconds{0};
  std::atomic_int m_last_frame_width{0};
  std::atomic_int m_last_frame_height{0};
  bool m_component_complete{false};
  bool m_verbose{false};
  QPointer<QObject> m_browser;
  std::chrono::steady_clock::time_point m_navigation_started;
  std::chrono::steady_clock::time_point m_first_frame_started;
  bool m_navigation_loading{false};
  bool m_navigation_timing_active{false};
  qreal m_corner_radius{0};
};
