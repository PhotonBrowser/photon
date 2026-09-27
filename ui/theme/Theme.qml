pragma Singleton
import QtQuick
import Photon as PhotonModule

QtObject {
    readonly property bool isDark: PhotonModule.PhotonAppearance.dark

    readonly property color background: isDark ? "#17181c" : "#f4f5f7"
    readonly property color surface: isDark ? "#22242a" : "#ffffff"
    readonly property color foreground: isDark ? "#f2f3f5" : "#202124"
    readonly property color subdued: isDark ? "#a4a7b0" : "#747780"
    readonly property color iconColor: foreground
    readonly property color clearColor: "transparent"
    readonly property color focusBorder: foreground
    readonly property color controlHover: isDark ? "#303239" : "#e8e9ec"
    readonly property color controlPressed: isDark ? "#3c3e46" : "#d9dbe0"
    readonly property color destructiveHover: "#c42b1c"
    readonly property color destructivePressed: "#9f2117"
    readonly property color destructiveForeground: "#ffffff"

    readonly property int defaultWindowWidth: 1100
    readonly property int defaultWindowHeight: 720
    readonly property int minimumWindowWidth: 640
    readonly property int minimumWindowHeight: 420
    readonly property int titlebarContentHeight: 76
    readonly property int titlebarControlHeight: 32
    readonly property int titlebarButtonSize: 32
    readonly property int iconButtonSize: 40
    readonly property int titlebarHorizontalPadding: 8
    readonly property int buttonHeight: 36
    readonly property int buttonMinimumWidth: 80
    readonly property int buttonHorizontalPadding: 24
    readonly property int buttonRadius: 8
    readonly property int inputHorizontalPadding: 12
    readonly property int inputVerticalPadding: 6
    readonly property int inputFocusBorderWidth: 1
    readonly property real iconSize: 16
    readonly property real iconButtonRadiusFactor: 0.2
    readonly property real pressedScale: 0.94
    readonly property real restScale: 1
    readonly property real pressSpring: 4
    readonly property real pressDamping: 0.3
    readonly property real pressMass: 0.35
    readonly property int controlTransitionDuration: 120
    readonly property int spacing: 4
    readonly property int windowPadding: 4
    readonly property int radius: 6
    readonly property int zero: 0
}
