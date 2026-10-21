import QtQuick 2.12

Item {
    id: mark
    property bool speaking: false
    property bool animate: false
    property real phase: 0
    property real sweep: 0
    readonly property real unit: width / 300
    NumberAnimation on phase {
        from: 0; to: Math.PI * 2; duration: 1450; loops: Animation.Infinite
        running: mark.animate && mark.visible && mark.speaking
    }
    NumberAnimation on sweep {
        from: 0; to: 360; duration: 24000; loops: Animation.Infinite
        running: mark.animate && mark.visible && mark.speaking
    }
    Image {
        anchors.fill: parent
        source: "file:///var/lib/pearwebos/ui/icons/agent-frame.png"
        smooth: true; mipmap: true
    }
    Image {
        anchors.fill: parent; rotation: mark.sweep
        source: "file:///var/lib/pearwebos/ui/icons/agent-rotor.png"
        smooth: true; mipmap: true
    }
    Repeater {
        model: 48
        Item {
            width: mark.width; height: mark.height; rotation: index * 7.5
            Rectangle {
                x: (parent.width - width) / 2; y: mark.unit * 96
                width: mark.unit * 1.2; height: mark.unit * 12; radius: width / 2
                color: index % 4 === 0 ? "#00dbe8" : "#08699a"
                transform: Scale {
                    origin.x: mark.unit * 0.6; origin.y: 0
                    yScale: mark.speaking ? 0.45 + 1.45 * Math.abs(Math.sin(mark.phase + index * 0.48) * Math.cos(mark.phase * 1.6 - index * 0.24)) : 0.65
                }
            }
        }
    }
    Rectangle {
        anchors.centerIn: parent; width: parent.width * 0.22; height: width; radius: width / 2
        color: "#02232e"; border.width: Math.max(1, mark.unit * 0.7); border.color: "#07839a"
    }
    Rectangle {
        anchors.centerIn: parent; width: parent.width * 0.12; height: width; radius: width / 2
        color: "#00dbe8"
        scale: mark.speaking ? 0.94 + 0.10 * Math.abs(Math.sin(mark.phase)) : 1
    }
}
