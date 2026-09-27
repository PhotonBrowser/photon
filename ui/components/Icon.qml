import QtQuick
import QtQuick.Effects
import Photon

Item {
    id: root

    property url source
    property color color: Theme.iconColor
    property real iconSize: Theme.iconSize

    implicitWidth: iconSize
    implicitHeight: iconSize

    Rectangle {
        id: tintSource

        anchors.fill: parent
        color: root.color
        visible: false
    }

    Image {
        id: alphaMask

        anchors.fill: parent
        source: root.source
        visible: false
    }

    MultiEffect {
        source: tintSource
        anchors.fill: tintSource
        maskEnabled: true
        maskSource: alphaMask
        maskSpreadAtMax: 1
    }
}
