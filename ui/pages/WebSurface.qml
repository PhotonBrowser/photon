pragma ComponentBehavior: Bound
import QtQuick
import Photon

Rectangle {
    id: root
    required property var browser

    radius: Theme.radius
    color: Theme.surface
    antialiasing: true
    clip: true

    PhotonWebView {
        anchors.fill: parent
        browser: root.browser
        cornerRadius: root.radius
    }
}
