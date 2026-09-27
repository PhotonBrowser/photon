import QtQuick
import QtQuick.Controls
import Photon

PressableControl {
    id: root

    property url iconSource
    property real buttonSize: Theme.iconButtonSize
    property bool destructive: false

    implicitWidth: buttonSize
    implicitHeight: buttonSize
    padding: Theme.zero
    display: AbstractButton.IconOnly
    icon.source: root.iconSource
    icon.width: Theme.iconSize
    icon.height: Theme.iconSize
    icon.color: root.destructive && root.hovered ? Theme.destructiveForeground : Theme.iconColor

    background: Rectangle {
        radius: Math.min(Theme.buttonRadius, root.height * Theme.iconButtonRadiusFactor)
        color: {
            if (root.destructive)
                return root.down ? Theme.destructivePressed : root.hovered ? Theme.destructiveHover : Theme.clearColor;
            if (root.down)
                return Theme.controlPressed;
            return root.hovered ? Theme.controlHover : Theme.clearColor;
        }

        Behavior on color {
            enabled: !root.destructive
            ColorAnimation {
                duration: Theme.controlTransitionDuration
            }
        }
    }
}
