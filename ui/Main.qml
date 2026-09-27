import QtQuick
import QtQuick.Controls
import Photon
import "theme"

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

        Column {
            anchors.centerIn: parent
            spacing: Theme.spacing
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: "Photon"
                color: Theme.foreground
                font.pixelSize: 32
                font.weight: Font.Medium
            }
            Rectangle {
                width: 440
                height: 1
                color: Theme.surface
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: "Your web, on your terms."
                color: Theme.subdued
                font.pixelSize: 14
            }
            Rectangle {
                width: 720
                height: 400
                radius: Theme.radius
                color: Theme.surface

                PhotonWebView {
                    anchors.fill: parent
                }

                Text {
                    anchors.centerIn: parent
                    text: "Page presentation placeholder"
                    color: Theme.subdued
                    font.pixelSize: 14
                }
            }
        }
    }
}
