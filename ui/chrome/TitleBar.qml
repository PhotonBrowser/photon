import QtQuick
import QtQuick.Layouts
import Photon

Rectangle {
    id: root

    required property var window
    required property var toggleMaximized
    required property bool macOS
    required property real safeHeight
    required property real safeLeft
    required property real safeRight
    required property var browser
    color: Theme.background
    topLeftRadius: Theme.radius
    topRightRadius: Theme.radius
    Layout.fillWidth: true
    Layout.preferredHeight: (macOS ? safeHeight : Theme.zero) + Theme.titlebarContentHeight

    ColumnLayout {
        anchors.fill: parent
        anchors.leftMargin: Theme.titlebarHorizontalPadding + root.safeLeft
        anchors.rightMargin: Theme.titlebarHorizontalPadding + root.safeRight
        anchors.topMargin: (root.macOS ? root.safeHeight : Theme.zero) + Theme.windowPadding
        anchors.bottomMargin: Theme.windowPadding
        spacing: Theme.spacing

        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: Theme.titlebarControlHeight
            spacing: Theme.spacing

            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                TapHandler {
                    acceptedButtons: Qt.LeftButton
                    onDoubleTapped: root.toggleMaximized()
                }

                DragHandler {
                    target: null
                    onActiveChanged: if (active)
                        root.window.startSystemMove()
                }
            }

            IconButton {
                visible: !root.macOS
                Layout.preferredWidth: Theme.titlebarButtonSize
                Layout.fillHeight: true
                buttonSize: Theme.titlebarButtonSize
                iconSource: Qt.resolvedUrl("icons/lucide/minimize.svg")
                Accessible.name: "Minimize"
                onClicked: root.window.showMinimized()
            }

            IconButton {
                visible: !root.macOS
                Layout.preferredWidth: Theme.titlebarButtonSize
                Layout.fillHeight: true
                buttonSize: Theme.titlebarButtonSize
                iconSource: Qt.resolvedUrl(root.window.visibility === Window.Maximized ? "icons/lucide/restore.svg" : "icons/lucide/maximize.svg")
                Accessible.name: root.window.visibility === Window.Maximized ? "Restore" : "Maximize"
                onClicked: root.toggleMaximized()
            }

            IconButton {
                visible: !root.macOS
                Layout.preferredWidth: Theme.titlebarButtonSize
                Layout.fillHeight: true
                buttonSize: Theme.titlebarButtonSize
                destructive: true
                iconSource: Qt.resolvedUrl("icons/lucide/close.svg")
                Accessible.name: "Close"
                onClicked: root.window.close()
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: Theme.titlebarControlHeight
            spacing: Theme.spacing

            IconButton {
                Layout.preferredWidth: Theme.titlebarButtonSize
                Layout.fillHeight: true
                enabled: root.browser.canGoBack
                iconSource: Qt.resolvedUrl("icons/lucide/back.svg")
                Accessible.name: "Back"
                onClicked: root.browser.back()
            }

            IconButton {
                Layout.preferredWidth: Theme.titlebarButtonSize
                Layout.fillHeight: true
                enabled: root.browser.canGoForward
                iconSource: Qt.resolvedUrl("icons/lucide/forward.svg")
                Accessible.name: "Forward"
                onClicked: root.browser.forward()
            }

            IconButton {
                Layout.preferredWidth: Theme.titlebarButtonSize
                Layout.fillHeight: true
                iconSource: Qt.resolvedUrl(root.browser.loading ? "icons/lucide/close.svg" : "icons/lucide/reload.svg")
                Accessible.name: root.browser.loading ? "Cancel loading" : "Reload"
                onClicked: root.browser.loading ? root.browser.cancelNavigation() : root.browser.reload()
            }

            Omnibox {
                id: omnibox
                Layout.fillWidth: true
                Layout.fillHeight: true
                url: root.browser.url
                loading: root.browser.loading
                onSubmitted: text => root.browser.navigate(text)
            }
        }
    }

    function focusOmnibox() {
        omnibox.focusAndSelectAll();
    }
}
