import QtQuick 2.12

Item {
    id: mark
    property bool speaking: false
    property bool animate: false
    property real phase: 0
    readonly property real unit: width / 300
    NumberAnimation on phase {
        from: 0; to: Math.PI * 2; duration: 1550; loops: Animation.Infinite
        running: mark.animate && mark.visible && mark.speaking
    }
    // Radial strokes form an instrument around a small, open house-shaped face.
    // They react only to the manual speaking demo, with no audio input.
    Repeater {
        model: 64
        Item {
            width: mark.width; height: mark.height
            rotation: index * 360 / 64
            Rectangle {
                x: (parent.width - width) / 2; y: mark.unit * 13
                width: mark.unit * 2.4; height: mark.unit * 15
                radius: width / 2
                color: index % 8 === 0 ? "#ff6a2e" : "#777971"
                transform: Scale {
                    origin.x: mark.unit * 1.2; origin.y: mark.unit * 15
                    yScale: mark.speaking ? 0.5 + 1.9 * Math.abs(Math.sin(mark.phase + index * 0.43) * Math.cos(mark.phase * 2 - index * 0.17)) : 0.65
                }
            }
        }
    }
    Rectangle {
        anchors.centerIn: parent
        width: parent.width * 0.72; height: width; radius: width / 2
        color: "transparent"; border.color: "#343530"; border.width: Math.max(1, mark.unit)
    }
    Image {
        anchors.centerIn: parent; width: parent.width * 0.49; height: width
        source: "file:///var/lib/pearwebos/ui/ember-face.png"
        sourceSize.width: Math.ceil(width)
        smooth: true
    }
    Row {
        anchors.horizontalCenter: parent.horizontalCenter; y: parent.height * 0.45
        spacing: mark.unit * 29
        Repeater {
            model: 2
            Rectangle { width: mark.unit * 8; height: mark.unit * 15; radius: width / 2; color: "#ff6a2e" }
        }
    }
    Rectangle {
        anchors.horizontalCenter: parent.horizontalCenter; y: parent.height * 0.60
        width: mark.unit * 24; height: mark.unit * 3; radius: height / 2; color: "#ecece6"
        scale: mark.speaking ? 0.6 + 0.8 * Math.abs(Math.sin(mark.phase * 2)) : 1
    }
}
