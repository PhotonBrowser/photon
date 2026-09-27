pragma Singleton
import QtQuick

SystemPalette {
    readonly property color background: window
    readonly property color surface: mid
    readonly property color foreground: text
    readonly property color subdued: dark
    readonly property int spacing: 16
    readonly property int radius: 10
    readonly property int motion: 180
}
