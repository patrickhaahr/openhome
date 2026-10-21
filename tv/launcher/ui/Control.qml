import QtQuick 2.12

Item {
    id: control
    property string label
    property string glyph
    property bool primary: false
    property bool selected: false
    property bool pending: false
    property bool accepted: false
    property bool failed: false
    signal activated()
    signal hovered()
    Accessible.role: Accessible.Button
    Accessible.name: label
    Accessible.onPressAction: activated()
    Rectangle {
        anchors.fill: parent; anchors.margins: -7
        radius: width / 2; color: "transparent"
        border.width: control.selected ? 3 : 0; border.color: "#ecece6"
    }
    Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: control.primary ? "#ecece6" : "#2a2b2d"
        border.width: control.failed || control.accepted ? 3 : 0
        border.color: control.failed ? "#ff8a7a" : "#ff6a2e"
    }
    Glyph {
        anchors.centerIn: parent
        width: parent.height * 0.46; height: width
        kind: control.glyph
        ink: control.primary ? "#000000" : "#ecece6"
        opacity: control.pending ? 0.4 : 1
    }
    // Acknowledgement is a temporary ring, never an inferred device state.
    Rectangle {
        visible: control.pending
        anchors.centerIn: parent
        width: parent.height * 0.17; height: width; radius: width / 2
        color: "transparent"; border.width: 2; border.color: "#ff6a2e"
    }
    Glyph {
        visible: control.failed
        anchors.right: parent.right; anchors.top: parent.top
        width: parent.height * 0.26; height: width
        kind: "warning"; ink: "#ff8a7a"
    }
    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        onEntered: control.hovered()
        onClicked: control.activated()
    }
}
