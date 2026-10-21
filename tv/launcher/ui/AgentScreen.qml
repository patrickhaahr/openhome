import QtQuick 2.12

Item {
    id: screen
    property bool speaking: false
    property bool reducedMotion: false
    readonly property real s: width / 1920
    signal dismissed()
    onVisibleChanged: if (!visible) speaking = false
    Rectangle { anchors.fill: parent; color: "#000000" }
    AgentMark {
        anchors.centerIn: parent; width: 800 * screen.s; height: width
        speaking: screen.speaking; animate: screen.visible && !screen.reducedMotion
    }
    MouseArea {
        anchors.fill: parent
        Accessible.role: Accessible.Button
        Accessible.name: screen.speaking ? "Stop animeret prøve" : "Afspil animeret prøve"
        onClicked: screen.speaking = !screen.speaking
    }
    Rectangle {
        x: parent.width - 176 * screen.s; y: 80 * screen.s
        width: 88 * screen.s; height: width; radius: width / 2; color: "#16171a"
        Glyph { anchors.centerIn: parent; width: 36 * screen.s; height: width; kind: "close" }
        MouseArea {
            anchors.fill: parent; Accessible.role: Accessible.Button; Accessible.name: "Luk agent"
            onClicked: screen.dismissed()
        }
    }
    Rectangle {
        x: parent.width - 176 * screen.s; y: parent.height - 168 * screen.s
        width: 88 * screen.s; height: width; radius: width / 2; color: "#16171a"
        Glyph { anchors.centerIn: parent; width: 34 * screen.s; height: width; kind: screen.speaking ? "pause" : "play" }
        MouseArea {
            anchors.fill: parent; Accessible.role: Accessible.Button
            Accessible.name: screen.speaking ? "Stop animeret prøve" : "Afspil animeret prøve"
            onClicked: screen.speaking = !screen.speaking
        }
    }
}
