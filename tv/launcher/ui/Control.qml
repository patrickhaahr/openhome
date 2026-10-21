import QtQuick 2.12

Item {
    id: control
    property string label
    property string glyph
    property string family
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
        anchors.fill: parent
        radius: height / 2
        color: control.selected ? "#ecece6" : "#16171a"
        border.width: control.failed ? 3 : control.accepted ? 3 : 0
        border.color: control.failed ? "#ff8a7a" : "#ff6a2e"
    }
    Glyph {
        anchors.horizontalCenter: parent.horizontalCenter
        y: parent.height * 0.16
        width: parent.height * 0.31; height: width
        kind: control.glyph
        ink: control.selected ? "#000000" : "#ecece6"
        opacity: control.pending ? 0.4 : 1
    }
    TVText {
        anchors.horizontalCenter: parent.horizontalCenter
        y: parent.height * 0.59
        text: control.label
        font.family: control.family
        font.pixelSize: parent.height * 0.18
        color: control.selected ? "#000000" : "#ecece6"
    }
    // Acknowledgement is a temporary ring, never an inferred device state.
    Rectangle {
        visible: control.pending
        anchors.centerIn: parent
        width: parent.height * 0.17; height: width; radius: width / 2
        color: "transparent"; border.width: 2; border.color: "#ff6a2e"
    }
    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        onEntered: control.hovered()
        onClicked: control.activated()
    }
}
