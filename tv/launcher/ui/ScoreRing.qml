import QtQuick 2.12

Item {
    id: ring
    property string label
    property string glyph
    property string family
    property var value: null
    property bool stale: false
    readonly property color ink: value === null ? "#9a9ea6" : glyph === "effort" ? "#5b9dff" : value >= 67 ? "#3ddc84" : value >= 34 ? "#ffcc33" : "#ff6b5e"
    Accessible.role: Accessible.StaticText
    Accessible.name: label + (value === null ? " unavailable" : " " + Math.round(value) + " of 100") + (stale ? ", stale" : "")
    Canvas {
        id: arc
        width: parent.width; height: width
        onPaint: {
            var c = getContext("2d"); c.reset();
            var w = width * 0.042, radius = width / 2 - w;
            c.lineWidth = w; c.lineCap = "round";
            c.strokeStyle = "#202124"; c.beginPath(); c.arc(width / 2, height / 2, radius, 0, Math.PI * 2); c.stroke();
            if (ring.value !== null && ring.value > 0) {
                c.strokeStyle = ring.ink; c.beginPath();
                c.arc(width / 2, height / 2, radius, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * Math.max(0, Math.min(1, ring.value / 100))); c.stroke();
            }
        }
        Connections { target: ring; onValueChanged: arc.requestPaint(); onInkChanged: arc.requestPaint() }
    }
    Glyph {
        x: (parent.width - width) / 2; y: parent.width * 0.17
        width: parent.width * 0.16; height: width
        kind: ring.glyph; ink: ring.ink
    }
    TVText {
        anchors.horizontalCenter: parent.horizontalCenter; y: parent.width * 0.32
        text: ring.value === null ? "–" : Math.round(ring.value)
        font.family: ring.family; font.pixelSize: parent.width * 0.32
        color: "#ecece6"
    }
    TVText {
        anchors.horizontalCenter: parent.horizontalCenter; y: parent.width + parent.width * 0.08
        text: ring.label
        font.family: ring.family; font.pixelSize: parent.width * 0.15
        color: "#cfd0ca"
    }
}
