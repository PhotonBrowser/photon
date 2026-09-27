import QtQuick
import QtQuick.Controls
import Photon

TextField {
    id: root

    required property string url
    required property bool loading
    signal submitted(string text)
    property bool editorDirty: false

    leftPadding: Theme.inputHorizontalPadding
    rightPadding: Theme.inputHorizontalPadding
    topPadding: Theme.inputVerticalPadding
    bottomPadding: Theme.inputVerticalPadding
    color: Theme.foreground
    placeholderText: "Enter address"
    selectByMouse: true

    background: Rectangle {
        radius: Theme.radius
        color: Theme.surface
        border.width: root.activeFocus ? Theme.inputFocusBorderWidth : Theme.zero
        border.color: Theme.focusBorder
    }

    onTextEdited: editorDirty = true
    Component.onCompleted: text = url
    onActiveFocusChanged: {
        if (!activeFocus) {
            editorDirty = false;
            text = root.url;
        }
    }
    onUrlChanged: {
        if (!activeFocus || !editorDirty) {
            editorDirty = false;
            text = url;
        }
    }
    onAccepted: {
        submit();
    }

    Keys.priority: Keys.BeforeItem
    Keys.onReturnPressed: event => {
        event.accepted = true;
        submit();
    }
    Keys.onEnterPressed: event => {
        event.accepted = true;
        submit();
    }

    function submit() {
        editorDirty = false;
        submitted(text);
    }

    function focusAndSelectAll() {
        forceActiveFocus();
        selectAll();
    }
}
