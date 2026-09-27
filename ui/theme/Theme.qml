pragma Singleton
import QtQuick

SystemPalette {
    readonly property color background: window
    readonly property color surface: mid
    readonly property color foreground: text
    readonly property color subdued: dark
    readonly property int spacing: 4
    readonly property int radius: 6
    readonly property int motion: 180
}
