#pragma once
#include <QQuickItem>

class PhotonWebView : public QQuickItem {
  Q_OBJECT
  QML_ELEMENT
public:
  explicit PhotonWebView(QQuickItem *parent = nullptr);
};
