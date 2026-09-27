import QtQuick
import QtQuick.Controls

Button {
    id: root

    property real pressedScale: Theme.pressedScale

    scale: down ? pressedScale : Theme.restScale

    Behavior on scale {
        SpringAnimation {
            spring: Theme.pressSpring
            damping: Theme.pressDamping
            mass: Theme.pressMass
        }
    }
}
