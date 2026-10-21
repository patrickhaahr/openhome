import QtQuick 2.12

Item {
    id: screen
    property string family
    property bool speaking: false
    property bool reducedMotion: false
    readonly property real s: width / 1920
    signal dismissed()
    onVisibleChanged: if (!visible) speaking = false
    Rectangle { anchors.fill: parent; color: "#000000" }
    TVText {
        x: 96 * screen.s; y: 68 * screen.s
        text: "OpenHome / Ember"; font.family: screen.family; font.pixelSize: 30 * screen.s; color: "#ecece6"
    }
    TVText {
        anchors.right: parent.right; anchors.rightMargin: 96 * screen.s; y: 70 * screen.s
        text: "Visual concept"; font.family: screen.family; font.pixelSize: 26 * screen.s; color: "#9a9ea6"
    }
    AgentMark {
        anchors.horizontalCenter: parent.horizontalCenter; y: 194 * screen.s
        width: 530 * screen.s; height: width
        speaking: screen.speaking; animate: screen.visible && !screen.reducedMotion
    }
    TVText {
        anchors.horizontalCenter: parent.horizontalCenter; y: 766 * screen.s
        text: screen.speaking ? "A voice, taking shape." : "A little presence. A home that listens."
        font.family: screen.family; font.pixelSize: 42 * screen.s; color: "#ecece6"
    }
    TVText {
        anchors.horizontalCenter: parent.horizontalCenter; y: 837 * screen.s
        text: "Voice and agent connections are not enabled."
        font.family: screen.family; font.pixelSize: 27 * screen.s; color: "#9a9ea6"
    }
    TVText {
        anchors.horizontalCenter: parent.horizontalCenter; y: 951 * screen.s
        text: "OK  " + (screen.speaking ? "Stop speaking demo" : "Play speaking demo") + "     ·     Back  Close"
        font.family: screen.family; font.pixelSize: 27 * screen.s; color: "#cfd0ca"
    }
    MouseArea { anchors.fill: parent; onClicked: screen.speaking = !screen.speaking }
}
