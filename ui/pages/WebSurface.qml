import QtQuick
import Photon

Rectangle {
    id: root
    required property var browser

    radius: Theme.radius
    color: Theme.surface
    clip: true

    PhotonWebView {
        anchors.fill: parent
        browser: root.browser
        cornerRadius: Theme.radius
    }
}
