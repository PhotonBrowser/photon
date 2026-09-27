import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Photon
import Photon as PhotonUi

ApplicationWindow {
    id: window
    visible: true
    width: Theme.defaultWindowWidth
    height: Theme.defaultWindowHeight
    minimumWidth: Theme.minimumWindowWidth
    minimumHeight: Theme.minimumWindowHeight
    title: browser.title.length > 0 ? browser.title + " — Photon" : "Photon"
    color: Theme.clearColor
    topPadding: Theme.zero
    readonly property bool isMacOS: Qt.platform.os === "osx"
    Shortcut {
        sequence: window.isMacOS ? "Meta+L" : "Ctrl+L"
        context: Qt.ApplicationShortcut
        onActivated: titleBar.focusOmnibox()
    }
    flags: Qt.Window | (isMacOS ? Qt.ExpandedClientAreaHint | Qt.NoTitleBarBackgroundHint : Qt.FramelessWindowHint)

    background: Rectangle {
        color: Theme.background
        radius: Theme.radius
        antialiasing: true
    }

    ColumnLayout {
        id: chromeLayout
        anchors.fill: parent
        spacing: Theme.zero
        readonly property real titlebarSafeHeight: window.isMacOS ? SafeArea.margins.top : Theme.zero
        readonly property real titlebarSafeLeft: window.isMacOS ? SafeArea.margins.left : Theme.zero
        readonly property real titlebarSafeRight: window.isMacOS ? SafeArea.margins.right : Theme.zero

        PhotonUi.TitleBar {
            id: titleBar
            window: window
            toggleMaximized: function () {
                window.toggleMaximized();
            }
            macOS: window.isMacOS
            safeHeight: chromeLayout.titlebarSafeHeight
            safeLeft: chromeLayout.titlebarSafeLeft
            safeRight: chromeLayout.titlebarSafeRight
            browser: browser
        }

        PhotonPage {
            Layout.fillWidth: true
            Layout.fillHeight: true

            contentItem: PhotonUi.WebSurface {
                browser: browser
            }
        }
    }

    // Keep Rust browser state alive until PhotonWebView has shut down its engine callbacks.
    BrowserController {
        id: browser
    }

    function toggleMaximized() {
        visibility = visibility === Window.Maximized ? Window.Windowed : Window.Maximized;
    }
}
