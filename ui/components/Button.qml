import QtQuick
import Photon

PressableControl {
    id: root

    property color hoverColor: Theme.controlHover
    property color pressedColor: Theme.controlPressed
    property color textColor: Theme.foreground
    property int cornerRadius: Theme.buttonRadius

    implicitWidth: Math.max(Theme.buttonMinimumWidth, label.implicitWidth + Theme.buttonHorizontalPadding)
    implicitHeight: Theme.buttonHeight
    padding: Theme.zero

    background: Rectangle {
        radius: root.cornerRadius
        color: root.down ? root.pressedColor : root.hovered ? root.hoverColor : Theme.clearColor

        Behavior on color {
            ColorAnimation {
                duration: Theme.controlTransitionDuration
            }
        }
    }

    contentItem: Item {
        Text {
            id: label

            anchors.centerIn: parent
            text: root.text
            color: root.textColor
            font: root.font
        }
    }
}
