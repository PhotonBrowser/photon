import QtQuick
import QtQuick.Controls
import Photon

ApplicationWindow {
    id: rootWindow
    visible: true
    width: 1100
    height: 720
    minimumWidth: 640
    minimumHeight: 420
    title: "Photon"
    color: Theme.background

    Rectangle {
        anchors.fill: parent
        color: Theme.background

        Column {
            anchors.centerIn: parent
            spacing: Theme.spacing
            Rectangle {
                width: Math.max(1, Math.min(720, rootWindow.width - 48))
                height: Math.max(160, Math.min(400, rootWindow.height - 320))
                radius: Theme.radius
                color: Theme.surface

                PhotonWebView {
                    anchors.fill: parent
                    cornerRadius: Theme.radius
                }
            }
        }
    }
}
