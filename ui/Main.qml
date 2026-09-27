import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Photon

ApplicationWindow {
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

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: Theme.spacing
            spacing: Theme.spacing

            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.minimumHeight: 0

                radius: Theme.radius
                color: Theme.surface

                clip: true

                PhotonWebView {
                    anchors.fill: parent
                    cornerRadius: Theme.radius
                }
            }
        }
    }
}
