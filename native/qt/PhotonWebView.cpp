#include "PhotonWebView.h"

PhotonWebView::PhotonWebView(QQuickItem *parent) : QQuickItem(parent) {
  setFlag(ItemHasContents, false);
}
